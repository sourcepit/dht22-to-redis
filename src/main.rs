#[macro_use]
extern crate common_failures;
extern crate caps;
#[macro_use]
extern crate failure;
extern crate libc;
extern crate redis;
extern crate thread_priority;

mod dht22;
mod gpio;

use common_failures::prelude::*;

use caps::CapSet;
use caps::Capability;
use dht22::Dht22;
use dht22::DhtResult;
use redis::Commands;
use std::thread;
use std::time::Duration;
use thread_priority::set_thread_priority;
use thread_priority::thread_native_id;
use thread_priority::RealtimeThreadSchedulePolicy;
use thread_priority::ThreadPriority;
use thread_priority::ThreadSchedulePolicy;

fn try_upgrade_thread_priority() -> Result<()> {
    let has_cap_sys_nice = match caps::has_cap(None, CapSet::Permitted, Capability::CAP_SYS_NICE) {
        Ok(v) => v,
        Err(e) => return Err(format_err!("{}", e)),
    };
    if has_cap_sys_nice {
        let thread_id = thread_native_id();
        let res = set_thread_priority(
            thread_id,
            ThreadPriority::Max,
            ThreadSchedulePolicy::Realtime(RealtimeThreadSchedulePolicy::Fifo),
        );
        match res {
            Ok(_) => {}
            Err(e) => return Err(format_err!("{:?}", e)),
        }
    };
    Ok(())
}

fn run() -> Result<()> {
    // requires:
    // sudo setcap cap_sys_nice=ep <file>
    try_upgrade_thread_priority()?;

    let mut redis = redis::Client::open("redis://127.0.0.1/")?.get_connection()?;

    let pin = 4;

    let mut dht22 = Dht22::open(pin)?;
    loop {
        match dht22.read_data() {
            DhtResult::Data(data) => {
                println!(
                    "Temperature {}°C, Humidity {}%",
                    data.temperature, data.humidity
                );
                redis.publish("dht22/temperature", data.temperature)?;
                redis.publish("dht22/humidity", data.humidity)?;
            }
            DhtResult::Timeout => println!("Timeout"),
            DhtResult::ChecksumError => println!("ChecksumError"),
        }
        thread::sleep(Duration::from_secs(3));
    }
}

quick_main!(run);
