use anyhow::{bail, Context, Result};
use chrono::Local;
use log::{debug, info};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

use crate::meter::MeterReading;
use crate::probe::{open_port, IEC_INIT_SEQUENCE};

/// Open a serial port, send the IEC 62056-21 init sequence, read and parse
/// the complete meter telegram, and return a MeterReading.
pub fn read_meter(port_path: &str, device_id: &str, timeout: Duration) -> Result<MeterReading> {
    info!("Opening {} for meter reading", port_path);

    let mut port = open_port(port_path, timeout)?;

    port.write_all(IEC_INIT_SEQUENCE)?;
    port.flush()?;
    std::thread::sleep(Duration::from_millis(500));

    let mut reader = BufReader::new(&mut *port);
    let mut reading = MeterReading::default();
    let mut device_confirmed = false;

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
                device_confirmed = true;
                reading.device_id = trimmed.trim_start_matches('/').to_string();
            } else {
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

    if !device_confirmed {
        bail!("Never received device identification line");
    }

    reading.timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string();

    info!("Reading complete: {:?}", reading);
    Ok(reading)
}

/// Parse a single OBIS data line like `1-0:1.8.0(0011404.409*kWh)` and
/// populate the corresponding field in MeterReading.
fn parse_obis_line(line: &str, reading: &mut MeterReading) {
    let (code, raw_value) = match (line.find('('), line.find(')')) {
        (Some(open), Some(close)) if open < close => (&line[..open], &line[open + 1..close]),
        _ => return,
    };

    let value_str = raw_value.replace("*kWh", "").replace("*V", "");

    match code {
        "0.8.1" => {
            // Time: "120054" → "12:00:54"
            if value_str.len() >= 6 {
                reading.time = format!(
                    "{}:{}:{}",
                    &value_str[0..2],
                    &value_str[2..4],
                    &value_str[4..6]
                );
            }
        }
        "0.8.2" => {
            // Date: "1200703" → "20-07-03"
            if value_str.len() >= 7 {
                reading.date = format!(
                    "{}-{}-{}",
                    &value_str[1..3],
                    &value_str[3..5],
                    &value_str[5..7]
                );
            }
        }
        "1-0:1.8.0" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.consumption_ht_kwh = v;
            }
        }
        "1-0:1.8.2" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.consumption_nt_kwh = v;
            }
        }
        "1-0:2.8.0" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.production_t1_kwh = v;
            }
        }
        "1-0:2.8.2" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.production_t2_kwh = v;
            }
        }
        "1-0:32.7.0" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.phase1_voltage = v;
            }
        }
        "1-0:52.7.0" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.phase2_voltage = v;
            }
        }
        "1-0:72.7.0" => {
            if let Ok(v) = value_str.trim().parse() {
                reading.phase3_voltage = v;
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
    fn parse_consumption_ht() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:1.8.0(0011404.409*kWh)", &mut r);
        assert!((r.consumption_ht_kwh - 11404.409).abs() < 0.001);
    }

    #[test]
    fn parse_consumption_nt() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:1.8.2(0023813.725*kWh)", &mut r);
        assert!((r.consumption_nt_kwh - 23813.725).abs() < 0.001);
    }

    #[test]
    fn parse_production_t1() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:2.8.0(0015608.962*kWh)", &mut r);
        assert!((r.production_t1_kwh - 15608.962).abs() < 0.001);
    }

    #[test]
    fn parse_production_t2() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:2.8.2(0000900.569*kWh)", &mut r);
        assert!((r.production_t2_kwh - 900.569).abs() < 0.001);
    }

    #[test]
    fn parse_voltage_phase1() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:32.7.0(230.1*V)", &mut r);
        assert!((r.phase1_voltage - 230.1).abs() < 0.01);
    }

    #[test]
    fn parse_voltage_phase2() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:52.7.0(229.8*V)", &mut r);
        assert!((r.phase2_voltage - 229.8).abs() < 0.01);
    }

    #[test]
    fn parse_voltage_phase3() {
        let mut r = MeterReading::default();
        parse_obis_line("1-0:72.7.0(231.2*V)", &mut r);
        assert!((r.phase3_voltage - 231.2).abs() < 0.01);
    }

    #[test]
    fn parse_time() {
        let mut r = MeterReading::default();
        parse_obis_line("0.8.1(120054)", &mut r);
        assert_eq!(r.time, "12:00:54");
    }

    #[test]
    fn parse_date() {
        let mut r = MeterReading::default();
        parse_obis_line("0.8.2(1200703)", &mut r);
        assert_eq!(r.date, "20-07-03");
    }

    #[test]
    fn unknown_code_ignored() {
        let mut r = MeterReading::default();
        parse_obis_line("C.1.6(FDF5)", &mut r);
        // Should not panic or change any values
        assert_eq!(r.consumption_ht_kwh, 0.0);
    }

    #[test]
    fn malformed_line_ignored() {
        let mut r = MeterReading::default();
        parse_obis_line("garbage without parens", &mut r);
        assert_eq!(r.consumption_ht_kwh, 0.0);
    }
}
