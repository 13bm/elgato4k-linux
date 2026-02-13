//! Custom error types for the elgato4k-linux tool.
//!
//! Provides structured errors instead of `Box<dyn Error>`, so callers can
//! programmatically distinguish between device-not-found, USB transport
//! failures, invalid arguments, and unsupported features.

use thiserror::Error;

/// Top-level error type for all elgato4k operations.
#[derive(Debug, Error)]
pub enum ElgatoError {
    /// No supported Elgato device was found on the USB bus.
    #[error("Elgato 4K X or 4K S not found. Make sure it's connected.\n\
             Known PIDs: 4K X (009b, 009c, 009d), 4K S (00ae, 00af)")]
    DeviceNotFound,

    /// A USB/libusb transport error occurred.
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),

    /// HID packet size mismatch.
    #[error("HID packet must be exactly {expected} bytes, got {got}")]
    HidPacketSize { expected: usize, got: usize },

    /// A HID SET_REPORT or GET_REPORT transfer failed.
    #[error("HID transfer failed: {0}")]
    HidTransfer(String),

    /// A UVC control transfer failed.
    #[error("UVC transfer failed: {0}")]
    UvcTransfer(String),

    /// The requested feature is not supported on this device model.
    #[error("{feature} is not supported on {model}")]
    UnsupportedFeature {
        feature: &'static str,
        model: &'static str,
    },

    /// Invalid CLI argument value.
    #[error("Invalid value '{value}' for {arg}.\nValid values: {valid}")]
    InvalidArgument {
        arg: &'static str,
        value: String,
        valid: &'static str,
    },

    /// A required CLI argument value is missing.
    #[error("{0} requires a value")]
    MissingArgumentValue(String),
}
