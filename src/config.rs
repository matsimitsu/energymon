use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "energymon",
    about = "ISk5MT174 electricity meter reader via IR optical head"
)]
pub struct Config {
    /// MQTT broker hostname
    #[arg(long, default_value = "127.0.0.1")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(long, default_value_t = 1883)]
    pub mqtt_port: u16,

    /// MQTT client ID
    #[arg(long, default_value = "ISK5MT174-DATA")]
    pub mqtt_client_id: String,

    /// MQTT topic to publish to
    #[arg(long, default_value = "tele/ISK5MT174")]
    pub mqtt_topic: String,

    /// Device identifier substring to match in meter response
    #[arg(long, default_value = "ISk5MT174")]
    pub device_id: String,

    /// Serial port path (if omitted, probes all /dev/ttyUSB* ports)
    #[arg(long)]
    pub port: Option<String>,

    /// Serial read timeout in seconds
    #[arg(long, default_value_t = 10)]
    pub timeout_secs: u64,
}
