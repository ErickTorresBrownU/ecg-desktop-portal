// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use chrono::Local;
use serialport::SerialPort;
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, serde::Serialize)]
struct EcgReading {
    milliseconds: i64,
    value: f64,
}

fn setup_csv_file() -> PathBuf {
    if !matches!(Path::new("records").try_exists(), Ok(true)) {
        let _ = fs::create_dir("records");
    }

    let now = chrono::Local::now().date_naive().to_string();

    let mut file_path = Path::new("records").join(format!("{}.csv", now));

    let mut num = 1;
    while matches!(file_path.try_exists(), Ok(true)) {
        file_path = Path::new("records").join(format!("{} ({}).csv", now, num));
        num += 1;
    }

    File::create(&file_path).unwrap();

    file_path
}

fn parse_serial_entry(serial_entry: &str) -> Result<EcgReading, ()> {
    if serial_entry.is_empty() {
        return Err(());
    }

    if !(serial_entry.starts_with("(") && serial_entry.ends_with(")")) {
        return Err(());
    }

    let mut split = serial_entry[1..(serial_entry.len() - 1)].splitn(2, " ");

    let milliseconds = match split.next() {
        Some(ms_str) => ms_str.parse::<i64>().map_or(Err(()), |value| Ok(value)),
        None => return Err(()),
    }?;

    let reading_value = match split.next() {
        Some(value_str) => value_str.parse::<f64>().map_or(Err(()), |value| Ok(value)),
        None => return Err(()),
    }?;

    Ok(EcgReading {
        milliseconds,
        value: reading_value,
    })
}

fn main_backend(app_handle: AppHandle) {
    thread::spawn(move || {
        let file_path = setup_csv_file();

        // TODO: Add functionality such that when USB is unplugged, csv buffer is closed, a new file is opened, and writing starts with new "connection"
        let mut csv_writer = csv::Writer::from_path(&file_path).unwrap();

        let mut time_offsets: Option<(i64, i64)> = None;

        let mut serial_port: Option<Box<dyn SerialPort>> = None;
        let mut time_of_last_ok = Instant::now();

        const MAX_TIME_WITHOUT_VERIFICATION_MILLIS: u64 = 1000;
        const VERIFICATION_INTERVAL_MILLIS: u64 = MAX_TIME_WITHOUT_VERIFICATION_MILLIS / 2;

        'outer: loop {
            let ports = serialport::available_ports().unwrap();

            if ports.len() == 0 {
                continue;
            }

            if serial_port.is_none() {
                let port_builder = serialport::new(&ports.get(0).unwrap().port_name, 57600)
                    .timeout(Duration::from_secs(3));

                let opened = port_builder.clone().open();

                match opened {
                    Ok(port) => serial_port = Some(port),
                    Err(_) => {
                        serial_port = None;
                        continue;
                    }
                };
            }

            if time_of_last_ok.elapsed().as_millis() >= VERIFICATION_INTERVAL_MILLIS.into() {
                time_of_last_ok = Instant::now();

                let ok_state = serial_port.as_mut().unwrap().write("OK\n".as_bytes());

                if ok_state.is_err() {
                    dbg!("Couldn't Write");

                    serial_port = None;
                    continue;
                }
            }

            let mut string_buffer = String::new();

            let mut one_byte_buffer = [0; 1];

            loop {
                if serial_port
                    .as_mut()
                    .unwrap()
                    .read_exact(&mut one_byte_buffer)
                    .is_err()
                {
                    dbg!("Failed to read into buffer");
                    serial_port = None;
                    continue 'outer;
                }

                let read_chacter = match std::str::from_utf8(&one_byte_buffer) {
                    Ok(character) => character,
                    Err(_) => continue,
                };

                string_buffer.push_str(read_chacter);

                if read_chacter == "\n" {
                    break;
                }
            }

            let value_reading = string_buffer.trim();

            let parsed_entry = parse_serial_entry(value_reading);

            if let Ok(non_offset_reading) = parsed_entry {
                if time_offsets.is_none() {
                    time_offsets = Some((
                        non_offset_reading.milliseconds,
                        chrono::Local::now().timestamp_millis(),
                    ));
                }

                let time_offsets = time_offsets.unwrap();

                let offset_reading = EcgReading {
                    milliseconds: non_offset_reading.milliseconds - time_offsets.0 + time_offsets.1,
                    value: non_offset_reading.value,
                };

                let test = EcgReading {
                    milliseconds: Local::now().timestamp_millis(),
                    value: non_offset_reading.value,
                };

                app_handle.emit_all("new-reading", test).unwrap();

                use chrono::{DateTime, NaiveDateTime};

                let date: DateTime<Local> = DateTime::from_naive_utc_and_offset(
                    NaiveDateTime::from_timestamp_millis(offset_reading.milliseconds).unwrap(),
                    *Local::now().offset(),
                );

                csv_writer
                    .write_record(&[date.to_rfc3339(), offset_reading.value.to_string()])
                    .unwrap();
            } else {
                app_handle
                    .emit_all(
                        "new-reading",
                        EcgReading {
                            milliseconds: Local::now().timestamp_millis(),
                            value: 0.,
                        },
                    )
                    .unwrap();
            }
        }
    });
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            thread::spawn(move || main_backend(app_handle));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
