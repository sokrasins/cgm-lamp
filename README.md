# CGM Lamp

An ESP32-C6-powered device to indicate glucose as a color

- `/pcb` - KiCAD PCB design
- `/code` - Rust firmware project

## Features

- ✅ Displays glucose via lamp color
- Max brightness settable by rotating lamp body
- ✅ Device configuration through web server
  - ✅ Wifi credentials
  - ✅ Dexcom credentials
  - ✅ Device settings
- Non-volatile storage of critical device settings
  - ✅ Store mission-critical device settings
  - At-rest encryption
- AP Mode when device can't connect or has no credentials
- REST API for automated control of device settings
  - ✅ Home Assistant compatible
  - TLS
- Included battery, allows fully-wireless operation
  - Operation with battery optional
  - Indicators for:
    - Remaining capacity
    - Charge state
    - Battery preset/absent
- USB-C port for charging and debug
  - Provides both debug console and power/charge
- Re-flash as a generic ESPHome device
- Optional terminal to extend Neopixel configuration
- Configurable credential storage mode
  - Amnesia
  - Password-protected
  - Permissive

## API

API:

/api/v1/set - post

POST {
  "brightness": 0-255,
  "state": "on/off",
  "cred-store": "AMNESIA | PASSWORD | PERMISSIVE",
  "wifi-ssid": "",
  "wifi-psk": "",
  "dexcom-user": "",
  "dexcom-pass": "",
}

/api/v1/state - get

GET{
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
  "uptime": millis()
}

/api/v1/reset - post
