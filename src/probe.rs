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

/// Map IEC 62056-21 baud rate identification character to actual baud rate.
/// The character is the 4th char of the identification string (after 3-char vendor ID).
/// e.g. in `/ISk5MT174-0001`, vendor = `ISk`, baud char = `5` → 9600 baud.
pub fn baud_rate_from_char(c: char) -> Option<u32> {
    match c {
        '0' => Some(300),
        '1' => Some(600),
        '2' => Some(1200),
        '3' => Some(2400),
        '4' => Some(4800),
        '5' => Some(9600),
        '6' => Some(19200),
        _ => None,
    }
}

/// Extract the baud rate character from an identification line like `/ISk5MT174-0001`.
/// Returns the 4th character after the leading `/` (index 4 in the raw line).
pub fn parse_baud_char(identification: &str) -> Option<char> {
    // Strip leading `/` if present
    let id = identification.trim_start_matches('/');
    // 3-char vendor ID, then baud rate char
    id.chars().nth(3)
}

/// Send the IEC 62056-21 acknowledgement/option select message to negotiate
/// a higher baud rate, then switch the port to that baud rate.
/// ACK format: \x06 0 <baud_char> 0 \r \n
/// - byte 0: ACK (0x06)
/// - byte 1: protocol control character '0' (normal protocol)
/// - byte 2: baud rate identification character
/// - byte 3: mode control '0' (data readout)
pub fn negotiate_baud_rate(
    port: &mut dyn serialport::SerialPort,
    baud_char: char,
    baud_rate: u32,
) -> Result<()> {
    let ack = format!("\x060{}0\r\n", baud_char);
    port.write_all(ack.as_bytes())?;
    port.flush()?;

    // Wait for meter to switch baud rate (IEC 62056-21 specifies 200-300ms)
    std::thread::sleep(Duration::from_millis(300));

    port.set_baud_rate(baud_rate)
        .with_context(|| format!("Failed to set baud rate to {}", baud_rate))?;

    info!("Negotiated baud rate: {} (char '{}')", baud_rate, baud_char);
    Ok(())
}

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

/// Result of a successful probe: the open port, the device ID, and the negotiated baud rate.
pub struct ProbeResult {
    pub port: Box<dyn serialport::SerialPort>,
    pub device_id: String,
    /// The baud rate negotiated from the identification line (0 = no negotiation, stay at 300).
    pub negotiated_baud: u32,
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

        // Negotiate higher baud rate if supported
        let negotiated_baud = match parse_baud_char(first_line.trim()) {
            Some(bc) => match baud_rate_from_char(bc) {
                Some(rate) if rate > BAUD_RATE => {
                    // Drop the BufReader to reclaim the port before writing
                    drop(reader);
                    negotiate_baud_rate(&mut *port, bc, rate)?;
                    rate
                }
                _ => {
                    drop(reader);
                    BAUD_RATE
                }
            },
            None => {
                drop(reader);
                BAUD_RATE
            }
        };

        Ok(Some(ProbeResult {
            port,
            device_id: found_id,
            negotiated_baud,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baud_rate_char_mapping() {
        assert_eq!(baud_rate_from_char('0'), Some(300));
        assert_eq!(baud_rate_from_char('1'), Some(600));
        assert_eq!(baud_rate_from_char('2'), Some(1200));
        assert_eq!(baud_rate_from_char('3'), Some(2400));
        assert_eq!(baud_rate_from_char('4'), Some(4800));
        assert_eq!(baud_rate_from_char('5'), Some(9600));
        assert_eq!(baud_rate_from_char('6'), Some(19200));
        assert_eq!(baud_rate_from_char('7'), None);
        assert_eq!(baud_rate_from_char('A'), None);
    }

    #[test]
    fn parse_baud_char_from_identification() {
        // ISk5MT174-0001: vendor=ISk, baud char=5 (9600)
        assert_eq!(parse_baud_char("/ISk5MT174-0001"), Some('5'));
        assert_eq!(parse_baud_char("ISk5MT174-0001"), Some('5'));
    }

    #[test]
    fn parse_baud_char_different_rates() {
        // Hypothetical meter at 300 baud (char '0')
        assert_eq!(parse_baud_char("/ABC0Model"), Some('0'));
        // Meter at 19200 baud (char '6')
        assert_eq!(parse_baud_char("/XYZ6Model"), Some('6'));
    }

    #[test]
    fn parse_baud_char_too_short() {
        assert_eq!(parse_baud_char("/AB"), None);
        assert_eq!(parse_baud_char(""), None);
    }
}
