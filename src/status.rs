//! Status reading and firmware version decoding for the 4K X and 4K S.
//!
//! **4K X:** Settings are stored in the UVC Extension Unit and read back via
//! GET_CUR at specific payload lengths. Firmware version uses AT command 0x77.
//!
//! **4K S:** Settings are read via HID GET_REPORT using the ReadI2cData protocol:
//!   `cmd=0x55, sub_cmd=feature_id, read_len` bytes.
//! Discovered by decompiling EGAVDeviceSupport.dll (CCamLinkSupport class).

use std::fmt;

use crate::device::ElgatoDevice;
use crate::error::ElgatoError;
use crate::protocol::*;
use crate::settings::{
    AudioInput, DeviceModel, EdidRangePolicy, EdidSource, HdrToneMapping, VideoScaler,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A value read from the device that may be a known enum variant or an
/// unrecognized raw byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadValue<T> {
    /// A recognized, strongly-typed value.
    Known(T),
    /// An unrecognized byte from the device.
    Unknown(u8),
}

impl<T: fmt::Display> fmt::Display for ReadValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(v) => write!(f, "{}", v),
            Self::Unknown(b) => write!(f, "Unknown (0x{:02x})", b),
        }
    }
}

/// USB speed mode reported by the device (derived from product ID).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeedStatus {
    /// USB 2.0 High-Speed (480 Mbps).
    Usb2,
    /// 5 Gbps SuperSpeed (USB 3.0).
    FiveGbps,
    /// 10 Gbps SuperSpeed+ (USB 3.1 Gen 2).
    TenGbps,
}

impl fmt::Display for UsbSpeedStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usb2 => write!(f, "USB 2.0 (480 Mbps)"),
            Self::FiveGbps => write!(f, "5Gbps (SuperSpeed)"),
            Self::TenGbps => write!(f, "10Gbps (SuperSpeed+)"),
        }
    }
}

/// Custom EDID preset state as read from the device (4K X only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomEdidStatus {
    /// Custom EDID is disabled.
    Off,
    /// Custom EDID is enabled with the given preset index.
    On {
        /// The preset index selected on the device.
        preset_index: u8,
    },
}

impl fmt::Display for CustomEdidStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::On { preset_index } => write!(f, "On (preset index {})", preset_index),
        }
    }
}

/// All readable settings from a device.
///
/// Fields are `None` when a setting is not applicable to the device model
/// (e.g. `audio_input` is only available on the 4K S) or when the device
/// returned an unexpected/unreadable response.
///
/// **4K X:** Firmware version, USB speed, HDMI color range, and HDR tone
/// mapping are readable. EDID source, custom EDID, audio input, and video
/// scaler are not readable.
#[derive(Debug, Clone)]
pub struct DeviceStatus {
    /// Firmware version string (e.g. "25.02.10").
    pub firmware_version: String,
    /// USB speed mode (4K X only).
    pub usb_speed: Option<ReadValue<UsbSpeedStatus>>,
    /// HDMI color range (4K X via AT cmd 0x91 family 0x07; 4K S via HID).
    pub hdmi_color_range: Option<ReadValue<EdidRangePolicy>>,
    /// HDR tone mapping (4K X via AT cmd 0x90; 4K S via HID).
    pub hdr_tone_mapping: Option<ReadValue<HdrToneMapping>>,
    /// EDID source selection (4K S only; not readable on 4K X).
    pub edid_source: Option<ReadValue<EdidSource>>,
    /// Custom EDID preset state (not currently readable).
    pub custom_edid: Option<CustomEdidStatus>,
    /// Audio input source (4K S only).
    pub audio_input: Option<ReadValue<AudioInput>>,
    /// Video scaler state (4K S only).
    pub video_scaler: Option<ReadValue<VideoScaler>>,
}

impl fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Firmware version: {}", self.firmware_version)?;
        if let Some(v) = &self.usb_speed {
            writeln!(f, "USB speed: {}", v)?;
        }
        if let Some(v) = &self.hdmi_color_range {
            writeln!(f, "HDMI color range: {}", v)?;
        }
        if let Some(v) = &self.hdr_tone_mapping {
            writeln!(f, "HDR tone mapping: {}", v)?;
        }
        if let Some(v) = &self.edid_source {
            writeln!(f, "EDID source: {}", v)?;
        }
        if let Some(v) = &self.custom_edid {
            writeln!(f, "Custom EDID: {}", v)?;
        }
        if let Some(v) = &self.audio_input {
            writeln!(f, "Audio input: {}", v)?;
        }
        if let Some(v) = &self.video_scaler {
            writeln!(f, "Video scaler: {}", v)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// HID decode functions (4K S)
// ---------------------------------------------------------------------------

/// Decode HDR tone mapping byte.
fn decode_hdr(v: u8) -> ReadValue<HdrToneMapping> {
    match v {
        0x01 => ReadValue::Known(HdrToneMapping::On),
        0x00 => ReadValue::Known(HdrToneMapping::Off),
        _ => ReadValue::Unknown(v),
    }
}

/// Decode HDMI color range byte.
fn decode_color_range(v: u8) -> ReadValue<EdidRangePolicy> {
    match v {
        0x00 => ReadValue::Known(EdidRangePolicy::Auto),
        0x01 => ReadValue::Known(EdidRangePolicy::Expand),
        0x02 => ReadValue::Known(EdidRangePolicy::Shrink),
        _ => ReadValue::Unknown(v),
    }
}

/// Decode EDID mode byte.
fn decode_edid_mode(v: u8) -> ReadValue<EdidSource> {
    match v {
        0x00 => ReadValue::Known(EdidSource::Merged),
        0x01 => ReadValue::Known(EdidSource::Display),
        0x02 => ReadValue::Known(EdidSource::Internal),
        _ => ReadValue::Unknown(v),
    }
}

/// Decode audio input byte.
fn decode_audio_input(v: u8) -> ReadValue<AudioInput> {
    match v {
        0x00 | 0x01 => ReadValue::Known(AudioInput::Embedded),
        0x03 => ReadValue::Known(AudioInput::Analog),
        _ => ReadValue::Unknown(v),
    }
}

/// Decode video scaler byte.
fn decode_video_scaler(v: u8) -> ReadValue<VideoScaler> {
    match v {
        0x01 => ReadValue::Known(VideoScaler::On),
        0x00 => ReadValue::Known(VideoScaler::Off),
        _ => ReadValue::Unknown(v),
    }
}

// ---------------------------------------------------------------------------
// ElgatoDevice status methods
// ---------------------------------------------------------------------------

impl ElgatoDevice {
    // --- Public data-returning API (for library consumers) ---

    /// Read all available settings from the device.
    ///
    /// Returns a [`DeviceStatus`] struct with all readable fields populated.
    /// Fields that are not applicable to the device model are set to `None`.
    pub fn read_status(&self) -> Result<DeviceStatus, ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => self.read_status_4kx(),
            DeviceModel::Elgato4KS => self.read_status_4ks(),
        }
    }

    /// Read the firmware version as a string.
    ///
    /// - **4K X:** AT command 0x77 via `a1 06` family probe. Response is 133 bytes
    ///   with ASCII version string at bytes 4–9 (e.g. "250210" = 25.02.10).
    /// - **4K S:** HID read command 0x55/0x02 (BCD DateThreeBytes).
    pub fn read_firmware_version(&self) -> Result<String, ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => {
                match self.read_at_command(UVC_SUBCMD_FIRMWARE_VERSION) {
                    Ok(data) if data.len() >= 10 => {
                        Ok(Self::format_firmware_version_4kx(&data))
                    }
                    Ok(data) => {
                        Ok(format!("Unexpected response ({} bytes): {:02x?}", data.len(), data))
                    }
                    Err(e) => {
                        Ok(format!("Failed to read: {}", e))
                    }
                }
            }
            DeviceModel::Elgato4KS => {
                match self.read_hid_data(HID_READ_CMD, SUBCMD_FIRMWARE_VERSION, 8) {
                    Ok(data) if data.len() >= 5 => {
                        Ok(Self::format_firmware_version_4ks(&data))
                    }
                    Ok(data) => {
                        Ok(format!("Unexpected response ({} bytes): {:02x?}", data.len(), data))
                    }
                    Err(e) => {
                        Ok(format!("Failed to read: {}", e))
                    }
                }
            }
        }
    }

    // --- Internal: firmware version formatting ---

    /// Format firmware version from AT command 0x77 response (4K X).
    ///
    /// The 133-byte response has header `a1 80 81 00` then ASCII YYMMDD at
    /// bytes 4–9 (e.g. "250210" = firmware version 25.02.10).
    fn format_firmware_version_4kx(data: &[u8]) -> String {
        // Extract ASCII version string starting at byte 4
        let version_bytes = &data[4..];
        // Find end of ASCII digits
        let end = version_bytes.iter().position(|&b| b == 0 || !b.is_ascii_digit()).unwrap_or(version_bytes.len());
        let version_str = std::str::from_utf8(&version_bytes[..end]).unwrap_or("");

        if version_str.is_empty() || version_str == "0" {
            return format!("Unknown (raw: {:02x?})", &data[..std::cmp::min(16, data.len())]);
        }

        // Parse YYMMDD
        if let Ok(version) = version_str.parse::<u32>() {
            let yy = version / 10000;
            let mm = (version / 100) % 100;
            let dd = version % 100;

            if (1..=12).contains(&mm) && (1..=31).contains(&dd) {
                return format!("{:02}.{:02}.{:02}", yy, mm, dd);
            }
        }

        format!("Raw: {}", version_str)
    }

    /// Format firmware version from HID response (4K S).
    ///
    /// The 8-byte response contains the version in bytes 3–5 as DateThreeBytes
    /// (versionFormat 1): `[YY, MM, DD]` in BCD encoding.
    fn format_firmware_version_4ks(data: &[u8]) -> String {
        let yy = data[3];
        let mm = data[4];
        let dd = data[5];

        if yy == 0 && mm == 0 && dd == 0 {
            return "Unknown (no version reported)".to_string();
        }

        if (1..=BCD_MAX_MONTH).contains(&mm) && (1..=BCD_MAX_DAY).contains(&dd) {
            format!("{:02x}.{:02x}.{:02x}", yy, mm, dd)
        } else {
            format!("Raw: {:02x?}", &data[..std::cmp::min(8, data.len())])
        }
    }

    // --- Internal: generic typed readers ---

    /// Read a single HID status field and decode it via the provided function.
    fn read_hid_typed<T>(&self, sub_cmd: u8, decode: fn(u8) -> ReadValue<T>) -> Option<ReadValue<T>> {
        match self.read_hid_data(HID_READ_CMD, sub_cmd, 1) {
            Ok(data) if !data.is_empty() => Some(decode(data[0])),
            _ => None,
        }
    }

    // --- Internal: 4K S status reading ---

    /// Read all 4K S settings into a DeviceStatus.
    fn read_status_4ks(&self) -> Result<DeviceStatus, ElgatoError> {
        let firmware_version = self.read_firmware_version()?;

        Ok(DeviceStatus {
            firmware_version,
            usb_speed: None,
            hdr_tone_mapping: self.read_hid_typed(SUBCMD_HDR_TONEMAPPING, decode_hdr),
            hdmi_color_range: self.read_hid_typed(SUBCMD_COLOR_RANGE, decode_color_range),
            edid_source: self.read_hid_typed(SUBCMD_EDID_MODE, decode_edid_mode),
            custom_edid: None,
            audio_input: self.read_hid_typed(SUBCMD_AUDIO_INPUT, decode_audio_input),
            video_scaler: self.read_hid_typed(SUBCMD_VIDEO_SCALER, decode_video_scaler),
        })
    }

    // --- Internal: 4K X status reading ---

    /// Determine USB speed from the device's Product ID.
    ///
    /// The 4K X uses a different PID for each USB speed mode:
    /// - 0x009b = 10 Gbps (SuperSpeed+)
    /// - 0x009c = 5 Gbps (SuperSpeed)
    /// - 0x009d = USB 2.0
    fn read_usb_speed_4kx(&self) -> Option<ReadValue<UsbSpeedStatus>> {
        Some(match self.pid {
            0x009b => ReadValue::Known(UsbSpeedStatus::TenGbps),
            0x009c => ReadValue::Known(UsbSpeedStatus::FiveGbps),
            0x009d => ReadValue::Known(UsbSpeedStatus::Usb2),
            _ => return None,
        })
    }

    /// Read HDMI color range (EDID Range Policy) from the 4K X via AT command 0x91.
    ///
    /// Uses the `a1 07` family (10-byte probe with param byte 0x01).
    /// Response byte[4] mirrors the `0x7c` write byte[9]:
    /// 0x00=Auto, 0x03=Expand, 0x04=Shrink.
    fn read_color_range_4kx(&self) -> Option<ReadValue<EdidRangePolicy>> {
        match self.read_at_command_family07(UVC_SUBCMD_EDID_RANGE_READ, 0x01) {
            Ok(data) if data.len() > 4 => {
                Some(match data[4] {
                    0x00 => ReadValue::Known(EdidRangePolicy::Auto),
                    0x03 => ReadValue::Known(EdidRangePolicy::Expand),
                    0x04 => ReadValue::Known(EdidRangePolicy::Shrink),
                    v => ReadValue::Unknown(v),
                })
            }
            _ => None,
        }
    }

    /// Read HDR tone mapping state from the 4K X via AT command 0x90.
    ///
    /// Standard `a1 06` family probe. Response byte[4]: 0x01=On, 0x00=Off.
    fn read_hdr_4kx(&self) -> Option<ReadValue<HdrToneMapping>> {
        match self.read_at_command(UVC_SUBCMD_HDR_READ) {
            Ok(data) if data.len() > 4 => Some(decode_hdr(data[4])),
            _ => None,
        }
    }

    /// Read all 4K X settings into a DeviceStatus.
    fn read_status_4kx(&self) -> Result<DeviceStatus, ElgatoError> {
        let firmware_version = self.read_firmware_version()?;
        let usb_speed = self.read_usb_speed_4kx();
        let hdmi_color_range = self.read_color_range_4kx();
        let hdr_tone_mapping = self.read_hdr_4kx();

        Ok(DeviceStatus {
            firmware_version,
            usb_speed,
            hdmi_color_range,
            hdr_tone_mapping,
            edid_source: None,
            custom_edid: None,
            audio_input: None,
            video_scaler: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- HID decode tests ---

    #[test]
    fn decode_hdr_values() {
        assert_eq!(decode_hdr(0x01), ReadValue::Known(HdrToneMapping::On));
        assert_eq!(decode_hdr(0x00), ReadValue::Known(HdrToneMapping::Off));
        assert_eq!(decode_hdr(0xff), ReadValue::Unknown(0xff));
    }

    #[test]
    fn decode_color_range_values() {
        assert_eq!(decode_color_range(0x00), ReadValue::Known(EdidRangePolicy::Auto));
        assert_eq!(decode_color_range(0x01), ReadValue::Known(EdidRangePolicy::Expand));
        assert_eq!(decode_color_range(0x02), ReadValue::Known(EdidRangePolicy::Shrink));
        assert_eq!(decode_color_range(0x99), ReadValue::Unknown(0x99));
    }

    #[test]
    fn decode_edid_mode_values() {
        assert_eq!(decode_edid_mode(0x00), ReadValue::Known(EdidSource::Merged));
        assert_eq!(decode_edid_mode(0x01), ReadValue::Known(EdidSource::Display));
        assert_eq!(decode_edid_mode(0x02), ReadValue::Known(EdidSource::Internal));
        assert_eq!(decode_edid_mode(0x03), ReadValue::Unknown(0x03));
    }

    #[test]
    fn decode_audio_input_values() {
        assert_eq!(decode_audio_input(0x00), ReadValue::Known(AudioInput::Embedded));
        assert_eq!(decode_audio_input(0x01), ReadValue::Known(AudioInput::Embedded));
        assert_eq!(decode_audio_input(0x03), ReadValue::Known(AudioInput::Analog));
        assert_eq!(decode_audio_input(0x02), ReadValue::Unknown(0x02));
    }

    #[test]
    fn decode_video_scaler_values() {
        assert_eq!(decode_video_scaler(0x01), ReadValue::Known(VideoScaler::On));
        assert_eq!(decode_video_scaler(0x00), ReadValue::Known(VideoScaler::Off));
        assert_eq!(decode_video_scaler(0x02), ReadValue::Unknown(0x02));
    }

    // --- Firmware version tests ---

    #[test]
    fn firmware_version_4kx_valid() {
        // Simulated 133-byte response: a1 80 81 00 "250210" + zeros
        let mut data = vec![0xa1, 0x80, 0x81, 0x00];
        data.extend_from_slice(b"250210");
        data.resize(133, 0x00);
        let result = ElgatoDevice::format_firmware_version_4kx(&data);
        assert_eq!(result, "25.02.10");
    }

    #[test]
    fn firmware_version_4kx_all_zero() {
        let mut data = vec![0xa1, 0x80, 0x81, 0x00];
        data.resize(133, 0x00);
        let result = ElgatoDevice::format_firmware_version_4kx(&data);
        assert!(result.starts_with("Unknown"));
    }

    #[test]
    fn firmware_version_4ks_valid() {
        let data = [0x00, 0x00, 0x00, 0x25, 0x0C, 0x03, 0x00, 0x00];
        let result = ElgatoDevice::format_firmware_version_4ks(&data);
        assert_eq!(result, "25.0c.03");
    }

    #[test]
    fn firmware_version_4ks_zero() {
        let data = [0x00; 8];
        let result = ElgatoDevice::format_firmware_version_4ks(&data);
        assert_eq!(result, "Unknown (no version reported)");
    }

    #[test]
    fn firmware_version_4ks_invalid_month() {
        let data = [0x00, 0x00, 0x00, 0x25, 0x15, 0x03, 0x00, 0x00];
        let result = ElgatoDevice::format_firmware_version_4ks(&data);
        assert!(result.starts_with("Raw:"));
    }

    // --- ReadValue Display tests ---

    #[test]
    fn read_value_display_known() {
        let v = ReadValue::Known(HdrToneMapping::On);
        assert_eq!(format!("{}", v), "On");
    }

    #[test]
    fn read_value_display_unknown() {
        let v: ReadValue<HdrToneMapping> = ReadValue::Unknown(0xab);
        assert_eq!(format!("{}", v), "Unknown (0xab)");
    }

    // --- CustomEdidStatus Display tests ---

    #[test]
    fn custom_edid_status_display() {
        assert_eq!(format!("{}", CustomEdidStatus::Off), "Off");
        assert_eq!(format!("{}", CustomEdidStatus::On { preset_index: 3 }), "On (preset index 3)");
    }

    // --- UsbSpeedStatus Display tests ---

    #[test]
    fn usb_speed_status_display() {
        assert_eq!(format!("{}", UsbSpeedStatus::Usb2), "USB 2.0 (480 Mbps)");
        assert_eq!(format!("{}", UsbSpeedStatus::FiveGbps), "5Gbps (SuperSpeed)");
        assert_eq!(format!("{}", UsbSpeedStatus::TenGbps), "10Gbps (SuperSpeed+)");
    }
}
