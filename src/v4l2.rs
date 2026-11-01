//! HDMI queries through the bound video driver, without libusb detachment.

use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::error::ElgatoError;
use crate::protocol::*;
use crate::signal::{HdmiSignal, read_signal};
use crate::uvc::frame_at_command;

#[repr(C)]
struct XuQuery {
    unit: u8,
    selector: u8,
    query: u8,
    size: u16,
    data: *mut u8,
}

fn transport_error(context: &str, error: std::io::Error) -> ElgatoError {
    let hint = if error.raw_os_error() == Some(libc::ENOBUFS) {
        "; uvcvideo needs the Elgato variable-length XU patch (no detach fallback attempted)"
    } else {
        ""
    };
    ElgatoError::UvcTransfer(format!("{context}: {error}{hint}"))
}

fn is_4kx(vendor: &str, product: &str) -> bool {
    u16::from_str_radix(vendor.trim(), 16) == Ok(VENDOR_ID)
        && u16::from_str_radix(product.trim(), 16)
            .is_ok_and(|pid| PIDS_4KX.iter().any(|(known, _)| *known == pid))
}

fn validate_device(file: &File) -> Result<(), ElgatoError> {
    let metadata = file
        .metadata()
        .map_err(|e| transport_error("stat video device", e))?;
    if !metadata.file_type().is_char_device() {
        return Err(ElgatoError::UvcTransfer(
            "expected a video character device".into(),
        ));
    }
    let node = PathBuf::from(format!(
        "/sys/dev/char/{}:{}",
        libc::major(metadata.rdev()),
        libc::minor(metadata.rdev())
    ));
    let subsystem = fs::canonicalize(node.join("subsystem"))
        .map_err(|e| transport_error("resolve video subsystem", e))?;
    if subsystem
        .file_name()
        .is_none_or(|name| name != "video4linux")
    {
        return Err(ElgatoError::UvcTransfer("expected a V4L2 device".into()));
    }
    let device = fs::canonicalize(node.join("device"))
        .map_err(|e| transport_error("resolve video USB device", e))?;
    for ancestor in device.ancestors() {
        if let (Ok(vendor), Ok(product)) = (
            fs::read_to_string(ancestor.join("idVendor")),
            fs::read_to_string(ancestor.join("idProduct")),
        ) {
            return if is_4kx(&vendor, &product) {
                Ok(())
            } else {
                Err(ElgatoError::DeviceNotFound)
            };
        }
    }
    Err(ElgatoError::DeviceNotFound)
}

fn query(file: &File, selector: u8, operation: u8, data: &mut [u8]) -> Result<(), ElgatoError> {
    let mut query = XuQuery {
        unit: UVC_ENTITY_ID as u8,
        selector,
        query: operation,
        size: data
            .len()
            .try_into()
            .map_err(|_| ElgatoError::UvcTransfer("XU transfer exceeds u16 length".into()))?,
        data: data.as_mut_ptr(),
    };
    // The C-layout query and its data buffer remain valid throughout ioctl.
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            libc::_IOWR::<XuQuery>(b'u' as u32, 0x21),
            &mut query,
        )
    };
    if result < 0 {
        return Err(transport_error(
            &format!(
                "XU selector {selector} query 0x{operation:02x} length {}",
                data.len()
            ),
            std::io::Error::last_os_error(),
        ));
    }
    Ok(())
}

fn read_length(file: &File, selector: u8) -> Result<usize, ElgatoError> {
    let mut bytes = [0; 2];
    query(file, selector, UVC_GET_LEN, &mut bytes)?;
    Ok(u16::from_le_bytes(bytes) as usize)
}

fn response_length(length: usize) -> Result<usize, ElgatoError> {
    // Only HDMI flag (9 bytes) and raw timing (45 bytes) responses are valid.
    if matches!(length, 9 | 45) {
        Ok(length)
    } else {
        Err(ElgatoError::UvcTransfer(format!(
            "unexpected HDMI response length {length}"
        )))
    }
}

/// Query HDMI receiver timing through `UVCIOC_CTRL_QUERY` on a 4K X video node.
/// Never opens libusb, claims a streaming interface, or falls back to detachment.
/// Requires the variable-length XU driver patch on kernels that cache GET_LEN.
pub fn read_hdmi_signal_v4l2(path: impl AsRef<Path>) -> Result<HdmiSignal, ElgatoError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path.as_ref())
        .map_err(|e| transport_error("open video device", e))?;
    validate_device(&file)?;
    // Coordinate this utility's multi-ioctl transactions on the same video node.
    // Capture does not use this advisory lock and remains active.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err(transport_error(
            "lock HDMI query",
            std::io::Error::last_os_error(),
        ));
    }
    read_signal(|command| {
        let mut payload = frame_at_command(command, &[]);
        let mut trigger = (payload.len() as u16).to_le_bytes();
        query(&file, UVC_SELECTOR_TRIGGER as u8, UVC_SET_CUR, &mut trigger)?;
        query(&file, UVC_SELECTOR_VALUE as u8, UVC_SET_CUR, &mut payload)?;
        let mut status = [0; 2];
        query(&file, UVC_SELECTOR_TRIGGER as u8, UVC_GET_CUR, &mut status)?;
        let length = response_length(read_length(&file, UVC_SELECTOR_VALUE as u8)?)?;
        let mut response = vec![0; length];
        query(&file, UVC_SELECTOR_VALUE as u8, UVC_GET_CUR, &mut response)?;
        Ok(response)
    })
}

/// Find one primary 4K X video node; require explicit selection if ambiguous.
pub fn find_hdmi_video_device() -> Result<PathBuf, ElgatoError> {
    let mut candidates = Vec::new();
    let entries = fs::read_dir("/sys/class/video4linux")
        .map_err(|e| transport_error("list video devices", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| transport_error("read video device entry", e))?;
        if fs::read_to_string(entry.path().join("index")).is_ok_and(|index| index.trim() == "0") {
            let device = fs::canonicalize(entry.path().join("device"))
                .map_err(|e| transport_error("resolve video device", e))?;
            if device.ancestors().any(|ancestor| {
                match (
                    fs::read_to_string(ancestor.join("idVendor")),
                    fs::read_to_string(ancestor.join("idProduct")),
                ) {
                    (Ok(vendor), Ok(product)) => is_4kx(&vendor, &product),
                    _ => false,
                }
            }) {
                candidates.push(PathBuf::from("/dev").join(entry.file_name()));
            }
        }
    }
    match candidates.len() {
        0 => Err(ElgatoError::DeviceNotFound),
        1 => Ok(candidates.remove(0)),
        _ => Err(ElgatoError::UvcTransfer(
            "multiple 4K X devices found; select --video-device /dev/videoN".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_4kx_usb_ids() {
        for product in ["009b\n", "009c", "009d"] {
            assert!(is_4kx("0fd9\n", product));
        }
        for (vendor, product) in [
            ("0fd9", "00af"),
            ("0fd9", "00ae"),
            ("1234", "009b"),
            ("0fd9", "oops"),
        ] {
            assert!(!is_4kx(vendor, product));
        }
    }

    #[test]
    fn bounds_response_allocation() {
        assert_eq!(response_length(9).unwrap(), 9);
        assert_eq!(response_length(45).unwrap(), 45);
        for length in [0, 1, 8, 10, 44, 46, 4096, 65535] {
            assert!(response_length(length).is_err());
        }
    }

    #[test]
    fn explains_unpatched_kernel_without_fallback() {
        let text =
            transport_error("query", std::io::Error::from_raw_os_error(libc::ENOBUFS)).to_string();
        assert!(text.contains("variable-length XU patch"));
        assert!(text.contains("no detach fallback"));
    }
}
