use serde::Serialize;

#[derive(Debug, Serialize, Default)]
pub struct MeterReading {
    pub device_id: String,
    /// Positive active energy tariff 1 / HT (kWh) — OBIS 1-0:1.8.0
    pub consumption_ht_kwh: f64,
    /// Positive active energy tariff 2 / NT (kWh) — OBIS 1-0:1.8.2
    pub consumption_nt_kwh: f64,
    /// Negative active energy tariff 1 (kWh) — OBIS 1-0:2.8.0
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
    /// Phase 1 apparent power (W) — calculated: voltage × current
    pub phase1_power: f64,
    /// Phase 2 apparent power (W) — calculated: voltage × current
    pub phase2_power: f64,
    /// Phase 3 apparent power (W) — calculated: voltage × current
    pub phase3_power: f64,
    /// Total apparent power (W) — sum of all phases
    pub total_power: f64,
    pub timestamp: String,
}

impl MeterReading {
    /// Calculate per-phase and total power from voltage and current.
    pub fn calculate_power(&mut self) {
        self.phase1_power = (self.phase1_voltage * self.phase1_current * 100.0).round() / 100.0;
        self.phase2_power = (self.phase2_voltage * self.phase2_current * 100.0).round() / 100.0;
        self.phase3_power = (self.phase3_voltage * self.phase3_current * 100.0).round() / 100.0;
        self.total_power = (((self.phase1_power + self.phase2_power + self.phase3_power) * 100.0).round()) / 100.0;
    }
}
