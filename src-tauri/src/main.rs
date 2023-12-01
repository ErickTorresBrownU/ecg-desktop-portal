// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use chrono::{Local, Offset, Utc};
use serialport::SerialPort;
use tauri::{AppHandle, Manager};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

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

    // When user plugs in arduino
    File::create(&file_path).unwrap();

    file_path
}

fn parse_serial_entry(serial_entry: &str) -> Result<EcgReading, ()> {
    // dbg!(&serial_entry);
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
        // let mut reader =
        //     csv::Reader::from_path("C:\\Users\\erick\\Downloads\\samples (4).csv").unwrap();

        // let values: Vec<f64> = reader
        //     .records()
        //     .skip(1)
        //     .map(|record| record.unwrap())
        //     .map(|record| record.get(2).unwrap().to_owned().parse::<f64>().unwrap())
        //     .collect();

        // let mut idx = 0;

        let mut time_offsets: Option<(i64, i64)> = None;

        let mut serial_port: Option<Box<dyn SerialPort>> = None;
        let mut rizz: Option<BufReader<Box<dyn SerialPort>>> = None;
        let mut time_of_last_ok = Instant::now();

        const MAX_TIME_WITHOUT_VERIFICATION_MILLIS: u64 = 1000;
        const VERIFICATION_INTERVAL_MILIS: u64 = MAX_TIME_WITHOUT_VERIFICATION_MILLIS / 2;

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
                        rizz = None;
                        continue;
                    }
                };

                // serial_reader = Some(BufReader::new(port_builder.open().unwrap()));
            }

            if time_of_last_ok.elapsed().as_millis() >= VERIFICATION_INTERVAL_MILIS.into() {
                // dbg!("SENDING OK MESSAGE");
                // dbg!(time_of_last_ok.elapsed().as_millis());
                time_of_last_ok = Instant::now();

                let ok_state = serial_port.as_mut().unwrap().write("OK\n".as_bytes());

                if ok_state.is_err() {
                    dbg!("Couldn't Write");

                    serial_port = None;
                    rizz = None;
                    continue;
                }
            }

            let mut string_buffer = String::new();

            let mut b = [0; 1];

            loop {
                // if serial_port.as_mut().unwrap().bytes_to_read().unwrap() == 0 {
                //     dbg!("no bytes to read");
                //     continue;
                // }

                if serial_port.as_mut().unwrap().read_exact(&mut b).is_err() {
                    dbg!("Failed to read into buffer");
                    serial_port = None;
                    continue 'outer;
                }

                let read_chacter = match std::str::from_utf8(&b) {
                    Ok(character) => character,
                    Err(_) => continue,
                };

                string_buffer.push_str(read_chacter);

                if read_chacter == "\n" {
                    break;
                }
            }

            // dbg!(&string_buffer);

            // if rizz.is_none() {
            //     let cloned = serial_port.as_mut().unwrap().try_clone().unwrap();

            //     rizz = Some(BufReader::new(cloned));
            // }

            // let mut beef = String::new();
            // rizz.as_mut().unwrap().read_line(&mut beef);

            // dbg!(beef);

            // let mut string_buffer = String::new();

            // let mut b = [0; 1];

            // loop {
            //     let read_value = serial_port.as_mut().read_exact(&mut b);

            //     let read_chacter = match std::str::from_utf8(&b) {
            //         Ok(character) => character,
            //         Err(_) => continue,
            //     };

            //     string_buffer.push_str(read_chacter);

            //     if read_chacter == "\n" {
            //         break;
            //     }
            // }

            // dbg!(string_buffer);

            // continue;

            // let then = Instant::now();

            let value_reading = string_buffer.trim();

            dbg!(&value_reading);

            let parsed_entry = parse_serial_entry(value_reading);

            if let Ok(non_offset_reading) = parsed_entry {
                dbg!(&non_offset_reading);

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

                // dbg!(then.elapsed().as_millis());
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

            // let then = Instant::now();
            // if idx >= values.len() {
            //     return;
            // }

            // let now = chrono::Local::now();
            // let value = *values.get(idx).unwrap();

            // app_handle
            //     .emit_all(
            //         "new-reading",
            //         EcgReading {
            //         milliseconds: now.timestamp_millis(),
            //         value: /* rnd.gen_range(-3.0..1.) */ value,
            //     },
            //     )
            //     .unwrap();

            // idx += 1;
            // writer
            //     .write_record(&[now.to_rfc3339(), value.to_string()])
            //     .unwrap();
            // // writer.flush();
            // dbg!(then.elapsed().as_millis());

            // thread::sleep(std::time::Duration::from_micros(100));
        }
    });
}

fn main() {
    // let v: Vec<_> = read_dir(".").unwrap().map(|dir| dir.map(|e| e.path())).collect();
    // dbg!(v);

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            // TODO
            tauri::async_runtime::spawn(async move {});
            thread::spawn(move || main_backend(app_handle));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
