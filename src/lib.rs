//! Elgato 4K X/S Controller library.
//!
//! Provides programmatic control of Elgato 4K X (UVC) and 4K S (HID)
//! capture cards on Linux via USB.
//!
//! # Quick Start
//!
//! ```no_run
//! use elgato4k_linux::{ElgatoDevice, HdrToneMapping, EdidSource};
//!
//! let device = ElgatoDevice::open()?;
//! device.set_hdr_mapping(HdrToneMapping::On)?;
//! device.set_edid_source(EdidSource::Display)?;
//!
//! let status = device.read_status()?;
//! println!("Firmware: {}", status.firmware_version);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod device;
mod edid;
mod error;
mod hid;
mod protocol;
mod settings;
mod signal;
mod status;
mod uvc;
mod v4l2;

pub use device::ElgatoDevice;
pub use edid::{EdidUploadPlan, validate_edid};
pub use error::ElgatoError;
pub use settings::{
    AudioInput, DeviceModel, EdidRangePolicy, EdidSource, HdrToneMapping, UsbSpeed, VideoScaler,
};
pub use signal::{HdmiSignal, HdmiTiming};
pub use status::{DeviceStatus, ReadValue, UsbSpeedStatus};
pub use v4l2::{find_hdmi_video_device, read_hdmi_signal_v4l2};
