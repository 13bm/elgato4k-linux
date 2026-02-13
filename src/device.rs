//! USB device discovery, opening, and lifecycle management.
//!
//! [`ElgatoDevice::open`] scans the USB bus for known Elgato 4K X and 4K S
//! product IDs, claims the appropriate interface (UVC for 4K X, HID for 4K S),
//! and returns a handle ready for control transfers.  The [`Drop`] impl
//! releases the interface and reattaches the kernel driver on cleanup.

use rusb::{Context, Device, DeviceHandle, UsbContext};

use crate::error::ElgatoError;
use crate::protocol::*;
use crate::settings::*;

/// Result of device discovery (internal).
struct FoundDevice {
    device: Device<Context>,
    model: DeviceModel,
    pid: u16,
    speed_desc: &'static str,
}

/// Handle to an opened Elgato capture card.
pub struct ElgatoDevice {
    pub(crate) handle: DeviceHandle<Context>,
    pub(crate) model: DeviceModel,
    pub(crate) pid: u16,
}

impl ElgatoDevice {
    /// Scan the USB bus, open the first supported device, and claim its interface.
    pub fn open() -> Result<Self, ElgatoError> {
        let context = Context::new()?;

        let found = Self::find_device(&context)?;
        let handle = found.device.open()?;
        let model = found.model;
        let pid = found.pid;
        let speed_desc = found.speed_desc;

        let interface_num = match model {
            DeviceModel::Elgato4KX => UVC_INTERFACE,
            DeviceModel::Elgato4KS => HID_INTERFACE,
        };

        let kernel_driver_was_active = handle.kernel_driver_active(interface_num as u8)?;

        if kernel_driver_was_active {
            handle.detach_kernel_driver(interface_num as u8)?;
            eprintln!("Temporarily detached kernel driver from interface {}", interface_num);
        }

        handle.claim_interface(interface_num as u8)?;
        eprintln!("Claimed interface {}", interface_num);
        eprintln!("Device: {} (0fd9:{:04x} - {})\n", model, pid, speed_desc);

        Ok(Self { handle, model, pid })
    }

    /// The device model (4K X or 4K S).
    pub fn model(&self) -> DeviceModel {
        self.model
    }

    /// The USB product ID detected during [`open()`](Self::open).
    pub fn pid(&self) -> u16 {
        self.pid
    }

    // --- High-level typed setters ---
    //
    // Each method constructs the correct UVC/HID payload internally and
    // dispatches via the appropriate protocol.  Model-specific features
    // return `ElgatoError::UnsupportedFeature` when called on the wrong device.

    /// Set the HDMI color range (EDID range policy).
    ///
    /// Supported on both 4K X and 4K S.
    pub fn set_hdmi_range(&self, range: EdidRangePolicy) -> Result<(), ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(range.payload_4kx()),
            DeviceModel::Elgato4KS => {
                let (ref pkt1, ref pkt2) = range.payload_4ks();
                self.send_hid_two_packet(pkt1, pkt2)
            }
        }
    }

    /// Set the EDID source selection.
    ///
    /// Supported on both 4K X and 4K S.
    pub fn set_edid_source(&self, source: EdidSource) -> Result<(), ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(source.payload_4kx()),
            DeviceModel::Elgato4KS => {
                let pkt = source.payload_4ks();
                self.send_hid_packet(&pkt)
            }
        }
    }

    /// Set HDR tone mapping on or off.
    ///
    /// Supported on both 4K X and 4K S.
    pub fn set_hdr_mapping(&self, mode: HdrToneMapping) -> Result<(), ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(mode.payload_4kx()),
            DeviceModel::Elgato4KS => {
                let (ref pkt1, ref pkt2) = mode.payload_4ks();
                self.send_hid_two_packet(pkt1, pkt2)
            }
        }
    }

    /// Set custom EDID preset on or off.
    ///
    /// **4K X only.** Returns [`ElgatoError::UnsupportedFeature`] on the 4K S.
    pub fn set_custom_edid(&self, mode: CustomEdidMode) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "Custom EDID",
                model: "4K S",
            });
        }
        self.set_uvc_setting(mode.payload_4kx())
    }

    /// Set the audio input source.
    ///
    /// **4K S only.** Returns [`ElgatoError::UnsupportedFeature`] on the 4K X.
    pub fn set_audio_input(&self, input: AudioInput) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KS {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "Audio input selection",
                model: "4K X",
            });
        }
        let (ref pkt1, ref pkt2) = input.payload_4ks();
        self.send_hid_two_packet(pkt1, pkt2)
    }

    /// Set the video scaler on or off.
    ///
    /// **4K S only.** Returns [`ElgatoError::UnsupportedFeature`] on the 4K X.
    pub fn set_video_scaler(&self, scaler: VideoScaler) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KS {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "Video scaler",
                model: "4K X",
            });
        }
        let (ref pkt1, ref pkt2) = scaler.payload_4ks();
        self.send_hid_two_packet(pkt1, pkt2)
    }

    /// Set the USB speed mode.
    ///
    /// **4K X only.** Returns [`ElgatoError::UnsupportedFeature`] on the 4K S.
    ///
    /// # Warning
    ///
    /// The device will disconnect and re-enumerate with a different product ID
    /// after changing speed modes.
    pub fn set_usb_speed(&self, speed: UsbSpeed) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "USB speed switching",
                model: "4K S",
            });
        }
        self.send_at_command(AT_CMD_SET_USB_SPEED, &speed.at_input())
    }

    fn find_device(context: &Context) -> Result<FoundDevice, ElgatoError> {
        for device in context.devices()?.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if desc.vendor_id() != VENDOR_ID {
                continue;
            }

            let pid = desc.product_id();

            for &(known_pid, speed_desc) in PIDS_4KX {
                if pid == known_pid {
                    return Ok(FoundDevice { device, model: DeviceModel::Elgato4KX, pid, speed_desc });
                }
            }

            for &(known_pid, speed_desc) in PIDS_4KS {
                if pid == known_pid {
                    return Ok(FoundDevice { device, model: DeviceModel::Elgato4KS, pid, speed_desc });
                }
            }
        }

        Err(ElgatoError::DeviceNotFound)
    }
}

impl Drop for ElgatoDevice {
    fn drop(&mut self) {
        let interface_num = match self.model {
            DeviceModel::Elgato4KX => UVC_INTERFACE,
            DeviceModel::Elgato4KS => HID_INTERFACE,
        };

        if let Err(e) = self.handle.release_interface(interface_num as u8) {
            eprintln!("Warning: Failed to release interface: {}", e);
        }

        // Best-effort reattach — will fail on platforms without kernel drivers
        let _ = self.handle.attach_kernel_driver(interface_num as u8);
    }
}
