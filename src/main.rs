mod config;
mod meter;
mod mqtt;
mod probe;
mod protocol;

use anyhow::Result;
use clap::Parser;
use log::info;
use std::time::Duration;

fn main() -> Result<()> {
    env_logger::init();

    let config = config::Config::parse();
    info!("Starting energymon");

    let reading = match &config.port {
        Some(path) => {
            info!("Using specified port: {}", path);
            protocol::read_meter(path, &config.device_id, Duration::from_secs(config.timeout_secs))?
        }
        None => {
            info!("No port specified, probing for {} ...", config.device_id);
            let result = probe::find_meter_port(&config.device_id)?;
            protocol::read_meter_probed(result.port, &result.device_id)?
        }
    };

    // Publish to MQTT
    mqtt::publish_reading(&config, &reading)?;

    info!("Done");
    Ok(())
}
