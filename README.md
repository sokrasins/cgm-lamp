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
- ✅ AP Mode when device can't connect or has no credentials
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
