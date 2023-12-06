use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::{DateTime, Local, NaiveDateTime};
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

fn read_line_from_serial(serial_port: &mut Option<Box<dyn SerialPort>>) -> Result<String, ()> {
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

            return Err(());
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

    return Ok(string_buffer);
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

macro_rules! nullify_and_skip {
    ($value: expr) => {
        $value = None;
        continue;
    };
}

pub fn main_loop(app_handle: AppHandle) {
    let mut serial_port: Option<Box<dyn SerialPort>> = None;

    let mut time_of_last_ok = Instant::now();

    let mut csv_writer: Option<csv::Writer<File>> = None;
    let mut csv_writer_been_flushed = false;

    // time_offsets.0 => Milliseconds since Arduino initialized from first valid reading for this session
    // time_offsets.1 => Timestamp milliseconds when first valid reading was parsed for this session
    let mut time_offsets: Option<(i64, i64)> = None;

    const MAX_TIME_WITHOUT_VERIFICATION_MILLIS: u64 = 1000;
    const VERIFICATION_INTERVAL_MILLIS: u64 = MAX_TIME_WITHOUT_VERIFICATION_MILLIS / 2;

    loop {
        if serial_port.is_none() {
            if let Some(ref mut csv_writer) = csv_writer {
                if !csv_writer_been_flushed {
                    // TODO: handle this result
                    dbg!("Flushing");
                    let _ = csv_writer.flush();

                    csv_writer_been_flushed = true;
                }
            }

            let ports = serialport::available_ports().unwrap();

            if ports.len() == 0 {
                continue;
            }

            let port_config = serialport::new(&ports.get(0).unwrap().port_name, 57600)
                .timeout(Duration::from_secs(3));

            match port_config.open() {
                Ok(port) => {
                    serial_port = Some(port);

                    csv_writer = Some(csv::Writer::from_path(setup_csv_file()).unwrap());
                    csv_writer_been_flushed = false;

                    time_offsets = None;

                    app_handle.emit_all("reset-monitor", ()).unwrap();
                }
                Err(_) => {
                    nullify_and_skip!(serial_port);
                }
            };
        }

        if time_of_last_ok.elapsed().as_millis() >= VERIFICATION_INTERVAL_MILLIS.into() {
            time_of_last_ok = Instant::now();

            let ok_send_state = serial_port.as_mut().unwrap().write("OK\n".as_bytes());

            if ok_send_state.is_err() {
                dbg!("Couldn't Write");

                nullify_and_skip!(serial_port);
            }
        }

        let line = if let Ok(line) = read_line_from_serial(&mut serial_port) {
            line
        } else {
            nullify_and_skip!(serial_port);
        };

        let parsed_entry = parse_serial_entry(line.trim());

        if parsed_entry.is_err() {
            app_handle
                .emit_all(
                    "new-reading",
                    EcgReading {
                        milliseconds: Local::now().timestamp_millis(),
                        value: 0.,
                    },
                )
                .unwrap();

            continue;
        }

        if let Ok(non_offset_reading) = parsed_entry {
            // Store the time offsets for the first valid reading encountered
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

            app_handle
                .emit_all("new-reading", non_offset_reading)
                .unwrap();

            let date: DateTime<Local> = DateTime::from_naive_utc_and_offset(
                NaiveDateTime::from_timestamp_millis(offset_reading.milliseconds).unwrap(),
                *Local::now().offset(),
            );

            csv_writer
                .as_mut()
                .unwrap()
                .write_record(&[date.to_rfc3339(), offset_reading.value.to_string()])
                .unwrap();
        }
    }
}
