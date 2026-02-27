mod config;
mod meter;
mod mqtt;
mod probe;
mod protocol;

use anyhow::Result;
use clap::Parser;
use log::{error, info};
use std::time::Duration;

fn main() -> Result<()> {
    env_logger::init();

    let config = config::Config::parse();
    info!("Starting energymon");

    let mut conn = match &config.port {
        Some(path) => {
            info!("Using specified port: {}", path);
            protocol::MeterConnection::open(
                path,
                &config.device_id,
                Duration::from_secs(config.timeout_secs),
            )?
        }
        None => {
            info!("No port specified, probing for {} ...", config.device_id);
            let result = probe::find_meter_port(&config.device_id)?;
            protocol::MeterConnection::from_probe(result.port, &result.device_id)
        }
    };

    loop {
        match conn.read() {
            Ok(reading) => {
                if let Err(e) = mqtt::publish_reading(&config, &reading) {
                    error!("Failed to publish: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to read meter: {}", e);
            }
        }
    }
}
