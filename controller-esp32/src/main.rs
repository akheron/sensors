#![no_std]
#![no_main]

use dht_embedded::{Dht22, DhtSensor, NoopInterruptControl};
use embedded_io::Read;
use esp_backtrace as _;
use esp_hal::gpio::{Io, Level, Output, OutputOpenDrain, Pull};
use esp_hal::{
    clock::ClockControl, delay::Delay, peripherals::Peripherals, prelude::*, system::SystemControl,
    uart,
};

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();
    let mut delay = Delay::new(&clocks);
    esp_println::logger::init_logger_from_env();

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut data = OutputOpenDrain::new(io.pins.gpio4, Level::High, Pull::Up);

    // Shut down the blue LED
    let mut led = Output::new(io.pins.gpio8, Level::High);

    let mut uart1 = uart::Uart::new_with_config(
        peripherals.UART1,
        uart::config::Config {
            baudrate: 9600,
            data_bits: uart::config::DataBits::DataBits8,
            parity: uart::config::Parity::ParityNone,
            stop_bits: uart::config::StopBits::STOP1,
            ..Default::default()
        },
        &clocks,
        io.pins.gpio1,
        io.pins.gpio2,
    )
    .unwrap();

    log::info!("Start delay...");
    delay.delay(2000.millis());
    loop {
        log::info!("Waiting for data in uart");
        {
            let mut bytes = [0u8; 32];
            loop {
                match uart1.read(&mut bytes) {
                    Ok(n) if n > 0 => break,
                    _ => delay.delay(10.millis()),
                }
            }
        };

        log::info!("Reading sensor");
        let mut dht = Dht22::new(NoopInterruptControl, &mut delay, &mut data);
        match dht.read() {
            Ok(reading) => {
                log::info!("Temperature: {}°C", reading.temperature());
                log::info!("Humidity: {}%", reading.humidity());
                let mut buffer = [0; 32];
                let s = format_no_std::show(
                    &mut buffer,
                    format_args!("T:{} H:{}\r\n", reading.temperature(), reading.humidity()),
                )
                .unwrap();
                uart1.write_bytes(s.as_bytes()).unwrap();
            }
            Err(e) => {
                log::error!("Reading error: {:?}", e);
                uart1.write_bytes(b"Error reading sensor\r\n").unwrap();
            }
        }

        for _ in 0..20 {
            led.toggle();
            delay.delay(50.millis());
        }
    }
}
