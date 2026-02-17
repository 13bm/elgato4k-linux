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
mod error;
mod hid;
mod protocol;
mod settings;
mod status;
mod uvc;

pub use device::ElgatoDevice;
pub use error::ElgatoError;
pub use settings::{
    AudioInput, CustomEdidMode, DeviceModel, EdidRangePolicy,
    EdidSource, HdrToneMapping, UsbSpeed, VideoScaler,
};
pub use status::{CustomEdidStatus, DeviceStatus, ReadValue, UsbSpeedStatus};
