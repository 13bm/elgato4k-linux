//! Device setting enums and payload generation.
//!
//! Each setting type maps to a device payload:
//! - **4K X (UVC):** Fixed `a1`-prefixed byte sequences sent via SET_CUR.
//! - **4K S (HID):** Single 255-byte packets using the `06 06 06 55 02` header.
//!
//! All enums implement [`std::fmt::Display`] for user-facing output and
//! [`std::str::FromStr`] for CLI parsing.

use std::fmt;
use std::str::FromStr;

use crate::protocol::*;

/// Which device model we're talking to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceModel {
    Elgato4KX,
    Elgato4KS,
}

impl fmt::Display for DeviceModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Elgato4KX => write!(f, "4K X"),
            Self::Elgato4KS => write!(f, "4K S"),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: build a 255-byte HID write packet from header + sub_cmd + value
// ---------------------------------------------------------------------------

/// Build a single HID settings write packet: `[06 06 06 55 02] [sub_cmd] [value]`
/// padded to [`HID_PACKET_SIZE`].
fn hid_write_packet(sub_cmd: u8, value: u8) -> [u8; HID_PACKET_SIZE] {
    let mut pkt = [0u8; HID_PACKET_SIZE];
    pkt[..HID_WRITE_HEADER.len()].copy_from_slice(&HID_WRITE_HEADER);
    pkt[HID_WRITE_HEADER.len()] = sub_cmd;
    pkt[HID_WRITE_HEADER.len() + 1] = value;
    pkt
}

// ---------------------------------------------------------------------------
// EDID Range Policy (HDMI Color Range)
// ---------------------------------------------------------------------------

/// EDID Range Policy (controls color range quantization).
///
/// Despite the CLI flag name `--hdmi-range`, this actually controls the
/// EDID Range Policy via the `a1 08 ... 7c` payload family (11 bytes).
/// The official Elgato software labels this as "HDMI Color Range" in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdidRangePolicy {
    /// Full range (0–255).
    Expand,
    /// Limited range (16–235).
    Shrink,
    /// Auto-detect.
    Auto,
}

impl fmt::Display for EdidRangePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expand => write!(f, "Expand (Full)"),
            Self::Shrink => write!(f, "Shrink (Limited)"),
            Self::Auto => write!(f, "Auto"),
        }
    }
}

impl FromStr for EdidRangePolicy {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "expand" | "full" => Ok(Self::Expand),
            "shrink" | "limited" => Ok(Self::Shrink),
            "auto" => Ok(Self::Auto),
            _ => Err(()),
        }
    }
}

impl EdidRangePolicy {
    pub const VALID_VALUES: &str = "expand, shrink, auto";

    pub fn payload_4kx(&self) -> &'static [u8] {
        match self {
            Self::Auto   => &[0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x00, 0xda],
            Self::Expand => &[0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x03, 0xd7],
            Self::Shrink => &[0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x04, 0xd6],
        }
    }

    pub fn payload_4ks(&self) -> [u8; HID_PACKET_SIZE] {
        let value = match self {
            Self::Auto   => 0x00,
            Self::Expand => 0x01,
            Self::Shrink => 0x02,
        };
        hid_write_packet(SUBCMD_COLOR_RANGE, value)
    }
}

// ---------------------------------------------------------------------------
// EDID Source
// ---------------------------------------------------------------------------

/// EDID source selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdidSource {
    /// Passthrough monitor's EDID.
    Display,
    /// Combined EDID from all displays.
    Merged,
    /// Capture card's built-in EDID.
    Internal,
}

impl fmt::Display for EdidSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Display => write!(f, "Display"),
            Self::Merged => write!(f, "Merged"),
            Self::Internal => write!(f, "Internal"),
        }
    }
}

impl FromStr for EdidSource {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "display" => Ok(Self::Display),
            "merged" => Ok(Self::Merged),
            "internal" => Ok(Self::Internal),
            _ => Err(()),
        }
    }
}

impl EdidSource {
    pub const VALID_VALUES: &str = "display, merged, internal";

    pub fn payload_4kx(&self) -> &'static [u8] {
        match self {
            Self::Display  => &[0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x07],
            Self::Merged   => &[0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04],
            Self::Internal => &[0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08],
        }
    }

    /// EDID source uses a single HID packet (no commit needed).
    /// All modes use sub-command 0x12 with values 0x00/0x01/0x02.
    pub fn payload_4ks(&self) -> [u8; HID_PACKET_SIZE] {
        let value = match self {
            Self::Merged   => 0x00,
            Self::Display  => 0x01,
            Self::Internal => 0x02,
        };
        hid_write_packet(SUBCMD_EDID_MODE, value)
    }
}

// ---------------------------------------------------------------------------
// HDR Tone Mapping
// ---------------------------------------------------------------------------

/// HDR tone mapping toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrToneMapping {
    On,
    Off,
}

impl fmt::Display for HdrToneMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "On"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for HdrToneMapping {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Ok(Self::On),
            "off" | "false" | "0" => Ok(Self::Off),
            _ => Err(()),
        }
    }
}

impl HdrToneMapping {
    pub const VALID_VALUES: &str = "on, off";

    pub fn payload_4kx(&self) -> &'static [u8] {
        match self {
            Self::On  => &[0xa1, 0x07, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x01, 0x38],
            Self::Off => &[0xa1, 0x07, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x39],
        }
    }

    pub fn payload_4ks(&self) -> [u8; HID_PACKET_SIZE] {
        let value = match self {
            Self::On  => 0x01,
            Self::Off => 0x00,
        };
        hid_write_packet(SUBCMD_HDR_TONEMAPPING, value)
    }
}

// ---------------------------------------------------------------------------
// Custom EDID (4K X only)
// ---------------------------------------------------------------------------

/// Custom EDID preset toggle (4K X only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomEdidMode {
    Off,
    On,
}

impl fmt::Display for CustomEdidMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "On"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for CustomEdidMode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Ok(Self::On),
            "off" | "false" | "0" => Ok(Self::Off),
            _ => Err(()),
        }
    }
}

impl CustomEdidMode {
    pub const VALID_VALUES: &str = "on, off";

    pub fn payload_4kx(&self) -> &'static [u8] {
        match self {
            Self::Off => &[0xa1, 0x0a, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x81],
            Self::On  => &[0xa1, 0x0a, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0x80],
        }
    }
}

// ---------------------------------------------------------------------------
// Audio Input (4K S only)
// ---------------------------------------------------------------------------

/// Audio input source selection (4K S only, HID sub-cmd 0x08).
///
/// Discovered via decompilation of EGAVDeviceSupport.dll.
/// Function: `CCamLinkSupport::SetAudioInputSelection`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioInput {
    /// HDMI embedded audio (default).
    Embedded,
    /// Analog/line-in audio.
    Analog,
}

impl fmt::Display for AudioInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedded => write!(f, "Embedded (HDMI)"),
            Self::Analog => write!(f, "Analog (line-in)"),
        }
    }
}

impl FromStr for AudioInput {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "embedded" | "hdmi" | "digital" => Ok(Self::Embedded),
            "analog" | "line" | "linein" => Ok(Self::Analog),
            _ => Err(()),
        }
    }
}

impl AudioInput {
    pub const VALID_VALUES: &str = "embedded, analog";

    pub fn payload_4ks(&self) -> [u8; HID_PACKET_SIZE] {
        let value = match self {
            Self::Embedded => 0x00,
            Self::Analog   => 0x01,
        };
        hid_write_packet(SUBCMD_AUDIO_INPUT, value)
    }
}

// ---------------------------------------------------------------------------
// Video Scaler (4K S only)
// ---------------------------------------------------------------------------

/// Video scaler toggle (4K S only, HID sub-cmd 0x19).
///
/// Discovered via decompilation of EGAVDeviceSupport.dll.
/// Function: `CCamLinkSupport::SetVideoScalerEnabled`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoScaler {
    On,
    Off,
}

impl fmt::Display for VideoScaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "On"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for VideoScaler {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Ok(Self::On),
            "off" | "false" | "0" => Ok(Self::Off),
            _ => Err(()),
        }
    }
}

impl VideoScaler {
    pub const VALID_VALUES: &str = "on, off";

    pub fn payload_4ks(&self) -> [u8; HID_PACKET_SIZE] {
        let value = match self {
            Self::On  => 0x01,
            Self::Off => 0x00,
        };
        hid_write_packet(SUBCMD_VIDEO_SCALER, value)
    }
}

// ---------------------------------------------------------------------------
// USB Speed (4K X only)
// ---------------------------------------------------------------------------

/// USB speed mode (4K X only, AT command 0x8e).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    FiveGbps,
    TenGbps,
}

impl fmt::Display for UsbSpeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FiveGbps => write!(f, "5Gbps"),
            Self::TenGbps => write!(f, "10Gbps"),
        }
    }
}

impl FromStr for UsbSpeed {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "5g" | "5gbps" | "5" => Ok(Self::FiveGbps),
            "10g" | "10gbps" | "10" => Ok(Self::TenGbps),
            _ => Err(()),
        }
    }
}

impl UsbSpeed {
    pub const VALID_VALUES: &str = "5g, 10g";

    /// AT command 0x8e input: 8-byte payload.
    ///
    /// From RTICE_SDK_X64 decompilation of `AT_USB_Set_Force_Speed`:
    /// ```c
    /// local_418 = 1;         // bytes 0-3: constant 0x00000001 (u32 LE)
    /// local_414 = param_1;   // bytes 4-7: speed value (u32 LE)
    /// rtk_sendATCommand(0x8e, &local_418, local_218, 8);
    /// ```
    /// Speed values (from EGAVDeviceSupport `SetUseUSBSpeed10G`):
    ///   `AT_USB_Set_Force_Speed(-(param_2 != '\0') & 3)` → 0x00=5Gbps, 0x03=10Gbps.
    pub fn at_input(&self) -> [u8; 8] {
        match self {
            Self::FiveGbps => [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            Self::TenGbps  => [0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00],
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edid_range_from_str() {
        assert_eq!("expand".parse(), Ok(EdidRangePolicy::Expand));
        assert_eq!("full".parse(), Ok(EdidRangePolicy::Expand));
        assert_eq!("shrink".parse(), Ok(EdidRangePolicy::Shrink));
        assert_eq!("limited".parse(), Ok(EdidRangePolicy::Shrink));
        assert_eq!("auto".parse(), Ok(EdidRangePolicy::Auto));
        assert_eq!("AUTO".parse(), Ok(EdidRangePolicy::Auto));
        assert!("bogus".parse::<EdidRangePolicy>().is_err());
    }

    #[test]
    fn edid_range_display() {
        assert_eq!(EdidRangePolicy::Expand.to_string(), "Expand (Full)");
        assert_eq!(EdidRangePolicy::Shrink.to_string(), "Shrink (Limited)");
        assert_eq!(EdidRangePolicy::Auto.to_string(), "Auto");
    }

    #[test]
    fn edid_source_from_str() {
        assert_eq!("display".parse(), Ok(EdidSource::Display));
        assert_eq!("merged".parse(), Ok(EdidSource::Merged));
        assert_eq!("internal".parse(), Ok(EdidSource::Internal));
        assert!("nope".parse::<EdidSource>().is_err());
    }

    #[test]
    fn hdr_from_str() {
        assert_eq!("on".parse(), Ok(HdrToneMapping::On));
        assert_eq!("true".parse(), Ok(HdrToneMapping::On));
        assert_eq!("1".parse(), Ok(HdrToneMapping::On));
        assert_eq!("off".parse(), Ok(HdrToneMapping::Off));
        assert_eq!("false".parse(), Ok(HdrToneMapping::Off));
        assert_eq!("0".parse(), Ok(HdrToneMapping::Off));
    }

    #[test]
    fn audio_input_from_str() {
        assert_eq!("embedded".parse(), Ok(AudioInput::Embedded));
        assert_eq!("hdmi".parse(), Ok(AudioInput::Embedded));
        assert_eq!("analog".parse(), Ok(AudioInput::Analog));
        assert_eq!("line".parse(), Ok(AudioInput::Analog));
    }

    #[test]
    fn usb_speed_from_str() {
        assert_eq!("5g".parse(), Ok(UsbSpeed::FiveGbps));
        assert_eq!("10g".parse(), Ok(UsbSpeed::TenGbps));
        assert_eq!("5gbps".parse(), Ok(UsbSpeed::FiveGbps));
        assert!("20g".parse::<UsbSpeed>().is_err());
    }

    #[test]
    fn payload_4kx_starts_with_a1() {
        assert_eq!(EdidRangePolicy::Auto.payload_4kx()[0], 0xa1);
        assert_eq!(EdidRangePolicy::Expand.payload_4kx()[0], 0xa1);
        assert_eq!(EdidSource::Display.payload_4kx()[0], 0xa1);
        assert_eq!(HdrToneMapping::On.payload_4kx()[0], 0xa1);
        assert_eq!(CustomEdidMode::Off.payload_4kx()[0], 0xa1);
    }

    #[test]
    fn hid_packets_are_correct_size() {
        assert_eq!(EdidRangePolicy::Expand.payload_4ks().len(), HID_PACKET_SIZE);
        assert_eq!(EdidSource::Display.payload_4ks().len(), HID_PACKET_SIZE);
        assert_eq!(AudioInput::Analog.payload_4ks().len(), HID_PACKET_SIZE);
        assert_eq!(HdrToneMapping::On.payload_4ks().len(), HID_PACKET_SIZE);
        assert_eq!(VideoScaler::Off.payload_4ks().len(), HID_PACKET_SIZE);
    }

    #[test]
    fn hid_packets_have_correct_header() {
        let pkt = HdrToneMapping::On.payload_4ks();
        assert_eq!(&pkt[..5], &HID_WRITE_HEADER);
        assert_eq!(pkt[5], SUBCMD_HDR_TONEMAPPING);
        assert_eq!(pkt[6], 0x01); // On = 0x01
    }

    #[test]
    fn hid_packets_correct_subcmds() {
        let pkt = EdidRangePolicy::Expand.payload_4ks();
        assert_eq!(pkt[5], SUBCMD_COLOR_RANGE);
        assert_eq!(pkt[6], 0x01);

        let pkt = EdidSource::Internal.payload_4ks();
        assert_eq!(pkt[5], SUBCMD_EDID_MODE);
        assert_eq!(pkt[6], 0x02);

        let pkt = AudioInput::Analog.payload_4ks();
        assert_eq!(pkt[5], SUBCMD_AUDIO_INPUT);
        assert_eq!(pkt[6], 0x01);

        let pkt = VideoScaler::On.payload_4ks();
        assert_eq!(pkt[5], SUBCMD_VIDEO_SCALER);
        assert_eq!(pkt[6], 0x01);
    }

    #[test]
    fn hid_packets_zero_padded() {
        let pkt = VideoScaler::On.payload_4ks();
        // Bytes after the header + sub_cmd + value should all be zero
        assert!(pkt[7..].iter().all(|&b| b == 0));
    }

    #[test]
    fn usb_speed_at_input() {
        let five = UsbSpeed::FiveGbps.at_input();
        // Bytes 0-3: constant 1 (u32 LE), Bytes 4-7: speed 0x00=5Gbps
        assert_eq!(five, [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let ten = UsbSpeed::TenGbps.at_input();
        // Bytes 0-3: constant 1 (u32 LE), Bytes 4-7: speed 0x03=10Gbps
        assert_eq!(ten, [0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn display_device_model() {
        assert_eq!(DeviceModel::Elgato4KX.to_string(), "4K X");
        assert_eq!(DeviceModel::Elgato4KS.to_string(), "4K S");
    }
}
