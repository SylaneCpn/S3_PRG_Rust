//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

use std::{
    io::{BufRead, BufReader, ErrorKind, Write},
    net::TcpStream,
    path::Path,
};

use serde::{Deserialize, Serialize};

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
fn game_client_init(
    argc: std::ffi::c_int,
    argv: *const *const std::ffi::c_char,
    inout_width: &mut std::ffi::c_int,
    inout_height: &mut std::ffi::c_int,
    inout_dt: &mut std::ffi::c_double,
) -> *mut std::ffi::c_void /* application */ {
    let args_utf8 = Vec::from_iter((0..argc).map(|a| {
        let c_ptr = unsafe { argv.offset(a as isize) };
        let c_str = unsafe { std::ffi::CStr::from_ptr(*c_ptr) };
        c_str.to_string_lossy()
    }));
    let args = Vec::from_iter(args_utf8.iter().map(|a| a.as_ref()));
    let mut w = *inout_width as usize;
    let mut h = *inout_height as usize;
    let mut dt = *inout_dt;
    match init_application(&args, &mut w, &mut h, &mut dt) {
        Ok(app) => {
            *inout_width = w as std::ffi::c_int;
            *inout_height = h as std::ffi::c_int;
            *inout_dt = dt as std::ffi::c_double;
            Box::into_raw(Box::new(app)) as *mut _
        }
        Err(e) => {
            eprintln!("ERROR: {}", e);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
fn game_client_update(
    c_evt: *const std::ffi::c_char,
    x: std::ffi::c_int,
    y: std::ffi::c_int,
    w: std::ffi::c_int,
    h: std::ffi::c_int,
    btn: std::ffi::c_int,
    c_key: *const std::ffi::c_char,
    c_screen: *mut std::ffi::c_char,
    c_app: *mut std::ffi::c_void,
) -> std::ffi::c_int /* -1: quit    0: go-on    1: redraw */ {
    let evt = unsafe { std::ffi::CStr::from_ptr(c_evt) }.to_string_lossy();
    let key = unsafe { std::ffi::CStr::from_ptr(c_key) }.to_string_lossy();
    let point = Point { x, y };
    let mut screen = Screen {
        width: w as usize,
        height: h as usize,
        pixels: unsafe { std::slice::from_raw_parts_mut(c_screen as *mut Color, (w * h) as usize) },
    };
    let app = unsafe { &mut *(c_app as *mut Application) };
    let status = update_application(
        evt.as_ref(),
        key.as_ref(),
        btn as usize,
        &point,
        &mut screen,
        app,
    )
    .unwrap_or_else(|e| {
        eprintln!("ERROR: {}", e);
        UpdateStatus::Quit
    });
    match status {
        UpdateStatus::GoOn => 0,
        UpdateStatus::Redraw => 1,
        UpdateStatus::Quit => {
            // ensure deallocation
            let _owned = unsafe { Box::from_raw(app) };
            -1
        }
    }
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#[derive(Debug)]
struct Screen<'a> {
    width: usize,
    height: usize,
    pixels: &'a mut [Color],
}

#[derive(Debug, Clone, Copy)]
enum UpdateStatus {
    GoOn,
    Redraw,
    Quit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Debug)]
struct Application {
    status: UpdateStatus,
    image: Image,
    position: Point,
    input: BufReader<TcpStream>,
    output: TcpStream,
    //id : usize,
    players: Vec<Player>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Image {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Player {
    id: usize,
    image: Image,
    position: Point,
}

#[derive(Debug, Serialize, Deserialize)]
struct SelfData {
    id: usize,
    position: Point,
}

impl Image {
    fn load<P>(path: P) -> Result<Self, Box<dyn std::error::Error>>
    where
        P: AsRef<Path>,
    {
        let content = std::fs::read_to_string(path)?;
        let mut words = content
            .as_str()
            .lines()
            .map(|l| l.find('#').map_or(l, |pos| &l[0..pos]))
            .flat_map(|l| l.split_whitespace())
            .filter(|w| !w.is_empty());

        //Extract format
        let _p3 = match words.next().ok_or("Invalid Format")? {
            "P3" => Ok("P3"),
            _ => Err("Invalid Format"),
        }?;

        //Extract width
        let width = words.next().ok_or("Invalid Format")?.parse::<usize>()?;

        //Extract height
        let height = words.next().ok_or("Invalid Format")?.parse::<usize>()?;

        //Extract maximul value
        let _max_val = words.next().ok_or("Invalid Format")?.parse::<u8>()?;

        let mut pixels: Vec<Color> = Vec::new();

        for _ in 0..width * height {
            let r = words.next().ok_or("Invalid Format")?.parse::<u8>()?;
            let g = words.next().ok_or("Invalid Format")?.parse::<u8>()?;
            let b = words.next().ok_or("Invalid Format")?.parse::<u8>()?;
            pixels.push(Color { r, g, b });
        }

        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    fn draw(&self, screen: &mut Screen, position: Point, transparency: Option<Color>) {
        let p0 = Point {
            x: position.x.clamp(0, screen.width as i32),
            y: position.y.clamp(0, screen.height as i32),
        };
        let p1 = Point {
            x: (position.x + self.width as i32).clamp(0, screen.width as i32),
            y: (position.y + self.height as i32).clamp(0, screen.height as i32),
        };
        let dx = 0.max(p0.x - position.x);
        let dy = 0.max(p0.y - position.y);
        let mut i_idx = dy as usize * self.width + dx as usize;
        let mut s_idx = p0.y as usize * screen.width + p0.x as usize;
        let w = 0.max(p1.x - p0.x) as usize;
        for _ in p0.y..p1.y {
            let src = &self.pixels[i_idx..i_idx + w];
            let dst = &mut screen.pixels[s_idx..s_idx + w];

            match transparency {
                None => {
                    for (d, s) in dst.iter_mut().zip(src.iter()) {
                        d.r = s.r;
                        d.g = s.g;
                        d.b = s.b;
                    }
                }

                Some(tr) => {
                    for (d, s) in dst.iter_mut().zip(src.iter()) {
                        if !((s.r == tr.r) && (s.g == tr.g) && (s.b == tr.b)) {
                            d.r = s.r;
                            d.g = s.g;
                            d.b = s.b;
                        }
                    }
                }
            }

            // assign to each Color of `dst` the corresponding Color from `src`

            i_idx += self.width;
            s_idx += screen.width;
        }
    }
}

fn init_application(
    args: &[&str],
    width: &mut usize,
    height: &mut usize,
    dt: &mut f64,
) -> Result<Application, Box<dyn std::error::Error>> {
    println!("args: {:?}", args);
    *width = 800;
    *height = 600;
    *dt = 1.0 / 30.0;
    println!("{}×{}@{:.3}", width, height, dt);
    let image = match args.get(2) {
        Some(path) => Image::load(*path),
        _ => Image::load("data/cat01.ppm"),
    }?;
    let addr = *args.get(3).unwrap_or(&"192.168.181.86");
    let port = *args.get(4).unwrap_or(&"8000");

    let full_addr = format!("{addr}:{port}");
    println!("connecting to server {full_addr}");

    let stream = TcpStream::connect(full_addr)?;
    println!("connected to {:?}", stream.peer_addr()?);
    let mut output = stream.try_clone()?;
    let mut input = BufReader::new(stream);

    println!("Sending image data to server...");
    output.write_all(format!("{}\n", serde_json::to_string(&image)?).as_bytes())?;

    println!("Recieving id from server...");
    let mut self_data_str = String::new();
    input.read_line(&mut self_data_str)?;
    let self_data = serde_json::from_str::<SelfData>(&self_data_str)?;

    let id = self_data.id;
    println!("client id = {id}...");
    let position = self_data.position;

    let mut app = Application {
        status: UpdateStatus::GoOn,
        image,
        position,
        input,
        output,
        //id,
        players: Vec::new(),
    };

    get_players_data(&mut app)?;

    Ok(app)
}

fn update_application(
    evt: &str,
    key: &str,
    btn: usize,
    point: &Point,
    screen: &mut Screen,
    app: &mut Application,
) -> Result<UpdateStatus, Box<dyn std::error::Error>> {
    let _maybe_unused = /* prevent some warnings */ (btn, point);
    if evt != "T" {
        println!(
            "evt={:?} btn={} key={:?} ({};{}) {}×{}",
            evt, btn, key, point.x, point.y, screen.width, screen.height
        );
    }
    app.status = UpdateStatus::GoOn;
    if let Some(motion) = handle_event(app, evt, key) {
        // println!("motion: {:?}", motion);
        // app.position.x += motion.x;
        // app.position.y += motion.y;
        // app.status = UpdateStatus::Redraw;
        let msg = serde_json::to_string(&motion)?;
        println!("sending motion to the server...");
        app.output.write_all(format!("{msg}\n").as_bytes())?;
        println!("waiting for a response...");

        // let mut new_pos_str = String::new();
        // app.input.read_line(&mut new_pos_str)?;

        // let new_pos = serde_json::from_str::<Point>(&new_pos_str)?;
        // app.position = new_pos;
        handle_messages(app)?;
        get_players_data(app)?;
    }
    redraw_if_needed(app, screen);
    Ok(app.status)
}

fn handle_event(app: &mut Application, evt: &str, key: &str) -> Option<Point> {
    let mut motion = None;
    match evt {
        "C" => app.status = UpdateStatus::Redraw,
        "Q" => app.status = UpdateStatus::Quit,
        "KP" => match key {
            "Escape" => app.status = UpdateStatus::Quit,
            "Left" => motion = Some(Point { x: -10, y: 0 }),
            "Right" => motion = Some(Point { x: 10, y: 0 }),
            "Up" => motion = Some(Point { x: 0, y: -10 }),
            "Down" => motion = Some(Point { x: 0, y: 10 }),
            " " => app.status = UpdateStatus::Redraw,
            _ => {}
        },
        _ => {}
    }
    motion
}

fn redraw_if_needed(app: &Application, screen: &mut Screen) {
    if let UpdateStatus::Redraw = app.status {
        // for c in screen.pixels.iter_mut() {
        //     let (r, g, b) =
        //         (c.r as u32 + 10, c.g as u32 + 25, c.b as u32 + 35);
        //     c.r = r as u8;
        //     c.g = g as u8;
        //     c.b = b as u8;
        // }

        for c in screen.pixels.iter_mut() {
            c.r = 120;
            c.g = 120;
            c.b = 120;
        }

        for player in app.players.iter() {
            player
                .image
                .draw(screen, player.position, Some(Color { r: 0, g: 255, b: 0 }));
        }

        app.image
            .draw(screen, app.position, Some(Color { r: 0, g: 255, b: 0 }));
    }
}

fn read_lines_nonblocking(
    input: &mut BufReader<TcpStream>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    fn inner(input: &mut BufReader<TcpStream>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            match input.read_line(&mut line) {
                Ok(r) => {
                    if !line.is_empty() {
                        lines.push(line);
                    }
                    if r == 0 {
                        lines.push(String::new()); // EOF
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() != ErrorKind::WouldBlock {
                        Err(e)?
                    }
                    if line.is_empty() {
                        // line not started, don't wait for the end
                        break;
                    }
                }
            }
        }
        Ok(lines)
    }
    input.get_mut().set_nonblocking(true)?;
    let result = inner(input);
    input.get_mut().set_nonblocking(false)?;
    result
}

fn handle_messages(app: &mut Application) -> Result<(), Box<dyn std::error::Error>> {
    let response = read_lines_nonblocking(&mut app.input)?;
    for line in response.into_iter() {
        if line.is_empty() {
            app.status = UpdateStatus::Quit
        }

        if let Ok(data) = serde_json::from_str::<Point>(&line) {
            app.position.x += data.x;
            app.position.y += data.y;

            app.status = UpdateStatus::Redraw;
        } else {
            println!("{line}");
        }
    }
    Ok(())
}

fn get_players_data(app: &mut Application) -> Result<(), Box<dyn std::error::Error>> {
    let response = read_lines_nonblocking(&mut app.input)?;
    let mut players = Vec::new();
    for line in response.into_iter() {
        if line.is_empty() {
            app.status = UpdateStatus::Quit
        }

        if let Ok(data) = serde_json::from_str::<Player>(&line) {
            players.push(data);

            app.status = UpdateStatus::Redraw;
        } else {
            println!("{line}");
        }
    }
    app.players = players;
    Ok(())
}
//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
