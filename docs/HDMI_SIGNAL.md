# Live HDMI Input Timing (4K X)

Read the HDMI input's resolution, refresh rate, interlace flag and signal
state without stopping capture:

```sh
elgato4k-linux --signal
```

Example output:

```text
HDMI input: 1920x1080p @ 59.99 Hz
```

The refresh rate is reported in hundredths of a hertz. This is the HDMI
receiver's input timing, not the USB capture format or delivered capture FPS.
Disconnected, unlocked and changing signals are reported separately rather
than showing an old timing as current.

This feature is available for the 4K X only. It does not report TMDS error
counters or decode HDR infoframes.

## Device Selection and Permissions

The utility automatically selects the primary video node when exactly one
4K X is available. With multiple cards, specify the video node:

```sh
elgato4k-linux --signal --video-device /dev/video0
```

Your account needs read/write access to that node. The utility checks its
V4L2 subsystem and USB identity before sending queries.

`--signal` uses `UVCIOC_CTRL_QUERY` through the bound video driver. It does
not detach the driver, reset the card or restart capture. Run it separately
from other control actions: commands such as `--status` use libusb and can
interrupt capture. There is no libusb fallback if the live query fails.

An advisory lock coordinates this utility's queries on the same video node.
Do not run other vendor-control tools against the card's command mailbox at
the same time.

## Driver Compatibility

The 4K X uses variable-length replies on a UVC extension control. Some Linux
`uvcvideo` drivers cache the initial control length and reject later transfers
of a different length with `ENOBUFS`. On those kernels, live signal queries
require the [variable-length XU patch](../tools/kernel/uvcvideo-elgato-variable-xu.patch).
No patch is needed if your driver already supports these transfers.

The patch permits 1 through 4096-byte raw `GET_CUR` and `SET_CUR` transfers
only for USB vendor `0fd9`, 4K X product IDs `009b`, `009c` and `009d`,
extension unit 4, selector 1. Other devices, mapped controls, capability
checks and locking are unchanged. It does not modify capture formats, URBs,
IRQ handling or audio behavior.

To use the patch:

1. Obtain the source for your exact running kernel, including distribution
   patches, and apply the supplied patch.
2. Build the matching `uvcvideo` module and retain the original for rollback.
3. Stop all applications using the module before replacing and reloading it.
4. Rebuild for subsequent kernel updates as needed.

Replacing the module interrupts capture for devices using it. Once loaded,
the patched module allows signal queries during capture. The project does
not include an automatic module installer or DKMS configuration.

This driver requirement applies only to the live V4L2 query on affected
kernels. EDID upload and other libusb operations do not require the patch.

## Rust API

- `find_hdmi_video_device()` locates the 4K X video node.
- `read_hdmi_signal_v4l2(path)` queries signal state without detaching the
  driver and returns `HdmiSignal`, with `HdmiTiming` for a locked signal.
- `ElgatoDevice::read_hdmi_signal()` provides the libusb equivalent. It does
  not require the driver patch, but opening that transport interrupts capture.

Use the V4L2 API when capture must remain active.
