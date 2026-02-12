use crate::hid::HID_PACKET_SIZE;

#[derive(Debug, Clone, Copy)]
pub enum DeviceModel {
    Elgato4KX,
    Elgato4KS,
}

/// EDID Range Policy (controls color range quantization)
///
/// Despite the CLI flag name `--hdmi-range`, this actually controls the
/// EDID Range Policy via the `a1 08 ... 7c` payload family (11 bytes).
/// The official Elgato software labels this as "HDMI Color Range" in the UI.
#[derive(Debug, Clone, Copy)]
pub enum EdidRangePolicy {
    Expand,  // Full (0-255)
    Shrink,  // Limited (16-235)
    Auto,
}

#[derive(Debug, Clone, Copy)]
pub enum EdidSource {
    Display,
    Merged,
    Internal,
}

#[derive(Debug, Clone, Copy)]
pub enum HdrToneMapping {
    On,
    Off,
}

#[derive(Debug, Clone, Copy)]
pub enum CustomEdidMode {
    Off,
    On,
}

/// Audio input source selection (4K S only via HID sub-cmd 0x08).
///
/// Discovered via Ghidra decompilation of EGAVDeviceSupport.dll.
/// Function: CCamLinkSupport::SetAudioInputSelection
/// HID command: 0x55 0x08, 1 byte data.
#[derive(Debug, Clone, Copy)]
pub enum AudioInput {
    Embedded, // 0 = HDMI embedded audio (default)
    Analog,   // 1 = Analog/line-in audio
}

/// Video scaler toggle (4K S only via HID sub-cmd 0x19).
///
/// Discovered via Ghidra decompilation of EGAVDeviceSupport.dll.
/// Function: CCamLinkSupport::SetVideoScalerEnabled
/// HID command: 0x55 0x19, 1 byte data.
#[derive(Debug, Clone, Copy)]
pub enum VideoScaler {
    On,
    Off,
}

#[derive(Debug, Clone, Copy)]
pub enum UsbSpeed {
    FiveGbps,
    TenGbps,
}

// --- EDID Range Policy payloads ---

impl EdidRangePolicy {
    pub fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::Auto   => vec![0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x00, 0xda],
            Self::Expand => vec![0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x03, 0xd7],
            Self::Shrink => vec![0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x04, 0xd6],
        }
    }

    pub fn payload_4ks(&self) -> (Vec<u8>, Vec<u8>) {
        let mut pkt1 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x0b];
        pkt1.push(match self {
            Self::Auto   => 0x00,
            Self::Expand => 0x01,
            Self::Shrink => 0x02,
        });
        pkt1.resize(HID_PACKET_SIZE, 0x00);

        let mut pkt2 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x13, 0x01];
        pkt2.resize(HID_PACKET_SIZE, 0x00);

        (pkt1, pkt2)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "expand" | "full" => Some(Self::Expand),
            "shrink" | "limited" => Some(Self::Shrink),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

// --- EDID Source payloads ---

impl EdidSource {
    pub fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::Display  => vec![0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x07],
            Self::Merged   => vec![0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04],
            Self::Internal => vec![0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08],
        }
    }

    pub fn payload_4ks(&self) -> Vec<u8> {
        let mut pkt = vec![0x06, 0x06, 0x06, 0x55, 0x02];
        match self {
            Self::Merged   => pkt.extend_from_slice(&[0x12, 0x00]),
            Self::Display  => pkt.extend_from_slice(&[0x12, 0x01]),
            Self::Internal => pkt.extend_from_slice(&[0x13, 0x01]),
        }
        pkt.resize(HID_PACKET_SIZE, 0x00);
        pkt
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "display" => Some(Self::Display),
            "merged" => Some(Self::Merged),
            "internal" => Some(Self::Internal),
            _ => None,
        }
    }
}

// --- HDR Tone Mapping payloads ---

impl HdrToneMapping {
    pub fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::On  => vec![0xa1, 0x07, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x01, 0x38],
            Self::Off => vec![0xa1, 0x07, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x39],
        }
    }

    pub fn payload_4ks(&self) -> (Vec<u8>, Vec<u8>) {
        let mut pkt1 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x0a];
        pkt1.push(match self {
            Self::On  => 0x01,
            Self::Off => 0x00,
        });
        pkt1.resize(HID_PACKET_SIZE, 0x00);

        let mut pkt2 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x13, 0x01];
        pkt2.resize(HID_PACKET_SIZE, 0x00);

        (pkt1, pkt2)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Some(Self::On),
            "off" | "false" | "0" => Some(Self::Off),
            _ => None,
        }
    }
}

// --- Custom EDID payloads ---

impl CustomEdidMode {
    pub fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::Off => vec![0xa1, 0x0a, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x81],
            Self::On  => vec![0xa1, 0x0a, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0x80],
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Some(Self::On),
            "off" | "false" | "0" => Some(Self::Off),
            _ => None,
        }
    }
}

// --- Audio Input payloads (4K S only) ---

impl AudioInput {
    /// HID sub-command 0x08: audio input selection.
    /// The DLL shows: param_2 == 2 maps to value 1 (analog), else 0 (embedded).
    pub fn payload_4ks(&self) -> (Vec<u8>, Vec<u8>) {
        let mut pkt1 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x08];
        pkt1.push(match self {
            Self::Embedded => 0x00,
            Self::Analog   => 0x01,
        });
        pkt1.resize(HID_PACKET_SIZE, 0x00);

        let mut pkt2 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x13, 0x01];
        pkt2.resize(HID_PACKET_SIZE, 0x00);

        (pkt1, pkt2)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "embedded" | "hdmi" | "digital" => Some(Self::Embedded),
            "analog" | "line" | "linein" => Some(Self::Analog),
            _ => None,
        }
    }
}

// --- Video Scaler payloads (4K S only) ---

impl VideoScaler {
    /// HID sub-command 0x19: video scaler on/off.
    pub fn payload_4ks(&self) -> (Vec<u8>, Vec<u8>) {
        let mut pkt1 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x19];
        pkt1.push(match self {
            Self::On  => 0x01,
            Self::Off => 0x00,
        });
        pkt1.resize(HID_PACKET_SIZE, 0x00);

        let mut pkt2 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x13, 0x01];
        pkt2.resize(HID_PACKET_SIZE, 0x00);

        (pkt1, pkt2)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Some(Self::On),
            "off" | "false" | "0" => Some(Self::Off),
            _ => None,
        }
    }
}

// --- USB Speed ---

impl UsbSpeed {
    /// AT command 0x8e input: 8 bytes [01,00,00,00, speed,00,00,00]
    pub fn at_input(&self) -> Vec<u8> {
        let speed_byte: u8 = match self {
            Self::FiveGbps => 0x00,
            Self::TenGbps  => 0x01,
        };
        vec![0x01, 0x00, 0x00, 0x00, speed_byte, 0x00, 0x00, 0x00]
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "5g" | "5gbps" | "5" => Some(Self::FiveGbps),
            "10g" | "10gbps" | "10" => Some(Self::TenGbps),
            _ => None,
        }
    }
}
