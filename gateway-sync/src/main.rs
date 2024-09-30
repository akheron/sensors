use clap::Parser;
use regex::bytes::{Captures, Regex};
use std::fmt::Debug;
use std::io::{BufRead, BufReader, Write};
use std::str;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

const SENSOR_COUNT: usize = 4;

fn main() {
    let options = CmdlineOptions::parse();

    let re = Regex::new(r"T:(?<t>\d+(\.\d+)?)/H:(?<h>\d+(\.\d+)?)\s|EEE\s")
        .expect("Unable to compile regex");

    let mut port = serialport::new(options.serial_port, options.baud_rate)
        .timeout(Duration::from_secs(5))
        .open_native()
        .expect("Unable to open serial port");

    let mut retries: u64 = 0;
    let readings = loop {
        if retries >= 5 {
            eprintln!("Unable to read data from sensor after {} retries", retries);
            return;
        }

        // Write something to trigger the sensor
        port.write_all("x".as_bytes())
            .expect("Unable to write to serial port");

        let mut buf = Vec::new();
        let read_result = BufReader::new(&mut port).read_until(b'\n', &mut buf);
        if read_result.is_err() {
            eprintln!("Sensor did not send data");
            thread::sleep(Duration::from_secs(1));
            retries += 1;
            continue;
        }

        let readings = re
            .captures_iter(&buf)
            .map(|caps| {
                if let (Some(t), Some(h)) = (
                    parse_capture::<f64>(&caps, "t"),
                    parse_capture::<f64>(&caps, "h"),
                ) {
                    Some((t, h))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if readings.len() != SENSOR_COUNT {
            eprintln!("Sensor output malformed: {:?}", str::from_utf8(&buf));
            thread::sleep(Duration::from_secs(1));
            retries += 1;
        } else {
            break readings;
        }
    };

    let client = Client {
        client: reqwest::blocking::Client::new(),
        url: options.server_endpoint,
        username: options.username,
        password: options.password,
    };

    let sensors = [
        options.sensor1,
        options.sensor2,
        options.sensor3,
        options.sensor4,
    ];

    println!("Readings:");
    for (i, (reading, sensor)) in readings.iter().zip(sensors.iter()).enumerate() {
        if let Some((t, h)) = reading {
            println!("{}. temperature: {} °C, humidity: {} %", i + 1, t, h);
            if let Some(sensor) = sensor {
                print!("   ");
                client.send(sensor, *t, *h);
            }
        } else {
            println!("- no data");
        }
    }
}

struct Client {
    client: reqwest::blocking::Client,
    url: String,
    username: String,
    password: String,
}

impl Client {
    fn send(&self, sensor: &str, t: f64, h: f64) {
        let mut retries = 0;
        loop {
            if retries >= 5 {
                eprintln!("Unable to send data to server after {} retries", retries);
                return;
            }
            let now = chrono::Utc::now();

            let result = self.client
                .post(&self.url)
                .basic_auth(&self.username, Some(&self.password))
                .json(&serde_json::json!({ "sensor": sensor, "timestamp": now, "temperature": t, "humidity": h }))
                .send();
            if let Ok(response) = result {
                println!(
                    "Response: {} {}",
                    response.status().as_u16(),
                    response
                        .text()
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or("(invalid utf-8)")
                );
                break;
            } else {
                eprintln!("Failed to send data to server: {:?}", result);
            }
            thread::sleep(Duration::from_secs(retries));
            retries += 1;
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CmdlineOptions {
    /// Serial port to use
    #[arg(short, long)]
    serial_port: String,

    /// Baud rate for the serial port
    #[arg(short, long)]
    baud_rate: u32,

    /// Server URL to send data to
    #[arg(short, long)]
    server_endpoint: String,

    /// HTTP Basic auth username
    #[arg(short, long)]
    username: String,

    /// HTTP Basic auth password
    #[arg(short, long)]
    password: String,

    /// Sensor ID 1
    #[arg(short, long)]
    sensor1: Option<String>,

    /// Sensor ID 2
    #[arg(short, long)]
    sensor2: Option<String>,

    /// Sensor ID 3
    #[arg(short, long)]
    sensor3: Option<String>,

    /// Sensor ID 4
    #[arg(short, long)]
    sensor4: Option<String>,
}

fn parse_capture<T>(captures: &Captures, name: &str) -> Option<T>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    let capture = captures.name(name)?;
    let text = str::from_utf8(capture.as_bytes()).ok()?;
    text.parse::<T>().ok()
}
