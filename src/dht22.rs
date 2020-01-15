use common_failures::prelude::*;

use super::gpio::Gpio;
use super::gpio::PinDirection::In;
use super::gpio::PinDirection::Out;
use super::gpio::PinValue::High;
use super::gpio::PinValue::Low;
use std::thread;
use std::time::Duration;

const DHT_PULSES: usize = 41;
const DHT_MAXCOUNT: usize = 128000;

pub struct DhtData {
    pub humidity: f32,
    pub temperature: f32,
}

pub enum DhtResult {
    Data(DhtData),
    Timeout,
    ChecksumError,
}

pub struct Dht22<'a> {
    gpio: Gpio<'a>,
    pin: usize,
}

impl<'a> Dht22<'a> {
    pub fn open(pin: usize) -> Result<Dht22<'a>> {
        let gpio = Gpio::open()?;
        Ok(Dht22 { gpio, pin })
    }

    pub fn read_data(&mut self) -> DhtResult {
        let gpio = &mut self.gpio;
        let pin = self.pin;

        gpio.set_pin_direction(pin, Out);
        gpio.set_pin_value(pin, High);
        thread::sleep(Duration::from_millis(500));
        gpio.set_pin_value(pin, Low);
        thread::sleep(Duration::from_millis(20));
        gpio.set_pin_direction(pin, In);
        for _ in 0..50 {}
        let mut count = 0;
        while gpio.get_pin_value(pin) == High {
            count += 1;
            if count > DHT_MAXCOUNT {
                return DhtResult::Timeout;
            }
        }
        let mut pulse_counts = [0usize; DHT_PULSES * 2];
        for i in (0..(DHT_PULSES * 2)).step_by(2) {
            let mut count = 0;
            while gpio.get_pin_value(pin) == Low {
                count += 1;
                if count > DHT_MAXCOUNT {
                    return DhtResult::Timeout;
                }
            }
            pulse_counts[i] = count;
            let mut count = 0;
            while gpio.get_pin_value(pin) == High {
                count += 1;
                if count > DHT_MAXCOUNT {
                    return DhtResult::Timeout;
                }
            }
            pulse_counts[i + 1] = count;
        }
        let mut threshold = 0;
        for i in (2..(DHT_PULSES * 2)).step_by(2) {
            threshold += pulse_counts[i];
        }
        threshold /= DHT_PULSES - 1;
        let mut data = [0u8; 5];
        for i in (3..(DHT_PULSES * 2)).step_by(2) {
            let index = (i - 3) / 16;
            data[index] <<= 1;
            if pulse_counts[i] >= threshold {
                data[index] |= 1;
            }
        }
        let checksum_actual =
            (data[0] as u32 + data[1] as u32 + data[2] as u32 + data[3] as u32) as u8 & 0xFF;
        if checksum_actual != data[4] {
            return DhtResult::ChecksumError;
        }
        let humidity = (data[0] as u16 * 256 + data[1] as u16) as f32 / 10.0;
        let mut temperature = ((data[2] & 0x7F) as u16 * 256 + data[3] as u16) as f32 / 10.0;
        if data[2] & 0x80 == 1 {
            temperature *= -1.0;
        }
        DhtResult::Data(DhtData {
            humidity,
            temperature,
        })
    }
}
