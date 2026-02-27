use anyhow::{Context, Result};
use log::info;
use rumqttc::{Client, MqttOptions, QoS};
use std::time::Duration;

use crate::config::Config;
use crate::meter::MeterReading;

/// Publish a meter reading as JSON to the configured MQTT broker.
/// Uses QoS 0 (fire-and-forget), matching the Python script's behavior.
pub fn publish_reading(config: &Config, reading: &MeterReading) -> Result<()> {
    let payload = serde_json::to_string(reading).context("Failed to serialize reading to JSON")?;

    let mut opts = MqttOptions::new(&config.mqtt_client_id, &config.mqtt_host, config.mqtt_port);
    opts.set_keep_alive(Duration::from_secs(60));

    let (client, mut connection) = Client::new(opts, 10);

    client
        .publish(
            &config.mqtt_topic,
            QoS::AtMostOnce,
            false,
            payload.as_bytes(),
        )
        .context("Failed to queue MQTT publish")?;

    // rumqttc requires driving the event loop to actually send the packet
    for event in connection.iter() {
        match event {
            Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Publish(_))) => {
                info!(
                    "Published to {} on {}:{}",
                    config.mqtt_topic, config.mqtt_host, config.mqtt_port
                );
                break;
            }
            Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => break,
            Err(e) => return Err(anyhow::anyhow!("MQTT connection error: {}", e)),
            _ => continue,
        }
    }

    client.disconnect().ok();
    Ok(())
}
