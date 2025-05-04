use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
};

use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let addr = local_ip()?;
    let tcp_port = args.get(1).map(|x| x.parse::<u16>()).unwrap_or(Ok(8000))?;
    let listener = std::net::TcpListener::bind((addr, tcp_port))?;
    println!(
        "tcp server waiting for connections at {} on port '{}'",
        addr, tcp_port
    );
    let players: Arc<Mutex<Vec<Player>>> = Arc::new(Mutex::new(Vec::new()));
    for incoming in listener.incoming() {
        let stream = incoming?;
        let state = Arc::clone(&players);
        println!("new connection from {:?}", stream.peer_addr()?);
        std::thread::spawn(move || {
            if let Err(e) = handle_connection(stream, state) {
                eprintln!("ERROR: {}", e);
            }
        });
    }

    Ok(())
}
fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<Vec<Player>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = stream.try_clone()?;
    let mut input = BufReader::new(stream);

    let mut player_image_str = String::new();
    //retrieve image from client
    input.read_line(&mut player_image_str)?;
    let player_image = serde_json::from_str::<Image>(&player_image_str)?;

    //shared state of the server
    let mut state_guard = state.lock().unwrap();

    let mut id = 0;
    //get an id for the player
    for i in 0.. {
        match state_guard.iter().find(|p| p.id == i) {
            Some(_) => {
                continue;
            }

            _ => {
                id = i;
                break;
            }
        }
    }

    //place the player on the map
    let position = Point {
        x: id as i32 * 20,
        y: id as i32 * 20,
    };

    let client_data = SelfData { position, id };

    //give it's data to the client
    output.write_all(format!("{}\n", serde_json::to_string(&client_data)?).as_bytes())?;

    let player_data = Player {
        id,
        image: player_image,
        position,
    };

    state_guard.push(player_data);

    send_players_data(&state_guard, id, &mut output)?;
    //drop the lock so the other threads can actually do something
    std::mem::drop(state_guard);

    loop {
        println!("\nwaiting for request from client...");
        let mut request = String::new();
        let r = input.read_line(&mut request)?;
        if r == 0 {
            println!("EOF");
            let mut state_guard = state.lock().unwrap();
            if let Some(pos) = state_guard.iter().position(|x| x.id == id) {
                state_guard.remove(pos);
            }
            break;
        }

        println!("obtained {:?} from client", request);

        if request.is_empty() {
            break;
        }

        if let Ok(data) = serde_json::from_str::<Point>(&request) {
            let mut state_guard = state.lock().unwrap();
            if let Some(player) = state_guard.iter_mut().find(|p| p.id == id) {
                player.position.x += data.x;
                player.position.y += data.y;

                let reply = serde_json::to_string(&data)?;
                println!("sending reply {:?} to client...", reply);
                output.write_all(format!("{reply}\n").as_bytes())?;
            }

            send_players_data(&state_guard, id, &mut output)?;
        } else {
            println!("{request}");
        }
    }

    Ok(())
}

fn send_players_data(
    players: &[Player],
    id: usize,
    output: &mut TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = String::new();
    for player in players.iter().filter(|p| p.id != id) {
        out = format!("{}{}\n", out, serde_json::to_string(player)?);
    }

    output.write_all(out.as_bytes())?;

    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
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

#[derive(Debug, Serialize, Deserialize)]
struct Image {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}
