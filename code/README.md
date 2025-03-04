# CGM Lamp Firmware

## Setup

TODO

## Building

```bash
cargo clean
cargo build
```

## Flashing

```bash
cargo run
```

## Testing

TODO

## API

NOTE: I don't seem to be able to define multiple REST methods on a single API endpoint

**/api/v1/set** - POST

```json
{
  "brightness": 0-255,
  "state": "on/off",
  "cred-store": "AMNESIA | PASSWORD | PERMISSIVE",
  "wifi-ssid": "",
  "wifi-psk": "",
  "dexcom-user": "",
  "dexcom-pass": "",
}
```

**/api/v1/state** - GET

```json
{
  "brightness": 0-255,
  "state": "on/off",
  "cred-store": "AMNESIA | PASSWORD | PERMISSIVE",
  "wifi-has-ssid": "true | false",
  "wifi-has-pass": "true | false",
  "dexcom-has-user": "true | false",
  "dexcom-has-pass": "true | false",
  "batt-capacity": 0-100,
  "batt-attached": "true | false"
  "batt-charging": "true | false"
  "temp": 0-100,
  "uptime": 0-0xFFFFFFFF
}
```

**/api/v1/reset** - POST

No body required for this endpoint - performs a factory reset, restoring default
settings to device.

## To Do

- Battery charge state needs more logic
  - Is usb connected?
- Re-implement lamp breathe, with faster response
- Change server for something with TLS
- Show power state in web UI
- Better encoder control over lamp
