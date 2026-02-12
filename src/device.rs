use rusb::{Context, Device, DeviceHandle, UsbContext};
use crate::settings::DeviceModel;
use crate::uvc::UVC_INTERFACE_NUM;
use crate::hid::HID_INTERFACE_NUM;

const VENDOR_ID: u16 = 0x0fd9;

/// All known 4K X product IDs (same device, different USB speed modes).
const PIDS_4KX: &[(u16, &str)] = &[
    (0x009b, "10Gbps / SuperSpeed+"),
    (0x009c, "5Gbps / SuperSpeed"),
    (0x009d, "USB 2.0"),
];

/// All known 4K S product IDs.
const PIDS_4KS: &[(u16, &str)] = &[
    (0x00af, "USB 3.0"),
    (0x00ae, "USB 2.0"),
];

/// Result of device discovery.
struct FoundDevice {
    device: Device<Context>,
    model: DeviceModel,
    pid: u16,
    speed_desc: &'static str,
}

pub struct ElgatoDevice {
    pub(crate) handle: DeviceHandle<Context>,
    pub(crate) model: DeviceModel,
    pub pid: u16,
}

impl ElgatoDevice {
    pub fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new()?;

        let found = Self::find_device(&context)?;
        let handle = found.device.open()?;
        let model = found.model;
        let pid = found.pid;
        let speed_desc = found.speed_desc;

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
        println!("Device: {:?} (0fd9:{:04x} - {})\n", model, pid, speed_desc);

        Ok(Self { handle, model, pid })
    }

    pub fn model(&self) -> DeviceModel {
        self.model
    }

    fn find_device(context: &Context) -> Result<FoundDevice, Box<dyn std::error::Error>> {
        for device in context.devices()?.iter() {
            let desc = device.device_descriptor()?;
            if desc.vendor_id() != VENDOR_ID {
                continue;
            }

            let pid = desc.product_id();

            // Check 4K X PIDs
            for &(known_pid, speed_desc) in PIDS_4KX {
                if pid == known_pid {
                    return Ok(FoundDevice { device, model: DeviceModel::Elgato4KX, pid, speed_desc });
                }
            }

            // Check 4K S PIDs
            for &(known_pid, speed_desc) in PIDS_4KS {
                if pid == known_pid {
                    return Ok(FoundDevice { device, model: DeviceModel::Elgato4KS, pid, speed_desc });
                }
            }
        }

        Err("Elgato 4K X or 4K S not found. Make sure it's connected.\n\
             Known PIDs: 4K X (009b, 009c, 009d), 4K S (00ae, 00af)".into())
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
