//! USB device discovery, opening, and lifecycle management.
//!
//! [`ElgatoDevice::open`] scans the USB bus for known Elgato 4K X and 4K S
//! product IDs, claims the appropriate interface (UVC for 4K X, HID for 4K S),
//! and returns a handle ready for control transfers.  The [`Drop`] impl
//! releases the interface and reattaches the kernel driver on cleanup.

use rusb::{Context, Device, DeviceHandle, UsbContext};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::edid::{EdidUploadPlan, rescue_finalize_packet};
use crate::error::ElgatoError;
use crate::protocol::*;
use crate::settings::*;

/// Result of device discovery (internal).
struct FoundDevice {
    device: Device<Context>,
    model: DeviceModel,
    pid: u16,
}

/// Raw RTK VCOM transport exposed only after `enter_rescue`.
struct ElgatoVcom {
    port: File,
}

/// Handle to an opened Elgato capture card.
pub struct ElgatoDevice {
    pub(crate) handle: DeviceHandle<Context>,
    pub(crate) model: DeviceModel,
    pub(crate) pid: u16,
    control_driver_was_active: bool,
    streaming_driver_was_active: bool,
}

impl ElgatoDevice {
    /// Scan the USB bus, open the first supported device, and claim its interface.
    pub fn open() -> Result<Self, ElgatoError> {
        let context = Context::new()?;

        let found = Self::find_device(&context)?;
        let handle = found.device.open()?;
        let model = found.model;
        let pid = found.pid;

        let interface_num = match model {
            DeviceModel::Elgato4KX => UVC_INTERFACE,
            DeviceModel::Elgato4KS => HID_INTERFACE,
        };

        let control_driver_was_active = handle.kernel_driver_active(interface_num as u8)?;
        let streaming_driver_was_active = if model == DeviceModel::Elgato4KX {
            handle.kernel_driver_active(UVC_STREAMING_INTERFACE as u8)?
        } else {
            false
        };

        if control_driver_was_active {
            handle.detach_kernel_driver(interface_num as u8)?;
        }
        // Detaching the UVC control interface unbinds the paired streaming
        // interface as part of one uvcvideo function. Detaching interface 1
        // separately returns ENODEV and leaves the function half-unbound.

        handle.claim_interface(interface_num as u8)?;

        Ok(Self {
            handle,
            model,
            pid,
            control_driver_was_active,
            streaming_driver_was_active,
        })
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
            DeviceModel::Elgato4KS => self.send_hid_packet(&range.payload_4ks()),
        }
    }

    /// Set the EDID source selection.
    ///
    /// Supported on both 4K X and 4K S.
    pub fn set_edid_source(&self, source: EdidSource) -> Result<(), ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(source.payload_4kx()),
            DeviceModel::Elgato4KS => self.send_hid_packet(&source.payload_4ks()),
        }
    }

    /// Set the HDMI receiver hot-plug state (4K X only).
    pub fn set_hdmi_hpd(&self, enabled: bool) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "HDMI hot-plug control",
                model: "4K S",
            });
        }
        self.send_at_command(AT_CMD_SET_HDMI_HPD, &[u8::from(enabled)])
            .map(|_| ())
    }

    /// Set HDR tone mapping on or off.
    ///
    /// Supported on both 4K X and 4K S.
    pub fn set_hdr_mapping(&self, mode: HdrToneMapping) -> Result<(), ElgatoError> {
        match self.model {
            DeviceModel::Elgato4KX => self.set_uvc_setting(mode.payload_4kx()),
            DeviceModel::Elgato4KS => self.send_hid_packet(&mode.payload_4ks()),
        }
    }

    /// Upload an EDID into one of the 4K X's seven flash slots.
    ///
    /// Enters the updater, follows both expected USB re-enumerations, and
    /// verifies the complete EDID table before returning.
    pub fn upload_edid(self, slot: u8, edid: &[u8]) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "Custom EDID upload",
                model: "4K S",
            });
        }
        let current_table = self.read_edid_table()?;
        self.upload_edid_from_table(&current_table, slot, edid)
    }

    /// Upload using a previously saved complete table as the source while
    /// still performing the required rescue transition.
    pub fn upload_edid_from_table(
        self,
        current_table: &[u8],
        slot: u8,
        edid: &[u8],
    ) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "Custom EDID upload",
                model: "4K S",
            });
        }
        let plan = EdidUploadPlan::build(current_table, slot, edid)?;

        // RTK_IO uses this recovery transition when its transport is not
        // already open. Linux starts without RTK's persistent port state, so
        // enter rescue and reopen the re-enumerated device before the update.
        self.enter_update_mode(&plan.entry)?;
        drop(self);

        let updater = ElgatoVcom::wait_for(Duration::from_secs(10))?;
        Self::execute_edid_plan(updater, &plan)
    }

    /// Resume an interrupted upload after `enter_rescue` using a previously
    /// saved complete EDID table.
    pub fn resume_edid_upload(
        current_table: &[u8],
        slot: u8,
        edid: &[u8],
    ) -> Result<(), ElgatoError> {
        let plan = EdidUploadPlan::build(current_table, slot, edid)?;
        let updater = ElgatoVcom::wait_for(Duration::from_secs(10))?;
        Self::execute_edid_plan(updater, &plan)
    }

    fn execute_edid_plan(
        mut updater: ElgatoVcom,
        plan: &EdidUploadPlan,
    ) -> Result<(), ElgatoError> {
        updater.send_update_packet(&plan.verify_trailer, "EE05 rescue authentication")?;
        updater.send_update_packet(&plan.start, "EE03 EDID update start")?;
        updater.send_update_packet(&plan.data_control, "EE00 EDID block metadata")?;
        updater.send_update_data(&plan.data)?;
        updater.send_update_packet(&plan.done, "EE04 EDID update completion")?;
        updater.send_finalize(&plan.finalize)?;
        drop(updater);

        Self::wait_for_edid_table(&plan.table, Duration::from_secs(15))
    }

    /// Finish an already completed rescue-mode update without rewriting data.
    pub fn finalize_pending_edid_update() -> Result<(), ElgatoError> {
        let mut updater = ElgatoVcom::wait_for(Duration::from_secs(10))?;
        updater.send_finalize(&rescue_finalize_packet())?;
        drop(updater);
        drop(Self::wait_for_4kx(Duration::from_secs(15))?);
        Ok(())
    }

    fn wait_for_4kx(timeout: Duration) -> Result<Self, ElgatoError> {
        let deadline = Instant::now() + timeout;
        loop {
            if ElgatoVcom::find_port().is_some() {
                if Instant::now() >= deadline {
                    return Err(ElgatoError::UvcTransfer(
                        "Elgato 4K X remained in RTK VCOM rescue mode after finalization".into(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            if let Ok(device) = Self::open() {
                if device.model == DeviceModel::Elgato4KX {
                    return Ok(device);
                }
            }
            if Instant::now() >= deadline {
                return Err(ElgatoError::UvcTransfer(
                    "Elgato 4K X did not re-enumerate before timeout".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn wait_for_edid_table(expected: &[u8], timeout: Duration) -> Result<(), ElgatoError> {
        let deadline = Instant::now() + timeout;
        let mut last_error = "normal device has not appeared".to_string();
        loop {
            if ElgatoVcom::find_port().is_none() {
                match Self::open() {
                    Ok(device) => match device.read_edid_table() {
                        Ok(table) if table == expected => return Ok(()),
                        Ok(_) => {
                            return Err(ElgatoError::UvcTransfer(
                                "complete EDID table readback does not match the requested update"
                                    .into(),
                            ));
                        }
                        Err(error) => last_error = error.to_string(),
                    },
                    Err(error) => last_error = error.to_string(),
                }
            }
            if Instant::now() >= deadline {
                return Err(ElgatoError::UvcTransfer(format!(
                    "EDID table readback did not become ready after re-enumeration: {}",
                    last_error
                )));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
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
        self.send_hid_packet(&input.payload_4ks())
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
        self.send_hid_packet(&scaler.payload_4ks())
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
        let _ack = self.send_at_command(AT_CMD_SET_USB_SPEED, &speed.at_input())?;
        Ok(())
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

            for &(known_pid, _) in PIDS_4KX {
                if pid == known_pid {
                    return Ok(FoundDevice { device, model: DeviceModel::Elgato4KX, pid });
                }
            }

            for &(known_pid, _) in PIDS_4KS {
                if pid == known_pid {
                    return Ok(FoundDevice { device, model: DeviceModel::Elgato4KS, pid });
                }
            }
        }

        Err(ElgatoError::DeviceNotFound)
    }
}

impl ElgatoVcom {
    fn open() -> Result<Self, ElgatoError> {
        let path = Self::find_port().ok_or(ElgatoError::DeviceNotFound)?;
        let port = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|error| {
                ElgatoError::UvcTransfer(format!(
                    "failed to open RTK VCOM port {}: {}",
                    path.display(),
                    error
                ))
            })?;
        Self::configure_raw(&port)?;
        Ok(Self { port })
    }

    fn find_port() -> Option<PathBuf> {
        for entry in std::fs::read_dir("/sys/class/tty").ok()?.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with("ttyACM") {
                continue;
            }
            let device = std::fs::canonicalize(entry.path().join("device")).ok()?;
            for ancestor in device.ancestors() {
                let vendor = std::fs::read_to_string(ancestor.join("idVendor")).ok();
                let product = std::fs::read_to_string(ancestor.join("idProduct")).ok();
                if vendor.as_deref().map(str::trim) != Some("0fd9") {
                    continue;
                }
                let product = u16::from_str_radix(product?.trim(), 16).ok()?;
                if PIDS_4KX.iter().any(|(pid, _)| *pid == product) {
                    return Some(Path::new("/dev").join(name));
                }
            }
        }
        None
    }

    fn configure_raw(port: &File) -> Result<(), ElgatoError> {
        let fd = port.as_raw_fd();
        let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr initializes `settings` on success; `fd` belongs to
        // the open tty and remains valid for the duration of these calls.
        if unsafe { libc::tcgetattr(fd, settings.as_mut_ptr()) } != 0 {
            return Err(Self::tty_error("tcgetattr"));
        }
        // SAFETY: `settings` was initialized by successful tcgetattr.
        let mut settings = unsafe { settings.assume_init() };
        // SAFETY: cfmakeraw only mutates the initialized termios structure.
        unsafe { libc::cfmakeraw(&mut settings) };
        settings.c_cflag |= libc::CLOCAL | libc::CREAD;
        // SAFETY: pointers and fd are valid as described above.
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &settings) } != 0 {
            return Err(Self::tty_error("tcsetattr"));
        }
        // SAFETY: fd is a valid tty descriptor.
        if unsafe { libc::tcflush(fd, libc::TCIOFLUSH) } != 0 {
            return Err(Self::tty_error("tcflush"));
        }
        Ok(())
    }

    fn tty_error(operation: &str) -> ElgatoError {
        ElgatoError::UvcTransfer(format!(
            "RTK VCOM {} failed: {}",
            operation,
            std::io::Error::last_os_error()
        ))
    }

    fn wait_for(timeout: Duration) -> Result<Self, ElgatoError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(device) = Self::open() {
                return Ok(device);
            }
            if Instant::now() >= deadline {
                return Err(ElgatoError::UvcTransfer(
                    "Elgato 4K X RTK VCOM rescue interface did not appear before timeout".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn send_update_packet(&mut self, packet: &[u8], operation: &str) -> Result<(), ElgatoError> {
        self.send_with_retry(packet, 3, operation)
    }

    fn send_update_data(&mut self, data: &[u8]) -> Result<(), ElgatoError> {
        self.send_with_retry(data, 5, "EDID data transfer")
    }

    fn send_finalize(&mut self, packet: &[u8]) -> Result<(), ElgatoError> {
        self.discard_pending_input()?;
        self.write_all(packet)?;
        self.port.flush().map_err(|error| {
            ElgatoError::UvcTransfer(format!("EE06 RTK VCOM flush failed: {}", error))
        })?;
        // SAFETY: the descriptor belongs to the open tty for this call.
        if unsafe { libc::tcdrain(self.port.as_raw_fd()) } != 0 {
            return Err(Self::tty_error("tcdrain after EE06"));
        }
        Ok(())
    }

    fn send_with_retry(
        &mut self,
        payload: &[u8],
        attempts: usize,
        operation: &str,
    ) -> Result<(), ElgatoError> {
        let mut last_response = Vec::new();
        for attempt in 0..attempts {
            self.discard_pending_input()?;
            self.write_all(payload)?;

            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                let mut response = [0u8; 3];
                match self.port.read(&mut response) {
                    Ok(length) if length > 0 => {
                        last_response = response[..length].to_vec();
                        if response[0] == 0x06 {
                            return Ok(());
                        }
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => {
                        return Err(ElgatoError::UvcTransfer(format!(
                            "{} RTK VCOM read failed: {}",
                            operation, error
                        )));
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if attempt + 1 < attempts {
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        Err(ElgatoError::UvcTransfer(format!(
            "{} did not return RTK VCOM completion byte 0x06 after {} attempts: {:02x?}",
            operation, attempts, last_response
        )))
    }

    fn discard_pending_input(&mut self) -> Result<(), ElgatoError> {
        let mut buffer = [0u8; 64];
        loop {
            match self.port.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => {
                    return Err(ElgatoError::UvcTransfer(format!(
                        "RTK VCOM input flush failed: {}",
                        error
                    )));
                }
            }
        }
    }

    fn write_all(&mut self, payload: &[u8]) -> Result<(), ElgatoError> {
        let mut offset = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        while offset < payload.len() {
            match self.port.write(&payload[offset..]) {
                Ok(0) => {
                    return Err(ElgatoError::UvcTransfer(
                        "RTK VCOM write made no progress".into(),
                    ));
                }
                Ok(written) => offset += written,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(ElgatoError::UvcTransfer("RTK VCOM write timed out".into()));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(ElgatoError::UvcTransfer(format!(
                        "RTK VCOM write failed: {}",
                        error
                    )));
                }
            }
        }
        Ok(())
    }
}

impl Drop for ElgatoDevice {
    fn drop(&mut self) {
        let interface_num = match self.model {
            DeviceModel::Elgato4KX => UVC_INTERFACE,
            DeviceModel::Elgato4KS => HID_INTERFACE,
        };

        let _ = self.handle.release_interface(interface_num as u8);

        // Restore both halves of the UVC function. The streaming interface can
        // remain unbound if only the extension-unit control interface is
        // reattached after a failed read.
        if self.control_driver_was_active {
            let _ = self.handle.attach_kernel_driver(interface_num as u8);
        }
        if self.streaming_driver_was_active {
            let _ = self
                .handle
                .attach_kernel_driver(UVC_STREAMING_INTERFACE as u8);
        }
    }
}
