# elgato4k-linux

**Unofficial Linux control utility for Elgato 4K capture cards**

A limited command-line tool to control Elgato 4K X and 4K S capture cards on Linux. Supports HDR tone mapping, HDMI color range adjustment, EDID source selection, and custom EDID configuration.

> ⚠️ **Disclaimer**: This is an unofficial, community-developed tool. It is not affiliated with, endorsed by, or supported by Elgato/Corsair.

## Features

- ✅ **HDR Tone Mapping** - Enable/disable HDR to SDR tone mapping
- ✅ **HDMI Color Range** - Fix color range mismatches (Auto/Expand/Shrink)
- ✅ **EDID Source Control** - Select EDID mode (Display/Merged/Internal)
- ✅ **Custom EDID** - Enable/disable custom EDID (4K X only)
- ✅ **Auto-detection** - Automatically detects 4K X or 4K S

## Supported Devices

| Device | USB VID:PID | Protocol | Status |
|--------|-------------|----------|--------|
| Elgato 4K X | `0fd9:009c` | UVC Extension Unit | ✅ Fully tested |
| Elgato 4K S | `0fd9:00af` | HID Output Reports | ✅ Fully tested |

## Installation

### Prerequisites

- Rust toolchain (install from [rustup.rs](https://rustup.rs))
- libusb development files
- Root/sudo access (or udev rules)

**Install dependencies:**

```bash
# Ubuntu/Debian
sudo apt-get install libusb-1.0-0-dev

# Fedora/RHEL
sudo dnf install libusb1-devel

# Arch
sudo pacman -S libusb
```

### Build from source

```bash
# Clone the repository
git clone https://github.com/13bm/elgato4k-linux.git
cd elgato4k-linux

# Build release binary
cargo build --release

# Install system-wide (optional)
sudo cp target/release/elgato4k-linux /usr/local/bin/
```

## Usage

### Basic Commands

```bash
# Enable HDR tone mapping
sudo elgato4k-linux --hdr-map on

# Set HDMI color range to Full (expand limited range)
sudo elgato4k-linux --hdmi-range expand

# Set EDID source to passthrough display
sudo elgato4k-linux --edid-source display

# Enable custom EDID (4K X only)
sudo elgato4k-linux --custom-edid on

# Combine multiple settings
sudo elgato4k-linux --hdr-map on --hdmi-range expand --edid-source display
```

### Command Reference

#### `--hdmi-range <VALUE>`
Set HDMI color range conversion:
- `auto` - Respect EDID (default/safest)
- `expand` - Convert Limited (16-235) to Full (0-255) - fixes washed out colors
- `shrink` - Convert Full (0-255) to Limited (16-235) - fixes crushed blacks

#### `--edid-source <VALUE>`
Select EDID mode:
- `display` - Passthrough display's EDID to source
- `merged` - Combined/negotiated EDID
- `internal` - Use capture card's built-in EDID

#### `--hdr-map <VALUE>`
Control HDR tone mapping:
- `on` - Enable HDR to SDR tone mapping
- `off` - Disable tone mapping (passthrough)

#### `--custom-edid <VALUE>`
Enable custom EDID preset (4K X only):
- `on` - Enable custom EDID
- `off` - Disable custom EDID

## Running without sudo

Create a udev rule to allow your user access to the device:

```bash
# Create udev rule file
sudo nano /etc/udev/rules.d/99-elgato-capture.rules
```

Add these lines:
```
# Elgato 4K X
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009c", MODE="0666", GROUP="plugdev"

# Elgato 4K S
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="00af", MODE="0666", GROUP="plugdev"
```

Reload udev rules:
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Add your user to the plugdev group:
```bash
sudo usermod -a -G plugdev $USER
```

Log out and back in for changes to take effect.

## Technical Details

### Protocol Implementation

**Elgato 4K X (UVC Protocol)**
- Uses UVC Extension Unit #4 with custom GUID
- Two-packet sequence: trigger (selector 0x02) + payload (selector 0x01)
- Interface 0 (VideoControl)
- Settings stored in device, persist across reboots

**Elgato 4K S (HID Protocol)**
- Uses HID SET_REPORT requests on Interface 7
- 255-byte zero-padded packets with structured headers
- EDID changes: Single packet
- Tone mapping/Color range: Two-packet sequence with confirmation

### Reverse Engineering

All protocols were reverse-engineered through USB packet capture and analysis:
- Wireshark/USBPcap monitoring of official Elgato software
- Manual packet analysis and pattern recognition
- Testing and validation of command sequences

### Why This Tool Exists

Elgato's official software doesn't support Linux. This tool enables Linux users to:
- Control capture card settings without Windows/macOS
- Integrate capture card configuration into scripts/automation
- Fix common issues like color range mismatches
- Enable HDR tone mapping for proper SDR display

## Common Use Cases

### Fix Washed Out Colors (PS5/Xbox)
```bash
sudo elgato4k-linux --hdmi-range expand
```
Game consoles often output Limited range, but displays expect Full range.

### Enable HDR Capture with Tone Mapping
```bash
sudo elgato4k-linux --hdr-map on --hdmi-range auto
```
Converts HDR input to SDR for display/recording.

### Passthrough Display Capabilities
```bash
sudo elgato4k-linux --edid-source display
```
Lets source device see your display's actual capabilities.

## Troubleshooting

### Device not found
```bash
# Check if device is connected
lsusb | grep 0fd9

# Expected output for 4K X:
# Bus 002 Device 014: ID 0fd9:009c Elgato Systems GmbH

# Expected output for 4K S:
# Bus 002 Device 015: ID 0fd9:00af Elgato Systems GmbH
```

### Permission denied
```bash
# Run with sudo
sudo elgato4k-linux --hdr-map on

# Or set up udev rules (see "Running without sudo" section)
```

### Settings not applying
- Ensure no other software is using the device (OBS, etc.)
- Try unplugging and replugging the device
- Check device is fully initialized (wait a few seconds after plugging in)

### Video stream interruption
The tool briefly detaches the kernel driver to send commands, which may cause a momentary interruption in video capture software. The driver is immediately reattached after commands are sent.

## Contributing

Contributions welcome! Areas for improvement:
- Support for other USB Elgato devices
- GUI wrapper
- Additional feature reverse engineering (like for the 4K S)

Since i gave away my 4K S testing and feature adding will be limited. If you have a 4K S and can test, please open an issue with your results! 


## Disclaimer

This project is not affiliated with, endorsed by, or supported by Elgato Systems GmbH or Corsair Gaming, Inc. All product names, logos, and brands are property of their respective owners.

Use at your own risk. The authors are not responsible for any damage to hardware or data.

## See Also

- [Elgato Official Website](https://www.elgato.com)
- [OBS Studio](https://obsproject.com) - Open source streaming/recording software
- [v4l2-ctl](https://www.mankier.com/1/v4l2-ctl) - Video4Linux control utility
