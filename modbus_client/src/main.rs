mod modbus_utils;
mod utils;

use std::{env::args, time::Duration};

use modbus::{Client, tcp};

use modbus_utils::{
    Event, detect_coil_events, detect_holding_events, print_coils_and_holding_registers,
    store_events,
};
use utils::now_utc_ms;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = args().collect();

    let machine_addr = arguments.get(1).map(|a| a.as_str()).unwrap_or("127.0.0.1");
    let machine_port = arguments
        .get(2)
        .map(|a| a.parse::<u16>())
        .transpose()
        .expect("Invalid port number")
        .unwrap_or(55022);
    let db_name = arguments.get(3).map(|a| a.as_str()).unwrap_or("plc.db");

    let cfg = tcp::Config {
        tcp_port: machine_port,
        ..Default::default()
    };

    println!("Starting transport on {machine_addr}:{machine_port}");
    let mut transport =
        tcp::Transport::new_with_cfg(machine_addr, cfg).expect("Invalid IP address");

    let (coils_quantity, holding_registers_quantity) = match machine_port {
        502 => (256, 125),
        _ => (20, 5),
    };

    let stop_after = match machine_port {
        502 => 600 * 1000,
        _ => 20 * 1000,
    };

    let db = rusqlite::Connection::open(db_name).unwrap();

    db.busy_handler(Some(|_retry_count| {
        std::thread::sleep(std::time::Duration::from_millis(1));
        true
    }))
    .unwrap();
    db.execute(
        "CREATE TABLE IF NOT EXISTS event (
            id      INTEGER PRIMARY KEY,
            utc_ms  INTEGER,
            address TEXT,
            state   INTEGER )",
        (),
    )?;

    let mut coils = Vec::new();
    let mut holding_registers = Vec::new();
    let mut events = Vec::new();

    let (channel_sender, channel_receiver) = std::sync::mpsc::channel::<Vec<Event>>();
    let db_handler = std::thread::spawn(move || {
        if let Err(e) = store_events(&db, channel_receiver) {
            eprintln!("ERROR: {:?}", e);
        }
    });

    let start = now_utc_ms();
    let mut last_db_commit = 0;

    loop {
        let new_coils = transport.read_coils(0, coils_quantity)?;
        let new_holding_registers =
            transport.read_holding_registers(0, holding_registers_quantity)?;

        if !holding_registers.is_empty() {
            let diff = new_holding_registers[0].saturating_sub(holding_registers[0]);
            if diff > 1 {
                println!("Missed {diff} data packets");
                print_coils_and_holding_registers(&new_coils, &new_holding_registers);
            }
        }

        if !coils.eq(&new_coils) || !holding_registers.eq(&new_holding_registers) {
            //print_coils_and_holding_registers(&new_coils, &new_holding_registers);

            let now = now_utc_ms();

            detect_coil_events(&mut events, now, &coils, &new_coils);
            detect_holding_events(&mut events, now, &holding_registers, &new_holding_registers);

            coils = new_coils;
            holding_registers = new_holding_registers;
        }

        std::thread::sleep(Duration::from_millis(50));

        if now_utc_ms() - start > stop_after {
            break;
        }

        if now_utc_ms() - last_db_commit > 1000 {
            channel_sender.send(std::mem::take(&mut events))?;
            last_db_commit = now_utc_ms();
        }
    }

    channel_sender.send(events)?;
    drop(channel_sender);
    db_handler.join().expect("Thread aborted");
    Ok(())
}
