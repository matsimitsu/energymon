use anyhow::{bail, Context, Result};
use log::{debug, info, warn};
use std::io::{BufRead, BufReader};
use std::time::Duration;

pub const IEC_INIT_SEQUENCE: &[u8] = b"\x2F\x3F\x21\x0D\x0A"; // /?!\r\n
pub const BAUD_RATE: u32 = 300;
pub const DATA_BITS: serialport::DataBits = serialport::DataBits::Seven;
pub const PARITY: serialport::Parity = serialport::Parity::Even;
pub const STOP_BITS: serialport::StopBits = serialport::StopBits::One;

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Open a serial port with IEC 62056-21 settings.
/// Sets DTR and RTS high to match pyserial defaults — the Weidmann IR head
/// uses DTR to power its IR LED.
pub fn open_port(path: &str, timeout: Duration) -> Result<Box<dyn serialport::SerialPort>> {
    let mut port = serialport::new(path, BAUD_RATE)
        .data_bits(DATA_BITS)
        .parity(PARITY)
        .stop_bits(STOP_BITS)
        .timeout(timeout)
        .open()
        .with_context(|| format!("Failed to open serial port {}", path))?;

    port.write_data_terminal_ready(true)
        .context("Failed to set DTR")?;
    port.write_request_to_send(true)
        .context("Failed to set RTS")?;

    Ok(port)
}

/// Send the IEC 62056-21 init sequence and wait for the meter to wake up.
pub fn send_init(port: &mut dyn serialport::SerialPort) -> Result<()> {
    port.write_all(IEC_INIT_SEQUENCE)?;
    port.flush()?;
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

/// Result of a successful probe: the open port and the device ID that was read.
pub struct ProbeResult {
    pub port: Box<dyn serialport::SerialPort>,
    pub device_id: String,
}

/// Probe a single port: send init sequence, check if first response line
/// contains the expected device identifier. Returns the open port on match
/// so the caller can continue reading the telegram.
fn probe_port(path: &str, device_id: &str) -> Result<Option<ProbeResult>> {
    debug!("Probing port {}", path);
    let mut port = open_port(path, PROBE_TIMEOUT)?;
    send_init(&mut *port)?;

    let mut reader = BufReader::new(&mut *port);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;

    if first_line.contains(device_id) {
        let found_id = first_line.trim().trim_start_matches('/').to_string();
        info!("Found {} on port {}", found_id, path);
        // Drop the BufReader to reclaim the port
        drop(reader);
        Ok(Some(ProbeResult {
            port,
            device_id: found_id,
        }))
    } else {
        debug!(
            "Port {} responded with: {:?} (not target device)",
            path,
            first_line.trim()
        );
        Ok(None)
    }
}

/// Enumerate available serial ports, probe each ttyUSB port, and return
/// the open port that responds with the expected device ID.
pub fn find_meter_port(device_id: &str) -> Result<ProbeResult> {
    let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;

    let usb_ports: Vec<_> = ports
        .iter()
        .filter(|p| p.port_name.contains("ttyUSB"))
        .collect();

    if usb_ports.is_empty() {
        bail!("No /dev/ttyUSB* ports found");
    }

    info!(
        "Found {} USB serial port(s), probing for {}",
        usb_ports.len(),
        device_id
    );

    for port_info in &usb_ports {
        match probe_port(&port_info.port_name, device_id) {
            Ok(Some(result)) => return Ok(result),
            Ok(None) => continue,
            Err(e) => {
                warn!("Error probing {}: {}", port_info.port_name, e);
                continue;
            }
        }
    }

    bail!(
        "Device {} not found on any of the {} USB serial port(s)",
        device_id,
        usb_ports.len()
    )
}
