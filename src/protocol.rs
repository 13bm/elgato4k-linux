//! Protocol constants for Elgato 4K X (UVC) and 4K S (HID) devices.
//!
//! All magic numbers, sub-command IDs, and payload templates are defined here
//! so the rest of the codebase references named constants instead of raw hex.

// ---------------------------------------------------------------------------
// USB device identifiers
// ---------------------------------------------------------------------------

/// Elgato vendor ID (Corsair).
pub const VENDOR_ID: u16 = 0x0fd9;

/// 4K X product IDs — the PID changes depending on USB speed mode.
pub const PIDS_4KX: &[(u16, &str)] = &[
    (0x009b, "10Gbps / SuperSpeed+"),
    (0x009c, "5Gbps / SuperSpeed"),
    (0x009d, "USB 2.0"),
];

/// 4K S product IDs.
pub const PIDS_4KS: &[(u16, &str)] = &[
    (0x00af, "USB 3.0"),
    (0x00ae, "USB 2.0"),
];

// ---------------------------------------------------------------------------
// HID protocol (4K S) — SET_REPORT / GET_REPORT on Interface 7
// ---------------------------------------------------------------------------

/// bmRequestType for host-to-device class request.
pub const HID_REQUEST_TYPE_OUT: u8 = 0x21;
/// bmRequestType for device-to-host class request.
pub const HID_REQUEST_TYPE_IN: u8 = 0xA1;
/// HID SET_REPORT bRequest.
pub const HID_SET_REPORT: u8 = 0x09;
/// HID GET_REPORT bRequest.
pub const HID_GET_REPORT: u8 = 0x01;
/// wValue for Output Report (Report Type=Output 0x02, Report ID=0x06).
pub const HID_REPORT_VALUE_OUTPUT: u16 = 0x0206;
/// wValue for Input Report (Report Type=Input 0x01, Report ID=0x06).
pub const HID_REPORT_VALUE_INPUT: u16 = 0x0106;
/// HID interface number on the 4K S.
pub const HID_INTERFACE: u16 = 7;
/// Fixed HID report size (all packets are zero-padded to 255 bytes).
pub const HID_PACKET_SIZE: usize = 255;
/// Report ID prepended to every HID packet.
pub const HID_REPORT_ID: u8 = 0x06;

/// HID write packet header: [report_id, 0x06, 0x06, 0x55, 0x02].
/// Byte 0: Report ID (0x06)
/// Bytes 1-2: Magic preamble (0x06, 0x06) — possibly protocol version
/// Byte 3: Command class (0x55 = settings)
/// Byte 4: Write indicator (0x02)
pub const HID_WRITE_HEADER: [u8; 5] = [0x06, 0x06, 0x06, 0x55, 0x02];

/// HID read command byte (cmd field in read request packets).
pub const HID_READ_CMD: u8 = 0x55;

// ---------------------------------------------------------------------------
// HID sub-command IDs (4K S)
// From EGAVDeviceSupport.dll decompilation (CCamLinkSupport class).
// ---------------------------------------------------------------------------

/// Firmware version read — `GetFirmwareVersion`, 8 bytes.
pub const SUBCMD_FIRMWARE_VERSION: u8 = 0x02;
/// Audio input selection — `GetAudioInputSelection` / `SetAudioInputSelection`, 1 byte.
pub const SUBCMD_AUDIO_INPUT: u8 = 0x08;
/// HDR tone mapping — `GetHDRTonemappingEnabled` / `SetHDRTonemappingEnabled`, 1 byte.
pub const SUBCMD_HDR_TONEMAPPING: u8 = 0x0a;
/// HDMI color range — `GetVideoColorRange` / `SetVideoColorRange`, 1 byte.
pub const SUBCMD_COLOR_RANGE: u8 = 0x0b;
/// EDID mode — `GetEDIDMode` / `SetEDIDMode`, 1 byte.
pub const SUBCMD_EDID_MODE: u8 = 0x12;
/// Commit/apply — write 0x13 0x01 as second packet to apply changes.
pub const SUBCMD_COMMIT: u8 = 0x13;
/// Video scaler — `GetVideoScalerEnabled` / `SetVideoScalerEnabled`, 1 byte.
pub const SUBCMD_VIDEO_SCALER: u8 = 0x19;

// ---------------------------------------------------------------------------
// UVC Extension Unit protocol (4K X)
// ---------------------------------------------------------------------------

/// bmRequestType for UVC class request (host-to-device).
pub const UVC_REQUEST_TYPE_OUT: u8 = 0x21;
/// bmRequestType for UVC class request (device-to-host).
pub const UVC_REQUEST_TYPE_IN: u8 = 0xA1;
/// SET_CUR bRequest.
pub const UVC_SET_CUR: u8 = 0x01;
/// GET_CUR bRequest.
pub const UVC_GET_CUR: u8 = 0x81;
/// GET_LEN bRequest — queries the current descriptor length for a selector.
/// The device dynamically changes this after a SET_CUR to reflect the response size.
pub const UVC_GET_LEN: u8 = 0x85;
/// UVC interface number for Extension Unit #4.
pub const UVC_INTERFACE: u16 = 0;
/// Extension Unit entity ID (XU #4, GUID 961073c7-49f7-44f2-ab42-e940405940c2).
pub const UVC_ENTITY_ID: u16 = 4;
/// Selector for trigger/length data.
pub const UVC_SELECTOR_TRIGGER: u16 = 0x02;
/// Selector for payload/value data.
pub const UVC_SELECTOR_VALUE: u16 = 0x01;

// ---------------------------------------------------------------------------
// UVC sub-command IDs (byte[4] in a1 06 family payloads)
// ---------------------------------------------------------------------------

/// Sub-command: read firmware version (AT_Get_Customer_Ver).
pub const UVC_SUBCMD_FIRMWARE_VERSION: u8 = 0x77;
/// Sub-command: read EDID Range Policy / HDMI color range (family 0x07, 10-byte probe).
/// Response byte[4] mirrors the `0x7c` write byte[9]: 0x00=Auto, 0x03=Expand, 0x04=Shrink.
pub const UVC_SUBCMD_EDID_RANGE_READ: u8 = 0x91;
/// Sub-command: read HDR tone mapping state (family 0x06).
/// Response byte[4]: 0x01=On, 0x00=Off.
pub const UVC_SUBCMD_HDR_READ: u8 = 0x90;
/// AT command ID for setting USB speed (4K X only, used with send_at_command).
/// From RTICE_SDK_X64: `rtk_sendATCommand(0x8e, &local_418, local_218, 8)`.
/// Payload: `[01 00 00 00, speed_value 00 00 00]` where speed=0x00 (5G) or 0x03 (10G).
pub const AT_CMD_SET_USB_SPEED: u32 = 0x8e;

// ---------------------------------------------------------------------------
// BCD validation constants (for firmware version decoding)
// ---------------------------------------------------------------------------

/// Maximum valid BCD month (0x12 = December).
pub const BCD_MAX_MONTH: u8 = 0x12;
/// Maximum valid BCD day (0x31 = 31st).
pub const BCD_MAX_DAY: u8 = 0x31;

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

/// Default USB control transfer timeout.
pub const USB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
/// Inter-packet delay for two-packet HID writes.
pub const HID_INTER_PACKET_DELAY: std::time::Duration = std::time::Duration::from_millis(1);
/// Delay after HID read request before GET_REPORT.
pub const HID_READ_DELAY: std::time::Duration = std::time::Duration::from_millis(10);
/// Delay between consecutive setting changes.
pub const SETTING_APPLY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);
