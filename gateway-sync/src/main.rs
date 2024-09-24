use clap::Parser;
use regex::bytes::{Captures, Regex};
use std::fmt::Debug;
use std::io::{BufRead, BufReader, Write};
use std::str;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

fn main() {
    let options = CmdlineOptions::parse();

    let re =
        Regex::new(r"^T:(?<t>\d+(\.\d+)?) H:(?<h>\d+(\.\d)+)").expect("Unable to compile regex");

    let mut port = serialport::new(options.serial_port, options.baud_rate)
        .timeout(Duration::from_secs(5))
        .open_native()
        .expect("Unable to open serial port");

    let mut retries: u64 = 0;
    let (t, h) = loop {
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

        match re.captures(&buf) {
            Some(caps) => {
                // capture to double
                if let (Some(t), Some(h)) = (
                    parse_capture::<f64>(&caps, "t"),
                    parse_capture::<f64>(&caps, "h"),
                ) {
                    break (t, h);
                }
            }
            None => {
                eprintln!("Sensor output malformed: {:?}", str::from_utf8(&buf));
                thread::sleep(Duration::from_secs(1));
                retries += 1;
            }
        };
    };

    println!("Temperature: {} °C", t);
    println!("Humidity: {} %", h);

    let client = reqwest::blocking::Client::new();
    retries = 0;
    loop {
        if retries >= 5 {
            eprintln!("Unable to send data to server after {} retries", retries);
            return;
        }
        let now = chrono::Utc::now();

        let result = client
            .post(&options.server_endpoint)
            .basic_auth(&options.username, Some(&options.password))
            .json(&serde_json::json!({ "sensor": options.sensor, "timestamp": now, "temperature": t, "humidity": h }))
            .send();
        if result.is_ok() {
            break;
        } else {
            eprintln!("Failed to send data to server: {:?}", result);
        }
        thread::sleep(Duration::from_secs(retries));
        retries += 1;
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

    /// Sensor ID
    #[arg(short, long)]
    sensor: String,
}

fn parse_capture<T>(captures: &Captures, name: &str) -> Option<T>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    let capture = captures.name(name).unwrap();
    let text = str::from_utf8(capture.as_bytes()).ok()?;
    text.parse::<T>().ok()
}
