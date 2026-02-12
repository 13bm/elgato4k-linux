use crate::device::ElgatoDevice;
use crate::settings::DeviceModel;

/// Status reading and decoding for the 4K X and 4K S.
///
/// **4K X:** Settings are stored in the UVC Extension Unit and read back via
/// GET_CUR at specific payload lengths. Firmware version uses AT command 0x77.
///
/// **4K S:** Settings are read via HID GET_REPORT using the ReadI2cData protocol:
///   cmd=0x55, sub_cmd=feature_id, read_len bytes.
/// Discovered by decompiling EGAVDeviceSupport.dll (CCamLinkSupport class).
impl ElgatoDevice {
    pub fn get_status(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self.model {
            DeviceModel::Elgato4KX => self.get_status_4kx(),
            DeviceModel::Elgato4KS => self.get_status_4ks(),
        }
    }

    /// Read and display firmware version.
    /// - 4K X: AT command 0x77 (AT_Get_Customer_Ver), versionFormat 2 (YYMMDD decimal)
    /// - 4K S: HID read command 0x55/0x02, versionFormat 1 (DateThreeBytes BCD)
    pub fn get_firmware_version(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self.model {
            DeviceModel::Elgato4KX => {
                print!("Firmware version: ");
                match self.read_at_command(0x77, 128) {
                    Ok(data) if data.len() >= 4 => {
                        Self::decode_firmware_version_4kx(&data);
                    }
                    Ok(data) => {
                        println!("Unexpected response ({} bytes): {:02x?}", data.len(), data);
                    }
                    Err(e) => {
                        println!("Failed to read firmware version: {}", e);
                    }
                }
                Ok(())
            }
            DeviceModel::Elgato4KS => {
                // Read MCU firmware version via HID: cmd=0x55, sub_cmd=0x02, 8 bytes
                print!("MCU firmware version: ");
                match self.read_hid_data(0x55, 0x02, 8) {
                    Ok(data) if data.len() >= 5 => {
                        Self::decode_firmware_version_4ks(&data);
                    }
                    Ok(data) => {
                        println!("Unexpected response ({} bytes): {:02x?}", data.len(), data);
                    }
                    Err(e) => {
                        println!("Failed to read firmware version: {}", e);
                    }
                }
                Ok(())
            }
        }
    }

    /// Decode firmware version from AT command 0x77 response (4K X).
    ///
    /// versionFormat 2: YYMMDD packed decimal in first 4 bytes as u32 LE.
    /// e.g. 250210 = firmware version 25.02.10 (2025-02-10).
    fn decode_firmware_version_4kx(data: &[u8]) {
        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if version == 0 {
            println!("Unknown (no version reported)");
            return;
        }

        // versionFormat 2: YYMMDD packed decimal
        let yy = version / 10000;
        let mm = (version / 100) % 100;
        let dd = version % 100;

        if (1..=12).contains(&mm) && (1..=31).contains(&dd) {
            println!("{:02}.{:02}.{:02} (raw: {})", yy, mm, dd, version);
        } else {
            println!("Raw: {} (0x{:08x}, bytes: {:02x?})", version, version, &data[..std::cmp::min(16, data.len())]);
        }
    }

    /// Decode firmware version from HID response (4K S).
    ///
    /// The 8-byte response contains the version in bytes 3-5 as DateThreeBytes
    /// (versionFormat 1): [YY, MM, DD] in BCD encoding.
    /// e.g. [0x25, 0x0C, 0x03] = 25.12.03 = December 3, 2025.
    fn decode_firmware_version_4ks(data: &[u8]) {
        // Multi-firmware devices return version in bytes 3-5 of the 8-byte response
        // Byte layout: [?, ?, ?, YY_bcd, MM_bcd, DD_bcd, ?, ?]
        let yy = data[3];
        let mm = data[4];
        let dd = data[5];

        if yy == 0 && mm == 0 && dd == 0 {
            println!("Unknown (no version reported)");
            return;
        }

        // BCD decode: 0x25 -> 25, 0x12 -> 18 (but we display hex as-is since BCD digits == hex repr)
        if (1..=0x12).contains(&mm) && (1..=0x31).contains(&dd) {
            println!("{:02x}.{:02x}.{:02x}", yy, mm, dd);
        } else {
            // Fallback: try raw bytes, maybe version is at a different offset
            println!("Raw: {:02x?}", &data[..std::cmp::min(8, data.len())]);
        }
    }

    /// Read and display all available settings from the 4K S via HID.
    ///
    /// Each setting is read using the ReadI2cData protocol:
    ///   SET_REPORT [06, 55, sub_cmd, read_len] → GET_REPORT → response byte(s)
    ///
    /// Sub-command mapping (from EGAVDeviceSupport.dll decompilation):
    ///   0x0a = HDR tone mapping,  0x0b = color range,  0x08 = audio input,
    ///   0x12 = EDID mode,  0x19 = video scaler
    fn get_status_4ks(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Reading current settings from 4K S (PID: 0x{:04x})...\n", self.pid);

        // Firmware version
        self.get_firmware_version()?;
        println!();

        // HDR Tone Mapping: sub_cmd=0x0a, 1 byte
        // DLL: CCamLinkSupport::GetHDRTonemappingEnabled → 0x01 = On
        print!("HDR tone mapping: ");
        match self.read_hid_data(0x55, 0x0a, 1) {
            Ok(data) if !data.is_empty() => {
                match data[0] {
                    0x01 => println!("On"),
                    0x00 => println!("Off"),
                    v => println!("Unknown (0x{:02x})", v),
                }
            }
            Ok(_) => println!("No data"),
            Err(e) => println!("Read error: {}", e),
        }

        // Color Range: sub_cmd=0x0b, 1 byte
        // DLL: CCamLinkSupport::GetVideoColorRange
        //   0x00 = Auto, 0x01 = Expand (Full), 0x02 = Shrink (Limited)
        print!("HDMI color range: ");
        match self.read_hid_data(0x55, 0x0b, 1) {
            Ok(data) if !data.is_empty() => {
                match data[0] {
                    0x00 => println!("Auto"),
                    0x01 => println!("Expand (Full)"),
                    0x02 => println!("Shrink (Limited)"),
                    v => println!("Unknown (0x{:02x})", v),
                }
            }
            Ok(_) => println!("No data"),
            Err(e) => println!("Read error: {}", e),
        }

        // EDID Mode: sub_cmd=0x12, 1 byte
        // DLL: CCamLinkSupport::GetEDIDMode
        //   0x00 = Merged, 0x01 = Display, 0x02 = Internal
        print!("EDID source: ");
        match self.read_hid_data(0x55, 0x12, 1) {
            Ok(data) if !data.is_empty() => {
                match data[0] {
                    0x00 => println!("Merged"),
                    0x01 => println!("Display"),
                    0x02 => println!("Internal"),
                    v => println!("Unknown (0x{:02x})", v),
                }
            }
            Ok(_) => println!("No data"),
            Err(e) => println!("Read error: {}", e),
        }

        // Audio Input: sub_cmd=0x08, 1 byte
        // DLL: CCamLinkSupport::GetAudioInputSelection
        //   0x00 = Embedded (HDMI), 0x01 = HDMI, 0x03 = Analog
        print!("Audio input: ");
        match self.read_hid_data(0x55, 0x08, 1) {
            Ok(data) if !data.is_empty() => {
                match data[0] {
                    0x00 => println!("Embedded (HDMI)"),
                    0x01 => println!("Embedded (HDMI)"),
                    0x03 => println!("Analog (line-in)"),
                    v => println!("Unknown (0x{:02x})", v),
                }
            }
            Ok(_) => println!("No data"),
            Err(e) => println!("Read error: {}", e),
        }

        // Video Scaler: sub_cmd=0x19, 1 byte
        // DLL: CCamLinkSupport::GetVideoScalerEnabled → 0x01 = On
        print!("Video scaler: ");
        match self.read_hid_data(0x55, 0x19, 1) {
            Ok(data) if !data.is_empty() => {
                match data[0] {
                    0x01 => println!("On"),
                    0x00 => println!("Off"),
                    v => println!("Unknown (0x{:02x})", v),
                }
            }
            Ok(_) => println!("No data"),
            Err(e) => println!("Read error: {}", e),
        }

        Ok(())
    }

    fn get_status_4kx(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Reading current settings from 4K X (PID: 0x{:04x})...\n", self.pid);

        // Read firmware version via AT command 0x77
        self.get_firmware_version()?;

        // Read USB speed via AT command 0x8d
        print!("USB speed: ");
        match self.read_at_command(0x8d, 128) {
            Ok(data) if data.len() > 4 => {
                match data[4] {
                    0x00 => println!("5Gbps (SuperSpeed)"),
                    0x01 => println!("10Gbps (SuperSpeed+)"),
                    v => println!("Unknown (0x{:02x})", v),
                }
            }
            Ok(data) => {
                println!("Unexpected response ({} bytes): {:02x?}", data.len(), data);
            }
            Err(e) => {
                println!("Failed to read: {}", e);
            }
        }

        println!();

        // Targeted reads at exact expected lengths for each setting family
        let settings: &[(usize, &str)] = &[
            (9,  "HDMI Color Range"),    // a1 06 header (9 bytes)
            (10, "HDR Tone Mapping"),    // a1 07 header (10 bytes)
            (11, "EDID Range Policy"),   // a1 08 header (11 bytes)
            (13, "EDID Source/Custom"),  // a1 0a header (13 bytes)
        ];

        for &(len, label) in settings {
            print!("{}: ", label);
            match self.read_uvc_setting(len) {
                Ok(data) if !data.is_empty() && data[0] == 0xa1 => {
                    self.decode_setting(label, &data);
                }
                Ok(data) if !data.is_empty() => {
                    println!("Unknown response: {:02x?}", data);
                }
                Ok(_) => {
                    println!("No data");
                }
                Err(e) => {
                    println!("Read error: {}", e);
                }
            }
        }

        Ok(())
    }

    fn decode_setting(&self, label: &str, data: &[u8]) {
        match data[1] {
            // HDMI Color Range (a1 06, 9 bytes)
            0x06 if data.len() >= 9 => {
                match data[4] {
                    0x43 => println!("Expand (Full)"),
                    0x2b => println!("Shrink (Limited)"),
                    0x37 => println!("Auto"),
                    _ => println!("Unknown (byte4=0x{:02x})", data[4]),
                }
            }
            // HDR Tone Mapping (a1 07, 10 bytes)
            0x07 if data.len() >= 10 && data[4] == 0x1f => {
                match data[8] {
                    0x01 => println!("On"),
                    0x00 => println!("Off"),
                    _ => println!("Unknown (0x{:02x})", data[8]),
                }
            }
            // EDID Range Policy (a1 08, 11 bytes)
            0x08 if data.len() >= 11 && data[4] == 0x7c => {
                match data[9] {
                    0x00 => println!("Auto"),
                    0x03 => println!("Expand (Full)"),
                    0x04 => println!("Shrink (Limited)"),
                    _ => println!("Unknown (0x{:02x})", data[9]),
                }
            }
            // EDID Source or Custom EDID (a1 0a, 13 bytes)
            0x0a if data.len() >= 13 => {
                match data[4] {
                    0x4d => {
                        // EDID Source Selection
                        match data[8] {
                            0x01 => println!("Display"),
                            0x04 => println!("Merged"),
                            0x00 => println!("Internal"),
                            _ => println!("Unknown (0x{:02x})", data[8]),
                        }
                    }
                    0x54 => {
                        // Custom EDID selector
                        match data[9] {
                            0x00 => println!("Off"),
                            idx => println!("On (preset index {})", idx),
                        }
                    }
                    _ => {
                        println!("Unknown sub-type (byte4=0x{:02x})", data[4]);
                    }
                }
                // If this is a 13-byte read, it could be either EDID Source or Custom EDID.
                // The label from the caller is a hint, but we decode based on byte[4].
                let _ = label;
            }
            _ => {
                println!("Unrecognized (header byte1=0x{:02x}, {:02x?})", data[1], data);
            }
        }
    }
}
