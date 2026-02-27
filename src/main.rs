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

    // Determine which serial port to use
    let port_path = match &config.port {
        Some(path) => {
            info!("Using specified port: {}", path);
            path.clone()
        }
        None => {
            info!("No port specified, probing for {} ...", config.device_id);
            probe::find_meter_port(&config.device_id)?
        }
    };

    // Read the meter
    let reading = protocol::read_meter(
        &port_path,
        &config.device_id,
        Duration::from_secs(config.timeout_secs),
    )?;

    // Publish to MQTT
    mqtt::publish_reading(&config, &reading)?;

    info!("Done");
    Ok(())
}
