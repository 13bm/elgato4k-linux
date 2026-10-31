# Custom EDID Upload (4K X)

Upload a custom EDID to the 4K X's internal flash table. EDID upload is not
supported on the 4K S and does not require a kernel patch.

## Upload an EDID

Stop capture applications and other capture-card control software before
starting. Connect only the card you intend to update: USB commands select the
first supported card, without a serial-number selector.

Back up the complete EDID table, then upload your file:

```sh
sudo elgato4k-linux --dump-edid-table edid-table-backup.bin
sudo elgato4k-linux --upload-edid custom.bin --slot 0 --confirm-edid-write
```

The utility accepts 128, 256, 384 or 512-byte EDID files and checks the header,
extension count and checksum of every block. These checks validate the file
structure; advertised modes must also be supported by the HDMI source, cable
and receiver.

The upload reads the current table, replaces the supplied bytes in the chosen
slot, writes the table and verifies the complete readback. Remaining bytes in
the slot and all other slots are preserved. The card disconnects and
re-enumerates during the update.

> **Warning:** Uploading writes capture-card flash. Do not disconnect USB or
> remove power until the command finishes successfully. An interrupted update
> can leave the card in updater mode. Keep the backup for recovery.

Dumping the table does not write flash, but both dump and upload use libusb
and interrupt capture by temporarily detaching the video driver.

## Slots and EDID Source

The flash table is 3584 bytes (`0xE00`), containing seven 512-byte slots
numbered 0 through 6. `--slot` selects a storage location to update, not an
active preset. The utility does not provide an active-slot selector or a
mapping of all slots to HDMI inputs; do not assume all seven are selectable
presets. The example above writes slot 0.

EDID source policy is a separate setting:

| Policy | EDID presented to the HDMI source |
| --- | --- |
| `display` | Passthrough display's EDID |
| `merged` | Capabilities merged by the capture card |
| `internal` | Capture card's internal EDID |

To use the internal EDID:

```sh
sudo elgato4k-linux --edid-source internal
```

After a successful upload, the card may need a USB power cycle to load the
updated table into its active HDMI EDID buffer. The HDMI source can also
cache its previous EDID. To request source rediscovery:

```sh
sudo elgato4k-linux --hdmi-hpd off
sudo elgato4k-linux --hdmi-hpd on
```

These commands interrupt the HDMI link. An HPD pulse does not replace a card
power cycle if the active EDID buffer has not been refreshed.

## Advanced Update Options

Use these options only when you have a complete table backup and know the
card's update state. Upload options still require `--confirm-edid-write`.

| Option | Behavior |
| --- | --- |
| `--source-table <PATH>` | With `--upload-edid`, uses a saved full table instead of reading the current table, then enters updater mode and performs the upload. |
| `--resume-table <PATH>` | With `--upload-edid`, uses a saved full table and starts the upload with the card already in RTK VCOM updater mode. |
| `--finalize-edid-update --confirm-edid-write` | Sends only the global update finalizer (`EE06`). Requires all per-image update stages to have completed; does not rewrite or verify the table. |

A saved table must be exactly `0xE00` bytes. Using an older backup can
overwrite newer contents in other slots. If an update fails, retain the error
output and backup before choosing a recovery operation.

## Rust API

- `validate_edid()` validates an EDID without accessing hardware.
- `EdidUploadPlan::build()` prepares the updated table and protocol packets
  without accessing hardware.
- `ElgatoDevice::upload_edid()` reads the current table, uploads the selected
  EDID and verifies the full table before returning success.

The upload consumes the device handle because the card re-enumerates.
Saved-table and resume variants are also available. API callers are
responsible for write confirmation and stopping capture before an update.
