use modbus::Coil;

pub fn coils_to_string(coils: &[Coil]) -> String {
    coils
        .iter()
        .map(|c| match c {
            Coil::Off => "_",
            Coil::On => "•",
        })
        .collect()
}

pub fn print_coils_and_holding_registers(coils: &[Coil], holding_registers: &[u16]) {
    println!("{} {:?}", coils_to_string(coils), holding_registers);
}

// pub fn store_coil_events(
//     db: &rusqlite::Connection,
//     utc_ms: u64,
//     prev_values: &[Coil],
//     new_values: &[Coil],
// ) {
//     for (index, (prev, new)) in prev_values.iter().zip(new_values.iter()).enumerate() {
//         if *prev != *new {
//             let state = *new == Coil::On;
//             let plc_addr = format!("%M{index}");
//             db.execute(
//                 "INSERT INTO event (utc_ms, address, state)
//                 VALUES (?1, ?2, ?3)",
//                 (utc_ms, plc_addr, state),
//             )
//             .unwrap();
//         }
//     }
// }

// pub fn store_holding_events(
//     db: &rusqlite::Connection,
//     utc_ms: u64,
//     prev_values: &[u16],
//     new_values: &[u16],
// ) {
//     for (index, (prev, new)) in prev_values.iter().zip(new_values.iter()).enumerate() {
//         if *prev != *new {
//             let state = *new;
//             let plc_addr = format!("%MW{index}");
//             db.execute(
//                 "INSERT INTO event (utc_ms, address, state)
//                 VALUES (?1, ?2, ?3)",
//                 (utc_ms, plc_addr, state),
//             )
//             .unwrap();
//         }
//     }
// }

pub fn detect_coil_events(
    events: &mut Vec<Event>,
    utc_ms: u64,
    prev_values: &[Coil],
    new_values: &[Coil],
) {
    for (index, (prev, new)) in prev_values.iter().zip(new_values.iter()).enumerate() {
        if *prev != *new {
            let event = Event {
                utc_ms,
                coil: true,
                address: index as u16,
                state: match new {
                    Coil::On => 1,
                    Coil::Off => 0,
                },
            };

            events.push(event);
        }
    }
}

pub fn detect_holding_events(
    events: &mut Vec<Event>,
    utc_ms: u64,
    prev_values: &[u16],
    new_values: &[u16],
) {
    for (index, (prev, new)) in prev_values.iter().zip(new_values.iter()).enumerate() {
        if *prev != *new {
            let event = Event {
                utc_ms,
                coil: false,
                address: index as u16,
                state: *new,
            };

            events.push(event);
        }
    }
}

pub struct Event {
    utc_ms: u64,
    coil: bool,
    address: u16,
    state: u16,
}

pub fn store_events(
    db: &rusqlite::Connection,
    channel_receiver: std::sync::mpsc::Receiver<Vec<Event>>,
) -> Result<(), Box<dyn std::error::Error>> {
    while let Ok(events) = channel_receiver.recv() {
        let mut insert_event =
            db.prepare("INSERT INTO event (utc_ms, address, state) VALUES (?1, ?2, ?3)")?;
        let transaction = db.unchecked_transaction().unwrap();
        for event in events {
            let address = format!("{}{}", if event.coil { "%M" } else { "%MW" }, event.address);
            insert_event.execute((event.utc_ms, address, event.state))?;
        }
        transaction.commit()?;
    }
    Ok(())
}
