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
- ✅ Included battery, allows fully-wireless operation
  - ✅ Operation with battery optional
  - Indicators for:
    - Remaining capacity
    - Charge state
    - Battery preset/absent
- ✅ USB-C port for charging and debug
  - ✅ Provides both debug console and power/charge
- Re-flash as a generic ESPHome device
- ✅ Optional terminal to extend Neopixel configuration
- Configurable credential storage mode
  - Amnesia
  - Password-protected
  - Permissive

## BOM

[BOM](pcb/bom/ibom.html)

## Things to do

- pcb v1 - fix usb D+ and D-, move encoder button pin
- pcb v1 - Add current measurement header
- pcb v1 - Move buttons away from board edge
- pcb v1 - Smaller power switch
- pcb v1 - Better silk screen, with functional button labels
