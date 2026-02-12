# Low-Confidence / Risky Commands

> **Warning:** These commands were discovered via Ghidra decompilation of the 4K S MCU firmware (FW_4K_S_MCU.bin, ARM Cortex-M0) but carry significant risk of bricking or disrupting the device. They are documented here for reference but **NOT implemented** in the tool.

## Source

All commands below were found in `FUN_0000f42c` (main loop) of the MCU firmware, dispatched via the outer `cVar1` switch on the HID command byte at packet offset 5.

HID packet format: `[06, 06, 06, 55, 02, CMD_BYTE, PARAM, ...]` (255 bytes, zero-padded)

## Commands

### 0x08 — Hard Reset

**Firmware handler (line 20605):**
```
case 0x08:
  *DAT_0000f85c = 1;       // Assert reset signal
  FUN_000015d8(1000);       // Delay 1000ms
  *puVar20 = 0;             // De-assert reset signal
```

**Risk:** Hard-resets the ITE base chip. Will kill any active capture session. The base chip takes several seconds to reinitialize. If interrupted (e.g., USB disconnect during reset), could leave the device in a bad state.

**Packet:** `[06, 06, 06, 55, 02, 08, 00, ...]`

---

### 0x09 — Soft Reset

**Firmware handler (line 20440):**
```
case 0x09:
  FUN_00005934(0);          // Disable something
  FUN_000015d8(1000);       // Delay 1000ms
  FUN_00005934(1);          // Re-enable
```

**Risk:** Performs a soft reset cycle. Less dangerous than hard reset but still disrupts capture. Semantics of FUN_00005934 unclear.

**Packet:** `[06, 06, 06, 55, 02, 09, 00, ...]`

---

### 0x0a — System Reset

**Firmware handler (line 20447):**
```
case 0x0a:
  FUN_0000b460();           // Full system reset function
```

**Risk:** Calls a comprehensive system reset. More aggressive than soft reset. Could require device re-enumeration.

**Packet:** `[06, 06, 06, 55, 02, 0a, 00, ...]`

---

### 0x13 — Watchdog Infinite Loop (Device Hang)

**Firmware handler (line 20496):**
```
case 0x13:
  do {
    // WARNING: Do nothing block with infinite loop
  } while(true);
```

**Risk:** **extermely dangerous.** This intentionally hangs the MCU in an infinite loop, relying on the hardware watchdog timer (WDT) to trigger a full MCU reset. If the WDT is not properly configured or fails to fire, the device will be completely unresponsive until power-cycled (USB unplug/replug).

**Packet:** `[06, 06, 06, 55, 02, 13, 00, ...]`

---

### 0x14 — Toggle Peripheral

**Firmware handler (line 20497):**
```
case 0x14:
  DAT_0000fe68[4] = 0;     // Disable peripheral
  FUN_000015d8(100);        // Delay 100ms
  puVar10[4] = 1;           // Re-enable peripheral
```

**Risk:** Toggles an unknown peripheral. The 100ms off/on cycle suggests a reset of a specific hardware block. Could disrupt video/audio path.

**Packet:** `[06, 06, 06, 55, 02, 14, 00, ...]`

---

### 0x1a — Hard Peripheral Reset

**Firmware handler (line 20526):**
```
case 0x1a:
  DAT_0000fe68[8] = 0;     // Disable peripheral #8
  FUN_000015d8(1000);       // Delay 1000ms
  puVar10[8] = 1;           // Re-enable peripheral #8
```

**Risk:** Hard-resets a different peripheral than 0x14, with a full 1-second delay. Likely resets a critical hardware block (possibly the video capture pipeline).

**Packet:** `[06, 06, 06, 55, 02, 1a, 00, ...]`

---

### 0x1b — USB Re-enumeration

**Firmware handler (line 20533):**
```
case 0x1b:
  FUN_000028b8();           // USB re-initialization
```

**Risk:** Triggers USB bus re-enumeration. The device will disconnect and reconnect with potentially different USB parameters. Active capture sessions will be interrupted. The MCU calls `FUN_000028b8()` which populates the device info struct (chip ID, capabilities, display timings) and then reinitializes the USB interface.

**Packet:** `[06, 06, 06, 55, 02, 1b, 00, ...]`

---

### 0x22 — Full Reset

**Firmware handler (line 20558):**
```
case 0x22:
  FUN_000024dc();           // Full device reset
  *(undefined2 *)((int)DAT_0000fe70 + 2) = 0;
  // Falls through to main loop reset
```

**Risk:** Performs a comprehensive device reset including clearing state registers. More aggressive than 0x08/0x09. Clears internal state tracking variables.

**Packet:** `[06, 06, 06, 55, 02, 22, 00, ...]`

---

### 0x23 — Reboot

**Firmware handler (line 20565):**
```
case 0x23:
  FUN_00007a80();           // Pre-reboot cleanup
  FUN_000015d8(0x32);       // Delay 50ms
  FUN_00007a80();           // Cleanup again
  // Falls through to code that resets main loop state
```

**Risk:** Reboots the MCU. The device will be unresponsive during reboot. If the USB host doesn't handle the disconnection gracefully, could cause kernel errors.

**Packet:** `[06, 06, 06, 55, 02, 23, 00, ...]`

---

### 0x24 — Factory Reset

**Firmware handler (line 20566):**
```
case 0x24:
  FUN_00007a80();           // Pre-reset cleanup
  FUN_000015d8(0x32);       // Delay 50ms
  FUN_00007a80();           // Cleanup again
  FUN_0000f612();           // Reset to factory state
  // Falls into infinite loop (0x13 handler) → WDT reset
```

**Risk:** **very high risk if not highest.** Performs a factory reset, clearing all user-configured settings, then intentionally triggers the watchdog reset (infinite loop) to fully reboot. This will:
1. Erase all saved settings (EDID, color range, HDR, etc.)
2. Hang the MCU until WDT fires
3. Cause full device re-enumeration

**Packet:** `[06, 06, 06, 55, 02, 24, 00, ...]`

---

## Additional Discovered Commands (Non-Risky)

These commands were also found in the firmware but are either already implemented or benign:

| Cmd | Description | Status |
|-----|-------------|--------|
| 0x0b | Set color range (FUN_00003df8) | ✅ Implemented as `--hdmi-range` |
| 0x0c | Set video format + color (FUN_00007924 + FUN_00003df8) | Not implemented (need param mapping) |
| 0x0d | Audio routing - enable (FUN_00006020, param=1) | Partially overlaps with `--audio-input` |
| 0x0e | Audio routing - config (FUN_00006020) | Partially overlaps with `--audio-input` |
| 0x0f | Toggle on (FUN_00005904, param=1) | Not implemented (unknown toggle) |
| 0x10 | Toggle off (FUN_00005904, param=0) | Not implemented (unknown toggle) |
| 0x11 | Display mode set (FUN_00004394, param=0) | Not implemented (need param mapping) |
| 0x12 | Display mode set (FUN_00004394, param=1) | Not implemented (need param mapping) |
| 0x15 | Enable device | Not implemented |
| 0x16 | Disable device | Not implemented |
| 0x18 | Scaler setting (FUN_00003e90) | Overlaps with `--video-scaler` |
| 0x19 | Video scaler + mode (FUN_00007a80 + FUN_00003e90) | ✅ Implemented as `--video-scaler` |
| 0x1c | EDID control (FUN_0000ddcc, param=0) | Related to EDID, needs testing |
| 0x1d | EDID control (FUN_0000ddcc, param=1) | Related to EDID, needs testing |
| 0x1e | Read EDID (FUN_00009748) | Complex HW init, not a simple read |
| 0x1f | Enable flag (*DAT_0000fe68 = 1) | Unknown purpose |
| 0x20 | Disable flag (*DAT_0000fe68 = 0) | Unknown purpose |
| 0x21 | Switch input source | Not implemented (need testing) |

## HID Read Commands (Safe)

These read-only commands are safe and documented here for completeness:

| Sub-cmd | Read len | Description | Status |
|---------|----------|-------------|--------|
| 0x00 | 8 | Signal state / timing info | Not implemented (complex struct) |
| 0x01 | 7 | USB pipe mode | Not implemented |
| 0x02 | 4-8 | Firmware version | ✅ Implemented as `--firmware-version` |
| 0x08 | 1 | Audio input selection | ✅ Read in `--status` |
| 0x09 | 0x21 | HDMI HDR status packet | Not implemented (complex struct) |
| 0x0a | 1 | HDR tone mapping state | ✅ Read in `--status` |
| 0x0b | 1 | Color range state | ✅ Read in `--status` |
| 0x0c | 5 | Multi-byte config | Not implemented |
| 0x0d | 1 | Line-in audio gain (part 1) | Not implemented |
| 0x0e | 1-0x20 | Line-in audio gain / HDMI SPD info | Not implemented |
| 0x12 | 1 | EDID mode | ✅ Read in `--status` |
| 0x14 | 0x20 | Extended device info | Not implemented |
| 0x19 | 1 | Video scaler state | ✅ Read in `--status` |
| 0x1c | 1 | Unknown setting | Not implemented |
| 0x2d | 1 | Unknown setting | Not implemented |

## Methodology

All of the above was discovered through:
1. **Ghidra decompilation** of `FW_4K_S_MCU.bin` (ARM Cortex-M0, 69,960 bytes)
2. **EGAVDeviceSupport.dll decompilation** (Windows x64 DLL from Elgato software)
3. Cross-referencing firmware command handlers with DLL function calls
4. The HID read sub-commands were found by searching for `thunk_FUN_18008e710` calls with `0x55` as the cmd parameter
