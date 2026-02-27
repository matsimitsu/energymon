use serde::Serialize;

#[derive(Debug, Serialize, Default)]
pub struct MeterReading {
    pub device_id: String,
    pub time: String,
    pub date: String,
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
    pub timestamp: String,
}
