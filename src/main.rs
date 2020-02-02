#[macro_use]
extern crate common_failures;
extern crate caps;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;
extern crate redis;
extern crate thread_priority;

mod dht22;
mod gpio;

use common_failures::prelude::*;

use caps::CapSet;
use caps::Capability;
use clap::App;
use clap::Arg;
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

const ARG_REDIS_HOST: &str = "redis-host";
const ARG_REDIS_PORT: &str = "redis-port";
const ARG_NAME: &str = "name";
const ARG_GPIO_PIN: &str = "gpio-pin";
const ARG_VERBOSITY: &str = "verbosity";
const ARG_QUIET: &str = "quiet";

quick_main!(run);

fn run() -> Result<()> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name(ARG_VERBOSITY)
                .long(ARG_VERBOSITY)
                .short("v")
                .multiple(true)
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name(ARG_QUIET)
                .long(ARG_QUIET)
                .short("q")
                .multiple(false)
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name(ARG_REDIS_HOST)
                .long(ARG_REDIS_HOST)
                .multiple(false)
                .takes_value(true)
                .required(false)
                .default_value("localhost"),
        )
        .arg(
            Arg::with_name(ARG_REDIS_PORT)
                .long(ARG_REDIS_PORT)
                .multiple(false)
                .takes_value(true)
                .required(false)
                .default_value("6379"),
        )
        .arg(
            Arg::with_name(ARG_NAME)
                .long(ARG_NAME)
                .multiple(false)
                .takes_value(true)
                .required(false)
                .default_value("dht22"),
        )
        .arg(
            Arg::with_name(ARG_GPIO_PIN)
                .long(ARG_GPIO_PIN)
                .multiple(false)
                .takes_value(true)
                .required(false)
                .default_value("4"),
        )
        .get_matches();

    let verbosity = args.occurrences_of(ARG_VERBOSITY) as usize + 1;
    let quiet = args.is_present(ARG_QUIET);

    stderrlog::new()
        .module(module_path!())
        .timestamp(stderrlog::Timestamp::Second)
        .verbosity(verbosity)
        .quiet(quiet)
        .init()?;

    let redis_host = value_t!(args, ARG_REDIS_HOST, String)?;
    let redis_port = value_t!(args, ARG_REDIS_PORT, usize)?;
    let name = value_t!(args, ARG_NAME, String)?;
    let gpio_pin = value_t!(args, ARG_GPIO_PIN, usize)?;

    // requires:
    // sudo setcap cap_sys_nice=ep <file>
    try_upgrade_thread_priority()?;

    let mut redis =
        redis::Client::open(format!("redis://{}:{}", redis_host, redis_port))?.get_connection()?;

    let mut dht22 = Dht22::open(gpio_pin)?;
    loop {
        match dht22.read_data() {
            DhtResult::Data(data) => {
                info!(
                    "Temperature {}°C, Humidity {}%",
                    data.temperature, data.humidity
                );
                redis.publish(format!("{}/temperature", name), data.temperature)?;
                redis.publish(format!("{}/humidity", name), data.humidity)?;
            }
            DhtResult::Timeout => debug!("Timeout"),
            DhtResult::ChecksumError => debug!("ChecksumError"),
        }
        thread::sleep(Duration::from_secs(3));
    }
}

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
