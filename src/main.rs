use rusb::{Context, Device, DeviceHandle, UsbContext};
use std::time::Duration;

// Elgato 4K X USB identifiers (UVC)
const VENDOR_ID_4KX: u16 = 0x0fd9;
const PRODUCT_ID_4KX: u16 = 0x009c;

// Elgato 4K S USB identifiers (HID)
const VENDOR_ID_4KS: u16 = 0x0fd9;
const PRODUCT_ID_4KS: u16 = 0x00af;

// 4K X (UVC) Control Request parameters
const UVC_REQUEST_TYPE: u8 = 0x21;
const UVC_REQUEST_SET_CUR: u8 = 0x01;
const UVC_INTERFACE_NUM: u16 = 0;
const UVC_ENTITY_ID: u16 = 4;
const UVC_SELECTOR_TRIGGER: u16 = 0x02;
const UVC_SELECTOR_VALUE: u16 = 0x01;

// 4K S (HID) Control Request parameters
const HID_REQUEST_TYPE: u8 = 0x21;
const HID_REQUEST_SET_REPORT: u8 = 0x09;
const HID_REPORT_VALUE: u16 = 0x0206;
const HID_INTERFACE_NUM: u16 = 7;
const HID_PACKET_SIZE: usize = 255;

const TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy)]
pub enum DeviceModel {
    Elgato4KX,
    Elgato4KS,
}

#[derive(Debug, Clone, Copy)]
pub enum HdmiColorRange {
    Expand,   // Full
    Shrink,   // Limited
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

impl HdmiColorRange {
    fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::Auto => vec![0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x00, 0xda],
            Self::Expand => vec![0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x03, 0xd7],
            Self::Shrink => vec![0xa1, 0x08, 0x00, 0x00, 0x7c, 0x00, 0x00, 0x00, 0x01, 0x04, 0xd6],
        }
    }
    
    fn payload_4ks(&self) -> (Vec<u8>, Vec<u8>) {
        let mut pkt1 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x0b];
        pkt1.push(match self {
            Self::Auto => 0x00,
            Self::Expand => 0x01,
            Self::Shrink => 0x02,
        });
        pkt1.resize(HID_PACKET_SIZE, 0x00);
        
        let mut pkt2 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x13, 0x01];
        pkt2.resize(HID_PACKET_SIZE, 0x00);
        
        (pkt1, pkt2)
    }
    
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "expand" | "full" => Some(Self::Expand),
            "shrink" | "limited" => Some(Self::Shrink),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

impl EdidSource {
    fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::Display => vec![0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x07],
            Self::Merged => vec![0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04],
            Self::Internal => vec![0xa1, 0x0a, 0x00, 0x00, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08],
        }
    }
    
    fn payload_4ks(&self) -> Vec<u8> {
        let mut pkt = vec![0x06, 0x06, 0x06, 0x55, 0x02];
        match self {
            Self::Merged => pkt.extend_from_slice(&[0x12, 0x00]),
            Self::Display => pkt.extend_from_slice(&[0x12, 0x01]),
            Self::Internal => pkt.extend_from_slice(&[0x13, 0x01]),
        }
        pkt.resize(HID_PACKET_SIZE, 0x00);
        pkt
    }
    
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "display" => Some(Self::Display),
            "merged" => Some(Self::Merged),
            "internal" => Some(Self::Internal),
            _ => None,
        }
    }
}

impl HdrToneMapping {
    fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::On => vec![0xa1, 0x07, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x01, 0x38],
            Self::Off => vec![0xa1, 0x07, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x39],
        }
    }
    
    fn payload_4ks(&self) -> (Vec<u8>, Vec<u8>) {
        let mut pkt1 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x0a];
        pkt1.push(match self {
            Self::On => 0x01,
            Self::Off => 0x00,
        });
        pkt1.resize(HID_PACKET_SIZE, 0x00);
        
        let mut pkt2 = vec![0x06, 0x06, 0x06, 0x55, 0x02, 0x13, 0x01];
        pkt2.resize(HID_PACKET_SIZE, 0x00);
        
        (pkt1, pkt2)
    }
    
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Some(Self::On),
            "off" | "false" | "0" => Some(Self::Off),
            _ => None,
        }
    }
}

impl CustomEdidMode {
    fn payload_4kx(&self) -> Vec<u8> {
        match self {
            Self::Off => vec![0xa1, 0x0a, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x81],
            Self::On => vec![0xa1, 0x0a, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0x80],
        }
    }
    
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Some(Self::On),
            "off" | "false" | "0" => Some(Self::Off),
            _ => None,
        }
    }
}

pub struct ElgatoDevice {
    handle: DeviceHandle<Context>,
    model: DeviceModel,
}

impl ElgatoDevice {
    pub fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new()?;
        
        // Try to find 4K X first, then 4K S
        let (device, model) = Self::find_device(&context)?;
        let handle = device.open()?;
        
        let interface_num = match model {
            DeviceModel::Elgato4KX => UVC_INTERFACE_NUM,
            DeviceModel::Elgato4KS => HID_INTERFACE_NUM,
        };
        
        let kernel_driver_was_active = handle.kernel_driver_active(interface_num as u8)?;
        
        if kernel_driver_was_active {
            handle.detach_kernel_driver(interface_num as u8)?;
            println!("Temporarily detached kernel driver from interface {}", interface_num);
        }
        
        handle.claim_interface(interface_num as u8)?;
        println!("Claimed interface {}", interface_num);
        println!("Device model: {:?}\n", model);
        
        Ok(Self { handle, model })
    }
    
    fn find_device(context: &Context) -> Result<(Device<Context>, DeviceModel), Box<dyn std::error::Error>> {
        for device in context.devices()?.iter() {
            let desc = device.device_descriptor()?;
            if desc.vendor_id() == VENDOR_ID_4KX && desc.product_id() == PRODUCT_ID_4KX {
                return Ok((device, DeviceModel::Elgato4KX));
            }
            if desc.vendor_id() == VENDOR_ID_4KS && desc.product_id() == PRODUCT_ID_4KS {
                return Ok((device, DeviceModel::Elgato4KS));
            }
        }
        Err("Elgato 4K X or 4K S not found. Make sure it's connected.".into())
    }
    
    // 4K X (UVC) methods
    fn send_uvc_trigger(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let data = [0x09, 0x00];
        let w_value = (UVC_SELECTOR_TRIGGER << 8) | 0x00;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE_NUM;
        
        self.handle.write_control(
            UVC_REQUEST_TYPE,
            UVC_REQUEST_SET_CUR,
            w_value,
            w_index,
            &data,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send UVC trigger: {}", e))?;
        
        Ok(())
    }
    
    fn send_uvc_payload(&mut self, payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let w_value = (UVC_SELECTOR_VALUE << 8) | 0x00;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE_NUM;
        
        self.handle.write_control(
            UVC_REQUEST_TYPE,
            UVC_REQUEST_SET_CUR,
            w_value,
            w_index,
            payload,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send UVC payload: {}", e))?;
        
        Ok(())
    }
    
    fn set_uvc_setting(&mut self, payload: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        self.send_uvc_trigger()?;
        self.send_uvc_payload(&payload)?;
        Ok(())
    }
    
    // 4K S (HID) methods
    fn send_hid_packet(&mut self, packet: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if packet.len() != HID_PACKET_SIZE {
            return Err(format!("HID packet must be exactly {} bytes", HID_PACKET_SIZE).into());
        }
        
        self.handle.write_control(
            HID_REQUEST_TYPE,
            HID_REQUEST_SET_REPORT,
            HID_REPORT_VALUE,
            HID_INTERFACE_NUM,
            packet,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send HID packet: {}", e))?;
        
        Ok(())
    }
    
    fn send_hid_two_packet(&mut self, pkt1: Vec<u8>, pkt2: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        self.send_hid_packet(&pkt1)?;
        std::thread::sleep(Duration::from_millis(1));
        self.send_hid_packet(&pkt2)?;
        Ok(())
    }
    
    // Read current settings (4K X only via UVC GET_CUR)
    fn read_uvc_setting(&mut self, length: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let w_value = (UVC_SELECTOR_VALUE << 8) | 0x00;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE_NUM;
        let mut buf = vec![0u8; length];
        
        match self.handle.read_control(
            0xa1, // Device-to-host, Class, Interface
            0x81, // GET_CUR
            w_value,
            w_index,
            &mut buf,
            TIMEOUT,
        ) {
            Ok(len) => Ok(buf[..len].to_vec()),
            Err(e) => Err(format!("Failed to read setting: {}", e).into()),
        }
    }
    
    pub fn get_status(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.model {
            DeviceModel::Elgato4KX => {
                println!("Reading current settings from 4K X...\n");
                
                // Try different read lengths to see what the device returns
                let lengths = [9, 10, 11, 13, 16, 32];
                
                for &len in &lengths {
                    println!("Attempting to read {} bytes:", len);
                    match self.read_uvc_setting(len) {
                        Ok(data) if !data.is_empty() => {
                            println!("  Success! Raw data: {:02x?}", data);
                            
                            // Try to decode based on what we got
                            if data.len() >= 9 && data[0] == 0xa1 {
                                match data[1] {
                                    0x06 if data.len() >= 9 => {
                                        println!("  Possible HDMI Color Range (header 0xa1 0x06)");
                                        self.decode_hdmi_range(&data);
                                    }
                                    0x07 if data.len() >= 10 => {
                                        println!("  Possible HDR Tone Mapping (header 0xa1 0x07)");
                                        self.decode_hdr_mapping(&data);
                                    }
                                    0x08 if data.len() >= 11 => {
                                        println!("  Possible EDID Range Policy (header 0xa1 0x08)");
                                        if data.len() >= 11 && data[4] == 0x7c {
                                            match data[9] {
                                                0x00 => println!("    Decoded: Auto"),
                                                0x03 => println!("    Decoded: Expand (Full)"),
                                                0x04 => println!("    Decoded: Shrink (Limited)"),
                                                _ => println!("    Decoded: Unknown ({})", data[9]),
                                            }
                                        }
                                    }
                                    0x0a if data.len() >= 13 => {
                                        println!("  Possible EDID Source or Custom EDID (header 0xa1 0x0a)");
                                        if data.len() >= 13 {
                                            match data[4] {
                                                0x4d => {
                                                    println!("    Type: EDID Source");
                                                    self.decode_edid_source(&data);
                                                }
                                                0x54 => {
                                                    println!("    Type: Custom EDID");
                                                    self.decode_custom_edid(&data);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(_) => {
                            println!("  Empty response");
                        }
                        Err(e) => {
                            println!("  Error: {}", e);
                        }
                    }
                    println!();
                }
                
                println!("\n--- Summary ---");
                println!("Note: The device may not support reading all settings.");
                println!("Some capture cards only allow writing settings, not reading them back.");
            }
            DeviceModel::Elgato4KS => {
                println!("Status reading not available for 4K S (HID protocol limitation)");
                println!("The 4K S uses output-only HID reports without read capability.");
            }
        }
        Ok(())
    }
    
    fn decode_hdmi_range(&self, data: &[u8]) {
        if data.len() >= 11 && data[0] == 0xa1 && data[1] == 0x08 {
            if data[4] == 0x7c {
                match data[9] {
                    0x00 => println!("  Status: Auto"),
                    0x03 => println!("  Status: Expand (Full)"),
                    0x04 => println!("  Status: Shrink (Limited)"),
                    _ => println!("  Status: Unknown ({})", data[9]),
                }
            }
        }
    }
    
    fn decode_edid_source(&self, data: &[u8]) {
        if data.len() >= 13 && data[0] == 0xa1 && data[1] == 0x0a {
            if data[4] == 0x4d {
                match data[8] {
                    0x01 => println!("  Status: Display"),
                    0x04 => println!("  Status: Merged"),
                    0x00 => println!("  Status: Internal"),
                    _ => println!("  Status: Unknown ({})", data[8]),
                }
            }
        }
    }
    
    fn decode_hdr_mapping(&self, data: &[u8]) {
        if data.len() >= 10 && data[0] == 0xa1 && data[1] == 0x07 {
            if data[4] == 0x1f {
                match data[8] {
                    0x01 => println!("  Status: On"),
                    0x00 => println!("  Status: Off"),
                    _ => println!("  Status: Unknown ({})", data[8]),
                }
            }
        }
    }
    
    fn decode_custom_edid(&self, data: &[u8]) {
        if data.len() >= 13 && data[0] == 0xa1 && data[1] == 0x0a {
            if data[4] == 0x54 {
                match data[9] {
                    0x00 => println!("  Status: Off"),
                    _ => println!("  Status: On"),
                }
            }
        }
    }
    
    // Public API
    pub fn set_hdmi_color_range(&mut self, range: HdmiColorRange) -> Result<(), Box<dyn std::error::Error>> {
        println!("Setting HDMI color range to {:?}", range);
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(range.payload_4kx()),
            DeviceModel::Elgato4KS => {
                let (pkt1, pkt2) = range.payload_4ks();
                self.send_hid_two_packet(pkt1, pkt2)
            }
        }
    }
    
    pub fn set_edid_source(&mut self, source: EdidSource) -> Result<(), Box<dyn std::error::Error>> {
        println!("Setting EDID source to {:?}", source);
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(source.payload_4kx()),
            DeviceModel::Elgato4KS => self.send_hid_packet(&source.payload_4ks()),
        }
    }
    
    pub fn set_hdr_tone_mapping(&mut self, mode: HdrToneMapping) -> Result<(), Box<dyn std::error::Error>> {
        println!("Setting HDR tone mapping to {:?}", mode);
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(mode.payload_4kx()),
            DeviceModel::Elgato4KS => {
                let (pkt1, pkt2) = mode.payload_4ks();
                self.send_hid_two_packet(pkt1, pkt2)
            }
        }
    }
    
    pub fn set_custom_edid(&mut self, mode: CustomEdidMode) -> Result<(), Box<dyn std::error::Error>> {
        match self.model {
            DeviceModel::Elgato4KX => {
                println!("Setting custom EDID to {:?}", mode);
                self.set_uvc_setting(mode.payload_4kx())
            }
            DeviceModel::Elgato4KS => {
                Err("Custom EDID not supported on 4K S".into())
            }
        }
    }
}

impl Drop for ElgatoDevice {
    fn drop(&mut self) {
        let interface_num = match self.model {
            DeviceModel::Elgato4KX => UVC_INTERFACE_NUM,
            DeviceModel::Elgato4KS => HID_INTERFACE_NUM,
        };
        
        if let Err(e) = self.handle.release_interface(interface_num as u8) {
            eprintln!("Warning: Failed to release interface: {}", e);
        } else {
            println!("\nReleased interface {}", interface_num);
        }
        
        if let Err(e) = self.handle.attach_kernel_driver(interface_num as u8) {
            eprintln!("Warning: Failed to reattach kernel driver: {}", e);
        } else {
            println!("Reattached kernel driver");
        }
    }
}

fn print_usage() {
    println!("Elgato 4K X/S Controller - USB Control Tool\n");
    println!("USAGE:");
    println!("    sudo elgato4k [OPTIONS]\n");
    println!("OPTIONS:");
    println!("    --status                    Read current device settings (4K X only)\n");
    println!("    --hdmi-range <VALUE>        Set HDMI color range");
    println!("                                Values: expand, shrink, auto");
    println!("                                (expand = Full, shrink = Limited)\n");
    println!("    --edid-source <VALUE>       Set EDID source selection");
    println!("                                Values: display, merged, internal\n");
    println!("    --hdr-map <VALUE>           Set HDR tone mapping");
    println!("                                Values: on, off\n");
    println!("    --custom-edid <VALUE>       Set custom EDID mode (4K X only)");
    println!("                                Values: on, off\n");
    println!("    --help, -h                  Show this help message\n");
    println!("EXAMPLES:");
    println!("    sudo elgato4k --status");
    println!("    sudo elgato4k --hdr-map on");
    println!("    sudo elgato4k --hdmi-range expand --hdr-map on");
    println!("    sudo elgato4k --edid-source display --hdmi-range auto");
    println!("    sudo elgato4k --custom-edid on  # 4K X only");
    println!("\nSUPPORTED DEVICES:");
    println!("    - Elgato 4K X (0fd9:009c)");
    println!("    - Elgato 4K S (0fd9:00af)");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 || args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return Ok(());
    }
    
    let mut device = ElgatoDevice::open()?;
    
    // std::thread::sleep(Duration::from_millis(200));
    
    // Handle --status flag specially (no value required)
    if args.contains(&"--status".to_string()) {
        device.get_status()?;
        return Ok(());
    }
    
    let mut i = 1;
    let mut settings_applied = false;
    
    while i < args.len() {
        let arg = &args[i];
        
        if i + 1 >= args.len() {
            eprintln!("Error: {} requires a value", arg);
            return Err("Missing argument value".into());
        }
        
        let value = &args[i + 1];
        
        match arg.as_str() {
            "--hdmi-range" => {
                if let Some(range) = HdmiColorRange::from_str(value) {
                    device.set_hdmi_color_range(range)?;
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for --hdmi-range", value);
                    eprintln!("Valid values: expand, shrink, auto");
                    return Err("Invalid argument".into());
                }
            }
            "--edid-source" => {
                if let Some(source) = EdidSource::from_str(value) {
                    device.set_edid_source(source)?;
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for --edid-source", value);
                    eprintln!("Valid values: display, merged, internal");
                    return Err("Invalid argument".into());
                }
            }
            "--hdr-map" => {
                if let Some(mode) = HdrToneMapping::from_str(value) {
                    device.set_hdr_tone_mapping(mode)?;
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for --hdr-map", value);
                    eprintln!("Valid values: on, off");
                    return Err("Invalid argument".into());
                }
            }
            "--custom-edid" => {
                if let Some(mode) = CustomEdidMode::from_str(value) {
                    device.set_custom_edid(mode)?;
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for --custom-edid", value);
                    eprintln!("Valid values: on, off");
                    return Err("Invalid argument".into());
                }
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", arg);
                print_usage();
                return Err("Invalid argument".into());
            }
        }
        
        i += 2;
        std::thread::sleep(Duration::from_millis(100));
    }
    
    if settings_applied {
        println!("\n✓ All settings applied successfully!");
    } else {
        println!("No settings were changed.");
    }
    
    Ok(())
}