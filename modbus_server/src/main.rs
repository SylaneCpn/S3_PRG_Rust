use std::{
    error::Error,
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rmodbus::{
    guess_request_frame_len,
    server::{context::ModbusContext, storage::ModbusStorage, ModbusFrame},
    ModbusFrameBuf, ModbusProto,
};

type ModbusSimu = ModbusStorage<20, 0, 0, 5>;

struct SharedState {
    unit_id: u8,
    context: RwLock<ModbusSimu>,
    must_quit: AtomicBool,
}

fn recv_request_bytes<'b>(
    stream: &mut TcpStream,
    buffer: &'b mut ModbusFrameBuf,
) -> Result<&'b [u8], Box<dyn Error>> {
    if let Err(e) = stream.read_exact(&mut buffer[..8]) {
        return match e.kind() {
            ErrorKind::UnexpectedEof => Ok(&buffer[0..0]),
            _ => Err(e)?,
        };
    }
    let length =
        guess_request_frame_len(&buffer[..8], ModbusProto::TcpUdp)? as usize;
    stream.read_exact(&mut buffer[8..length])?;
    Ok(&buffer[..length])
}

fn modbus_dialogue(
    mut stream: TcpStream,
    state: &SharedState,
) -> Result<(), Box<dyn Error>> {
    while !state.must_quit.load(Ordering::Relaxed) {
        let mut buffer = [0; 256];
        let bytes = recv_request_bytes(&mut stream, &mut buffer)?;
        if bytes.is_empty() {
            break; // EOF
        }
        let mut response = Vec::new();
        let mut frame = ModbusFrame::new(
            state.unit_id,
            bytes,
            ModbusProto::TcpUdp,
            &mut response,
        );
        frame.parse()?;
        if frame.processing_required {
            if frame.readonly {
                let guard = state.context.read().unwrap();
                frame.process_read(guard.deref())?;
            } else {
                let mut guard = state.context.write().unwrap();
                frame.process_write(guard.deref_mut())?;
            }
        }
        if frame.response_required {
            frame.finalize_response()?;
            stream.write_all(response.as_slice())?;
        }
    }
    Ok(())
}

fn modbus_tcp_server(
    tcp_port: u16,
    state: Arc<SharedState>,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, tcp_port))?;
    listener.set_nonblocking(true)?;
    println!(
        "modbus tcp server waiting for connections on port '{}'",
        tcp_port
    );
    while !state.must_quit.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                println!("new connection from {:?}", addr);
                stream.set_nonblocking(false)?;
                thread::spawn({
                    let state = Arc::clone(&state);
                    move || {
                        if let Err(e) =
                            modbus_dialogue(stream, state.as_ref())
                        {
                            eprintln!("{:?}", e);
                        }
                        println!("client {:?} disconnected", addr);
                    }
                });
            }
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    thread::sleep(Duration::from_millis(200));
                } else {
                    println!("ERROR {:?}", e);
                    state.must_quit.store(true, Ordering::Relaxed);
                    Err(e)?
                }
            }
        }
    }
    Ok(())
}

fn now_utc_ms() -> Result<u64, Box<dyn std::error::Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(now.as_millis() as u64)
}

fn run_plc(
    state: &SharedState,
    ms_div: u64,
) -> Result<(), Box<dyn Error>> {
    let (coil_count, holding_count) = {
        let context = state.context.read().unwrap();
        (context.coils.len(), context.holdings.len())
    };
    let mut counter = 0;
    let mut last_utc_ms = now_utc_ms()?;
    let mut coils = vec![false; coil_count];
    let mut holdings = vec![0; holding_count];
    while !state.must_quit.load(Ordering::Relaxed) {
        let utc_ms = now_utc_ms()?;
        if utc_ms / ms_div == last_utc_ms / ms_div {
            thread::sleep(Duration::from_micros(250));
            continue;
        }
        if let Ok(mut context) = state.context.try_write() {
            let low = coil_count / 3;
            let high = coil_count - low;
            let mut current = counter % (2 * (low - 1));
            if current >= low {
                current = 2 * (low - 1) - current;
            }
            for (i, c) in &mut coils[..low].iter_mut().enumerate() {
                *c = i == current;
            }
            let mut current = counter % (2 * high);
            if current > high {
                current = 2 * high - current;
            }
            for (i, c) in &mut coils[low..].iter_mut().enumerate() {
                *c = i < current;
            }
            context.set_coils_bulk(0, &coils)?;
            for (i, h) in holdings.iter_mut().enumerate() {
                *h = (counter / (i + 1)) as u16;
            }
            context.set_holdings_bulk(0, &holdings)?;
            counter += 1;
            last_utc_ms = utc_ms;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let ms_div = if std::env::args().any(|a| a == "fast") {
        1
    } else {
        100
    };
    let unit_id = 1;
    let tcp_port = 55022;
    let mut context = ModbusSimu::new();
    context.set_holding(0, context.coils.len() as u16)?;
    context.set_holding(1, context.discretes.len() as u16)?;
    context.set_holding(2, context.inputs.len() as u16)?;
    context.set_holding(3, context.holdings.len() as u16)?;
    println!("modbus unit id: {}", unit_id);
    println!("number of coils: {}", context.get_holding(0)?);
    println!("number of discrete inputs: {}", context.get_holding(1)?);
    println!("number of input registers: {}", context.get_holding(2)?);
    println!("number of holding registers: {}", context.get_holding(3)?);
    println!("changing state every {} ms", ms_div);
    let state = Arc::new(SharedState {
        unit_id,
        context: RwLock::new(context),
        must_quit: AtomicBool::new(false),
    });
    let th = thread::spawn({
        let state = Arc::clone(&state);
        move || {
            if let Err(e) = modbus_tcp_server(tcp_port, state) {
                panic!("{}", e);
            }
        }
    });
    let plc_result = run_plc(&state, ms_div);
    state.must_quit.store(true, Ordering::Relaxed);
    let server_result = th.join();
    match server_result {
        Ok(()) => plc_result,
        Err(s_err) => match plc_result {
            Ok(()) => Err(format!("{:?}", s_err))?,
            Err(p_err) => Err(format!("{:?}\n{:?}", s_err, p_err))?,
        },
    }
}
