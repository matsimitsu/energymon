# energymon

Reads an ISk5MT174 electricity meter via an IR optical head (Weidmann USB IR Kopf) using the IEC 62056-21 (D0) protocol and publishes the readings to MQTT.

## Features

- Reads energy consumption, production, and phase voltages via OBIS codes
- Publishes JSON payload to an MQTT broker
- Auto-probes `/dev/ttyUSB*` ports to find the correct meter when multiple IR heads are connected

## Usage

```bash
# Auto-probe USB serial ports to find the electricity meter:
RUST_LOG=info energymon

# Specify the port directly:
RUST_LOG=info energymon --port /dev/ttyUSB0

# Custom MQTT settings:
energymon --mqtt-host 192.168.1.10 --mqtt-topic home/energy
```

### Options

```
--mqtt-host <HOST>          MQTT broker hostname [default: 127.0.0.1]
--mqtt-port <PORT>          MQTT broker port [default: 1883]
--mqtt-client-id <ID>       MQTT client ID [default: ISK5MT174-DATA]
--mqtt-topic <TOPIC>        MQTT topic [default: tele/ISK5MT174]
--device-id <ID>            Device identifier to match [default: ISk5MT174]
--port <PATH>               Serial port path (skips probing)
--timeout-secs <SECS>       Serial read timeout [default: 10]
```

### MQTT payload

```json
{
  "device_id": "ISk5MT174-0001",
  "time": "12:00:54",
  "date": "20-07-03",
  "consumption_ht_kwh": 11404.409,
  "consumption_nt_kwh": 23813.725,
  "production_t1_kwh": 15608.962,
  "production_t2_kwh": 900.569,
  "phase1_voltage": 230.1,
  "phase2_voltage": 229.8,
  "phase3_voltage": 231.2,
  "timestamp": "2026-02-27 14:30:22.123456"
}
```

## Building

```bash
cargo build --release
```

### Static musl binary (for Linux)

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

Pre-built static binaries are available from [GitHub Actions](../../actions).

## Serial port permissions

The user running the binary needs access to `/dev/ttyUSB*`. Add your user to the `dialout` group:

```bash
sudo usermod -aG dialout $USER
```

## License

MIT
