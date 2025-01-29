# CGM Lamp

An ESP32-C6-powered device to indicate glucose as a color.

## Features

- ✅ Displays glucose via lamp color
- Max brightness settable by rotating lamp body
- Device configuration through web server
  - HTTPS
  - Wifi credentials
  - Dexcom credentials
  - ✅ Device settings
- REST API for automated control of device settings
  - TLS
  - ✅ Home Assistant compatible
- Included battery, allows fully-wireless operation
- USB-C port for charging and debug
  - Provides both debug console and power/charge
- Basic protection of non-volatile settings
- Re-flash as a generic ESPHome device
  - Neopixel
  - Encoder
  - Push-down button
- Optional terminal to extend Neopixel configuration
- Configurable credential storage mode
  - Amnesia
  - Password-protected
  - Permissive

## API 

API:

v1/set - post

POST {
  "brightness": 0-255,
  "state": "on/off",
  "cred-store": "AMNESIA | PASSWORD | PERMISSIVE",
  "wifi-ssid": "",
  "wifi-psk": "",
  "dexcom-user": "",
  "dexcom-pass": "",
}

v1/state - get

GET{
  "brightness": 0-255,
  "state": "on/off",
  "cred-store": "AMNESIA | PASSWORD | PERMISSIVE",
  "wifi-ssid": "",
  "wifi-has-pass": "true | false",
  "dexcom-user": "",
  "dexcom-has-pass": "true | false",
  "temp": 0-100,
  "uptime": millis()
}

v1/reset - post




