# CGM Lamp

An ESP32-C6-powered device to indicate glucose as a color.

## Features

- Displays glucose via lamp color
- Max brightness settable by rotating lamp body
- Device configuration through web server
  - Wifi credentials
  - Dexcom credentials
  - Device settings
- REST API for automated control of device settings
  - Home Assistant compatible
- Included battery, allows fully-wireless operation
- USB-C port for charging and debug
  - Provides both debug console and power/charge
- Basic protection of non-volatile settings
- Re-flash as a generic ESPHome device
  - Neopixel
  - Encoder
  - Push-down button
- Optional terminal to extend Neopixel configuration

## Settings/Status

Available settings include:

- Lamp brightness: set and get
- Volatile-only credential storage option: set and get
  - Or maybe password-protected?
- Connected to internet: get
- Connected to wifi: get
- Factory reset: set
