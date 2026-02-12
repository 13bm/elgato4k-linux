# elgato4k-linux

**Unofficial Linux control utility for Elgato 4K capture cards**

A command-line tool to control Elgato 4K X and 4K S capture cards on Linux. Supports HDR tone mapping, HDMI color range adjustment, EDID source selection, custom EDID configuration, USB speed switching, audio input selection, and video scaler control.

> ⚠️ **Disclaimer**: This is an unofficial, community-developed tool. It is not affiliated with, endorsed by, or supported by Elgato/Corsair.

## Features

- ✅ **HDR Tone Mapping** - Enable/disable HDR to SDR tone mapping
- ✅ **HDMI Color Range** - Fix color range mismatches (Auto/Expand/Shrink)
- ✅ **EDID Source Control** - Select EDID mode (Display/Merged/Internal)
- ✅ **Custom EDID** - Enable/disable custom EDID preset (4K X only)
- ✅ **USB Speed Switching** - Switch between 5Gbps and 10Gbps modes (4K X only)
- ✅ **Audio Input Selection** - Switch between HDMI embedded and analog audio (4K S only)
- ✅ **Video Scaler** - Enable/disable video scaling (4K S only)
- ✅ **Firmware Version** - Read firmware version from device
- ✅ **Status Reading** - Read back current settings from both 4K X and 4K S
- ✅ **Auto-detection** - Automatically detects 4K X or 4K S across all USB speed modes

## Supported Devices

| Device | USB VID:PID | Speed Mode | Protocol | Status |
|--------|-------------|------------|----------|--------|
| Elgato 4K X | `0fd9:009b` | 10Gbps / SuperSpeed+ (USB 3.2) | UVC Extension Unit | ✅ Supported |
| Elgato 4K X | `0fd9:009c` | 5Gbps / SuperSpeed (USB 3.1) | UVC Extension Unit | ✅ Fully tested |
| Elgato 4K X | `0fd9:009d` | USB 2.0 | UVC Extension Unit | ✅ Supported |
| Elgato 4K S | `0fd9:00af` | USB 3.0 | HID Output Reports | ✅ Fully tested |
| Elgato 4K S | `0fd9:00ae` | USB 2.0 | HID Output Reports | ✅ Supported |

> **Note:** The 4K X changes its PID depending on the active USB speed mode. If your device shows a different PID than expected, it's likely in a different speed mode. Use `--usb-speed` to switch modes.

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

### Note on 10Gbps Mode (PID 009b)

If your 4K X is in 10Gbps mode (PID `009b`) and your kernel doesn't recognize it, the simplest fix is to switch to 5Gbps mode:

```bash
sudo elgato4k-linux --usb-speed 5g
```

This changes the device to PID `009c` which works on all kernels. 5Gbps is sufficient for most capture scenarios.

If you specifically need 10Gbps mode, you can apply the `USB_QUIRK_NO_BOS` kernel quirk ([submitted upstream](https://lore.kernel.org/linux-usb/20251207090220.14807-1-johannes.bruederl@gmail.com/) by Reddit user [birdayz](https://reddit.com/u/birdayz)) as a boot parameter:

```bash
# Add to GRUB_CMDLINE_LINUX in /etc/default/grub:
usbcore.quirks=0fd9:009b:o
```

Then run `sudo update-grub` and reboot.

## Usage

### Basic Commands

```bash
# Enable HDR tone mapping
sudo elgato4k-linux --hdr-map on

# Set HDMI color range to Full (expand limited range)
sudo elgato4k-linux --hdmi-range expand

# Set EDID source to passthrough display
sudo elgato4k-linux --edid-source display

# Enable custom EDID preset (4K X only)
sudo elgato4k-linux --custom-edid on

# Switch USB speed to 10Gbps (4K X only)
sudo elgato4k-linux --usb-speed 10g

# Switch audio input to analog (4K S only)
sudo elgato4k-linux --audio-input analog

# Enable video scaler (4K S only)
sudo elgato4k-linux --video-scaler on

# Combine multiple settings
sudo elgato4k-linux --hdr-map on --hdmi-range expand --edid-source display

# Read current device settings
sudo elgato4k-linux --status

# Read firmware version
sudo elgato4k-linux --firmware-version
```

### Command Reference

#### `--hdmi-range <VALUE>` / `--edid-range <VALUE>`
Set HDMI color range (EDID range policy):
- `auto` - Respect EDID (default/safest)
- `expand` - Convert Limited (16-235) to Full (0-255) - fixes washed out colors
- `shrink` - Convert Full (0-255) to Limited (16-235) - fixes crushed blacks

#### `--edid-source <VALUE>`
Select EDID mode. This controls what EDID information the capture card presents to the source device:
- `display` - **Passthrough** your monitor's EDID to the source device. Best when you want the source to match your monitor's actual capabilities (resolution, HDR support, etc.)
- `merged` - **Combined/negotiated** EDID from all connected displays. The capture card merges capabilities to find a common set
- `internal` - **Built-in** EDID from the capture card's firmware. Use this when the source device doesn't work well with your monitor's EDID, or when no monitor is connected

#### `--hdr-map <VALUE>`
Control HDR tone mapping:
- `on` - Enable HDR to SDR tone mapping
- `off` - Disable tone mapping (passthrough)

#### `--custom-edid <VALUE>` (4K X only)
Enable/disable a custom EDID preset:
- `on` - Enable custom EDID (selects preset index 1)
- `off` - Disable custom EDID

> **Note:** This toggles a pre-configured EDID preset stored on the device. Uploading custom EDID files is not yet supported in this tool. The preset must first be configured using the official Elgato software on Windows/macOS.

#### `--audio-input <VALUE>` (4K S only)
Select audio input source:
- `embedded` - HDMI embedded audio (default)
- `analog` - Analog/line-in audio input

#### `--video-scaler <VALUE>` (4K S only)
Enable/disable video scaling:
- `on` - Enable video scaler
- `off` - Disable video scaler

#### `--usb-speed <VALUE>` (4K X only)
Switch USB speed mode:
- `5g` - 5Gbps (SuperSpeed, PID changes to `009c`)
- `10g` - 10Gbps (SuperSpeed+, PID changes to `009b`)

> **Warning:** The device will disconnect and re-enumerate with a different USB PID after changing speed. Your USB device path will change, and any active capture sessions will be interrupted.

#### `--status`
Read and display current device settings.
- **4K X**: Firmware version, USB speed mode, HDMI color range, HDR tone mapping, EDID range policy, and EDID source selection (via UVC Extension Unit reads)
- **4K S**: Firmware version, HDR tone mapping, HDMI color range, EDID mode, audio input, and video scaler state (via HID ReadI2cData protocol, discovered from EGAVDeviceSupport.dll decompilation)

#### `--firmware-version`
Read and display the device firmware version.
- **4K X**: Uses AT command `0x77` (`AT_Get_Customer_Ver`) to query the ITE UB700E chip. Version format: YYMMDD packed decimal (e.g., `25.02.10`)
- **4K S**: Uses HID read command `0x55`/`0x02` to query the MCU. Version format: DateThreeBytes BCD (e.g., `25.0c.03`)

## Running without sudo

Create a udev rule to allow your user access to the device:

```bash
# Create udev rule file
sudo nano /etc/udev/rules.d/99-elgato-capture.rules
```

Add these lines:
```
# Elgato 4K X (all speed modes)
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009b", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009c", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009d", MODE="0666", GROUP="plugdev"

# Elgato 4K S (all speed modes)
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="00af", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="00ae", MODE="0666", GROUP="plugdev"
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
- Uses UVC Extension Unit #4 with GUID `961073c7-49f7-44f2-ab42-e940405940c2`
- Two-packet sequence: trigger (selector 0x02) + payload (selector 0x01)
- Interface 0 (VideoControl)
- Settings stored in device, persist across reboots
- AT command framing for advanced features (USB speed switching)

**Elgato 4K S (HID Protocol)**
- Uses HID SET_REPORT/GET_REPORT requests on Interface 7
- 255-byte zero-padded packets with `06 06 06 55` header
- Command byte at offset 5 selects the feature (0x0a=HDR, 0x0b=color range, 0x08=audio, 0x12=EDID, 0x19=scaler)
- Most settings require a two-packet sequence (command + confirmation `0x13 0x01`)
- Read capability uses ReadI2cData protocol: SET_REPORT `[06, 55, sub_cmd, length]` then GET_REPORT to receive response
- Read sub-commands: 0x0a=HDR state, 0x0b=color range, 0x08=audio input, 0x12=EDID mode, 0x19=scaler, 0x02=firmware version
- MCU firmware: ARM Cortex-M0 (ITE "Splitter" chip), command dispatch in main loop function

### Reverse Engineering

All protocols were reverse-engineered through:
- USB packet capture and analysis (Wireshark/USBPcap)
- Ghidra decompilation of official Windows DLLs (RTK_IO_x64.dll, RTICE_SDK_x64.dll, EGAVDeviceSupport.dll)
- Ghidra decompilation of 4K S MCU firmware (FW_4K_S_MCU.bin, ARM Cortex-M0)
- ITE UB700E chip AT command protocol analysis
- Manual testing and validation

See [LOW_CONFIDENCE_COMMANDS.md](LOW_CONFIDENCE_COMMANDS.md) for a full list of discovered firmware commands that are not yet implemented.

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

### Use Analog Audio Input (4K S)
```bash
sudo elgato4k-linux --audio-input analog
```
Switch to the analog/line-in audio input instead of HDMI embedded audio.

### Switch to 10Gbps for Maximum Throughput
```bash
sudo elgato4k-linux --usb-speed 10g
```
Enables 10Gbps SuperSpeed+ mode for higher bandwidth capture. Requires a USB 3.2 Gen 2 port and may require the kernel quirk (see installation section).

## Known Limitations

- **Custom EDID upload**: Cannot upload custom EDID files to the device yet (only toggle pre-configured presets)
- **Firmware updates**: Not supported (use official software)
- **4K S USB speed**: The 4K S does not support USB speed switching
- **4K S audio/scaler**: Audio input and video scaler commands were discovered via Ghidra and need hardware testing
- **4K Pro/other models**: Only 4K X and 4K S are supported
- **Signal info**: HDMI signal timing and HDR infoframe data can be read but is not yet decoded (complex binary structures)

## Troubleshooting

### Device not found
```bash
# Check if device is connected
lsusb | grep 0fd9

# Expected output examples:
# Bus 002 Device 014: ID 0fd9:009b Elgato Systems GmbH   (4K X, 10Gbps)
# Bus 002 Device 014: ID 0fd9:009c Elgato Systems GmbH   (4K X, 5Gbps)
# Bus 002 Device 015: ID 0fd9:00af Elgato Systems GmbH   (4K S, USB 3.0)
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

### 10Gbps mode not working
- Easiest fix: switch to 5Gbps with `sudo elgato4k-linux --usb-speed 5g` (sufficient for most use cases)
- If you need 10Gbps: ensure your USB port supports USB 3.2 Gen 2
- Apply the kernel quirk if your kernel doesn't recognize PID `009b` (see installation section)
- Verify with `lsusb` that the device shows PID `009b`

## Acknowledgments

- [@RadioFreeKerbin](https://github.com/RadioFreeKerbin) — Discovered the 4K X PID changes across USB speed modes
- [@AndySchroder](https://github.com/AndySchroder) — Pointed out the need for clearer EDID option documentation
- [@Tatsh2DX](https://www.reddit.com/u/Tatsh2DX) — Identified the sendATCommand(0x8e) call for USB speed switching in the macOS binary.
- [konovalov-nk](https://www.reddit.com/u/konovalov-nk) — for their comedic journey of suffering enabling 10Gbps
- [birdayz](https://reddit.com/u/birdayz) (Johannes Brüderl) — Submitted the [`USB_QUIRK_NO_BOS` kernel patch](https://lore.kernel.org/linux-usb/20251207090220.14807-1-johannes.bruederl@gmail.com/) for 10Gbps mode support

## Contributing

Contributions welcome! Areas for improvement:
- Custom EDID file upload support
- Support for other USB Elgato devices
- GUI wrapper
- Additional feature reverse engineering

Since I gave away my 4K S, testing and feature adding will be limited. If you have a 4K S and can test, please open an issue with your results!

## Disclaimer

This project is not affiliated with, endorsed by, or supported by Elgato Systems GmbH or Corsair Gaming, Inc. All product names, logos, and brands are property of their respective owners.

Use at your own risk. The authors are not responsible for any damage to hardware or data.

## See Also

- [Elgato Official Website](https://www.elgato.com)
- [OBS Studio](https://obsproject.com) - Open source streaming/recording software
- [v4l2-ctl](https://www.mankier.com/1/v4l2-ctl) - Video4Linux control utility
