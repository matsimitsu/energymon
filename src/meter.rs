use serde::Serialize;

#[derive(Debug, Serialize, Default)]
pub struct MeterReading {
    pub device_id: String,
    /// Positive active energy total (kWh) — OBIS 1-0:1.8.0
    pub consumption_total_kwh: f64,
    /// Positive active energy tariff 1 / HT (kWh) — OBIS 1-0:1.8.1
    pub consumption_t1_kwh: f64,
    /// Positive active energy tariff 2 / NT (kWh) — OBIS 1-0:1.8.2
    pub consumption_t2_kwh: f64,
    /// Negative active energy total (kWh) — OBIS 1-0:2.8.0
    pub production_total_kwh: f64,
    /// Negative active energy tariff 1 (kWh) — OBIS 1-0:2.8.1
    pub production_t1_kwh: f64,
    /// Negative active energy tariff 2 (kWh) — OBIS 1-0:2.8.2
    pub production_t2_kwh: f64,
    /// Phase 1 voltage (V) — OBIS 1-0:32.7.0
    pub phase1_voltage: f64,
    /// Phase 2 voltage (V) — OBIS 1-0:52.7.0
    pub phase2_voltage: f64,
    /// Phase 3 voltage (V) — OBIS 1-0:72.7.0
    pub phase3_voltage: f64,
    /// Phase 1 current (A) — OBIS 1-0:31.7.0
    pub phase1_current: f64,
    /// Phase 2 current (A) — OBIS 1-0:51.7.0
    pub phase2_current: f64,
    /// Phase 3 current (A) — OBIS 1-0:71.7.0
    pub phase3_current: f64,
    /// Grid frequency (Hz) — OBIS 1-0:14.7.0
    pub frequency: f64,
    /// Phase 1 real power (W) — OBIS 1-0:21.7.0
    pub phase1_power: f64,
    /// Phase 2 real power (W) — OBIS 1-0:41.7.0
    pub phase2_power: f64,
    /// Phase 3 real power (W) — OBIS 1-0:61.7.0
    pub phase3_power: f64,
    /// Total real power (W) — sum of all phases
    pub total_power: f64,
    pub timestamp: String,
}

impl MeterReading {
    /// Calculate total real power as sum of per-phase real power readings.
    pub fn calculate_power(&mut self) {
        self.total_power =
            (((self.phase1_power + self.phase2_power + self.phase3_power) * 100.0).round()) / 100.0;
    }
}
