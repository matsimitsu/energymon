use anyhow::{bail, Context, Result};
use chrono::Local;
use log::{debug, info};
use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::meter::MeterReading;
use crate::probe::{
    baud_rate_from_char, negotiate_baud_rate, open_port, parse_baud_char, send_init, BAUD_RATE,
};

/// Holds an open serial connection to a meter for repeated readings.
pub struct MeterConnection {
    port: Box<dyn serialport::SerialPort>,
    device_id: String,
    /// Whether the first telegram is already in progress (from probing).
    first_read_primed: bool,
    /// The negotiated baud rate for reading telegrams (300 if no negotiation).
    negotiated_baud: u32,
}

impl MeterConnection {
    /// Open a fresh connection and send the first init sequence.
    pub fn open(port_path: &str, device_id: &str, timeout: Duration) -> Result<Self> {
        info!("Opening {} for meter reading", port_path);
        let mut port = open_port(port_path, timeout)?;
        send_init(&mut *port)?;
        Ok(Self {
            port,
            device_id: device_id.to_string(),
            first_read_primed: false,
            negotiated_baud: BAUD_RATE,
        })
    }

    /// Create from a port that was already initialized by the probe.
    /// The device ID line was already consumed during probing, and baud rate
    /// was already negotiated.
    pub fn from_probe(
        port: Box<dyn serialport::SerialPort>,
        device_id: &str,
        negotiated_baud: u32,
    ) -> Self {
        Self {
            port,
            device_id: device_id.to_string(),
            first_read_primed: true,
            negotiated_baud,
        }
    }

    /// Read one telegram from the meter. On subsequent calls, switches back
    /// to 300 baud, sends a new init sequence, negotiates baud rate, then
    /// reads the telegram at the higher baud rate.
    pub fn read(&mut self) -> Result<MeterReading> {
        if self.first_read_primed {
            self.first_read_primed = false;
            info!(
                "Reading first telegram (already primed at {} baud)",
                self.negotiated_baud
            );
            let reader = BufReader::new(&mut *self.port);
            read_telegram(reader, &self.device_id, true)
        } else {
            // Give the meter time to finish processing before the next request
            std::thread::sleep(Duration::from_secs(1));

            // Switch back to 300 baud for the init sequence
            if self.negotiated_baud != BAUD_RATE {
                self.port
                    .set_baud_rate(BAUD_RATE)
                    .context("Failed to reset baud rate to 300")?;
            }

            // Discard any stray bytes left in the serial buffer
            self.port
                .clear(serialport::ClearBuffer::Input)
                .context("Failed to clear serial input buffer")?;

            info!("Sending init sequence for new reading");
            send_init(&mut *self.port)?;

            // Read identification line and negotiate baud rate
            let mut id_line = String::new();
            {
                let mut reader = BufReader::new(&mut *self.port);
                reader
                    .read_line(&mut id_line)
                    .context("Failed to read identification line")?;
            }
            debug!("Identification: {}", id_line.trim());

            if let Some(bc) = parse_baud_char(id_line.trim()) {
                if let Some(rate) = baud_rate_from_char(bc) {
                    if rate > BAUD_RATE {
                        negotiate_baud_rate(&mut *self.port, bc, rate)?;
                        self.negotiated_baud = rate;
                    }
                }
            }

            let reader = BufReader::new(&mut *self.port);
            read_telegram(reader, &self.device_id, true)
        }
    }
}

/// Read and parse the meter telegram from a BufReader.
/// If `device_id_consumed` is true, the device ID line was already read (e.g. during probing).
fn read_telegram(
    mut reader: impl BufRead,
    device_id: &str,
    device_id_consumed: bool,
) -> Result<MeterReading> {
    let mut reading = MeterReading::default();

    if device_id_consumed {
        reading.device_id = device_id.to_string();
    }

    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .context("Failed to read line from serial port")?;

        if bytes_read == 0 {
            bail!("Serial port returned EOF before complete telegram");
        }

        let trimmed = line.trim();
        debug!("Serial: {}", trimmed);

        // Device identification line (e.g. "/ISk5MT174-0001")
        if trimmed.starts_with('/') {
            if trimmed.contains(device_id) {
                reading.device_id = trimmed.trim_start_matches('/').to_string();
            } else if !device_id_consumed {
                bail!("Unexpected device: {}", trimmed);
            }
            continue;
        }

        // End of telegram
        if trimmed.starts_with('!') {
            break;
        }

        if trimmed.is_empty() {
            continue;
        }

        parse_obis_line(trimmed, &mut reading);
    }

    if reading.device_id.is_empty() {
        bail!("Never received device identification line");
    }

    reading.calculate_power();
    reading.timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string();

    info!("Reading complete: {:?}", reading);
    Ok(reading)
}

/// Parse a single OBIS data line like `1-0:1.8.0(0011404.409*kWh)` and
/// populate the corresponding field in MeterReading.
fn parse_obis_line(line: &str, reading: &mut MeterReading) {
    let (raw_code, raw_value) = match (line.find('('), line.find(')')) {
        (Some(open), Some(close)) if open < close => (&line[..open], &line[open + 1..close]),
        _ => return,
    };

    // Strip *255 or similar suffixes from the OBIS code (e.g. "1-0:1.8.0*255" → "1-0:1.8.0")
    let code = raw_code.split('*').next().unwrap_or(raw_code);

    let value_str = raw_value
        .replace("*kWh", "")
        .replace("*kW", "")
        .replace("*V", "")
        .replace("*A", "")
        .replace("*Hz", "");

    let parsed: Option<f64> = value_str.trim().parse().ok();

    match code {
        "1-0:1.8.0" => {
            if let Some(v) = parsed {
                reading.consumption_total_kwh = v;
            }
        }
        "1-0:1.8.1" => {
            if let Some(v) = parsed {
                reading.consumption_t1_kwh = v;
            }
        }
        "1-0:1.8.2" => {
            if let Some(v) = parsed {
                reading.consumption_t2_kwh = v;
            }
        }
        "1-0:2.8.0" => {
            if let Some(v) = parsed {
                reading.production_total_kwh = v;
            }
        }
        "1-0:2.8.1" => {
            if let Some(v) = parsed {
                reading.production_t1_kwh = v;
            }
        }
        "1-0:2.8.2" => {
            if let Some(v) = parsed {
                reading.production_t2_kwh = v;
            }
        }
        "1-0:32.7.0" => {
            if let Some(v) = parsed {
                reading.phase1_voltage = v;
            }
        }
        "1-0:52.7.0" => {
            if let Some(v) = parsed {
                reading.phase2_voltage = v;
            }
        }
        "1-0:72.7.0" => {
            if let Some(v) = parsed {
                reading.phase3_voltage = v;
            }
        }
        "1-0:31.7.0" => {
            if let Some(v) = parsed {
                reading.phase1_current = v;
            }
        }
        "1-0:51.7.0" => {
            if let Some(v) = parsed {
                reading.phase2_current = v;
            }
        }
        "1-0:71.7.0" => {
            if let Some(v) = parsed {
                reading.phase3_current = v;
            }
        }
        "1-0:14.7.0" => {
            if let Some(v) = parsed {
                reading.frequency = v;
            }
        }
        "1-0:33.7.0" => {
            if let Some(v) = parsed {
                reading.phase1_pf = v;
            }
        }
        "1-0:53.7.0" => {
            if let Some(v) = parsed {
                reading.phase2_pf = v;
            }
        }
        "1-0:73.7.0" => {
            if let Some(v) = parsed {
                reading.phase3_pf = v;
            }
        }
        _ => {
            debug!("Ignoring OBIS code: {}", code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meter::MeterReading;

    #[test]
    fn parse_consumption_total() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:1.8.0*255(0002686.675*kWh)", &mut r);
        assert!((r.consumption_total_kwh - 2686.675).abs() < 0.001);
    }

    #[test]
    fn parse_consumption_t1() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:1.8.1*255(0001200.000*kWh)", &mut r);
        assert!((r.consumption_t1_kwh - 1200.0).abs() < 0.001);
    }

    #[test]
    fn parse_consumption_t2() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:1.8.2*255(0002686.675*kWh)", &mut r);
        assert!((r.consumption_t2_kwh - 2686.675).abs() < 0.001);
    }

    #[test]
    fn parse_production_total() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:2.8.0*255(0009354.299*kWh)", &mut r);
        assert!((r.production_total_kwh - 9354.299).abs() < 0.001);
    }

    #[test]
    fn parse_production_t1() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:2.8.1*255(0004000.000*kWh)", &mut r);
        assert!((r.production_t1_kwh - 4000.0).abs() < 0.001);
    }

    #[test]
    fn parse_production_t2() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:2.8.2*255(0009354.299*kWh)", &mut r);
        assert!((r.production_t2_kwh - 9354.299).abs() < 0.001);
    }

    #[test]
    fn parse_voltage() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:32.7.0*255(231.3*V)", &mut r);
        parse_obis_line("1-0:52.7.0*255(233.2*V)", &mut r);
        parse_obis_line("1-0:72.7.0*255(231.4*V)", &mut r);
        assert!((r.phase1_voltage - 231.3).abs() < 0.01);
        assert!((r.phase2_voltage - 233.2).abs() < 0.01);
        assert!((r.phase3_voltage - 231.4).abs() < 0.01);
    }

    #[test]
    fn parse_current() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:31.7.0*255(0.98*A)", &mut r);
        parse_obis_line("1-0:51.7.0*255(0.10*A)", &mut r);
        parse_obis_line("1-0:71.7.0*255(0.64*A)", &mut r);
        assert!((r.phase1_current - 0.98).abs() < 0.001);
        assert!((r.phase2_current - 0.10).abs() < 0.001);
        assert!((r.phase3_current - 0.64).abs() < 0.001);
    }

    #[test]
    fn parse_frequency() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:14.7.0*255(50.03*Hz)", &mut r);
        assert!((r.frequency - 50.03).abs() < 0.001);
    }

    #[test]
    fn parse_without_star_suffix() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:1.8.0(0011404.409*kWh)", &mut r);
        assert!((r.consumption_total_kwh - 11404.409).abs() < 0.001);
    }

    #[test]
    fn parse_power_factor() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:33.7.0*255(0.950)", &mut r);
        parse_obis_line("1-0:53.7.0*255(0.800)", &mut r);
        parse_obis_line("1-0:73.7.0*255(0.750)", &mut r);
        assert!((r.phase1_pf - 0.950).abs() < 0.001);
        assert!((r.phase2_pf - 0.800).abs() < 0.001);
        assert!((r.phase3_pf - 0.750).abs() < 0.001);
    }

    #[test]
    fn unknown_code_ignored() {
        let mut r = MeterReading::default();
        parse_obis_line("0-0:C.1.6*255(FDF5)", &mut r);
        assert_eq!(r.consumption_total_kwh, 0.0);
    }

    #[test]
    fn malformed_line_ignored() {
        let mut r = MeterReading::default();
        parse_obis_line("garbage without parens", &mut r);
        assert_eq!(r.consumption_total_kwh, 0.0);
    }

    #[test]
    fn read_full_telegram() {
        // Expected per-phase power: V × I × PF
        // L1: 231.3 × 0.98 × 1.0 = 226.67W
        // L2: 233.2 × 0.10 × 1.0 = 23.32W
        // L3: 231.4 × 0.64 × 1.0 = 148.10W
        // Total: 398.09W
        let telegram = "\
/ISk5MT174-0001\r\n\
\r\n\
1-0:0.0.0*255(88381140)\r\n\
1-0:1.8.0*255(0002686.675*kWh)\r\n\
1-0:1.8.2*255(0002686.675*kWh)\r\n\
1-0:2.8.0*255(0009354.299*kWh)\r\n\
1-0:2.8.2*255(0009354.299*kWh)\r\n\
1-0:32.7.0*255(231.3*V)\r\n\
1-0:52.7.0*255(233.2*V)\r\n\
1-0:72.7.0*255(231.4*V)\r\n\
1-0:31.7.0*255(0.98*A)\r\n\
1-0:51.7.0*255(0.10*A)\r\n\
1-0:71.7.0*255(0.64*A)\r\n\
1-0:14.7.0*255(50.03*Hz)\r\n\
1-0:33.7.0*255(1.000)\r\n\
1-0:53.7.0*255(1.000)\r\n\
1-0:73.7.0*255(1.000)\r\n\
!\r\n";
        let reader = std::io::BufReader::new(telegram.as_bytes());
        let reading = read_telegram(reader, "ISk5MT174", false).unwrap();
        assert_eq!(reading.device_id, "ISk5MT174-0001");
        assert!((reading.consumption_total_kwh - 2686.675).abs() < 0.001);
        assert!((reading.production_total_kwh - 9354.299).abs() < 0.001);
        assert!((reading.phase1_voltage - 231.3).abs() < 0.01);
        assert!((reading.phase1_current - 0.98).abs() < 0.001);
        assert!((reading.frequency - 50.03).abs() < 0.001);
        assert!((reading.phase1_power - 226.67).abs() < 0.1);
        assert!((reading.phase2_power - 23.32).abs() < 0.1);
        assert!((reading.phase3_power - 148.10).abs() < 0.1);
        assert!((reading.total_power - 398.09).abs() < 0.1);
    }
}
