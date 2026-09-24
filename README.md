# PressNots

PressNots is a Windows-only Rust console application that observes global mouse button events and raw reports from attached HID peripherals. It is intentionally dependency-free and uses the Win32 API directly.

## Requirements

- Windows 10 or later.
- A current stable Rust toolchain. The project uses Rust edition 2024.
- The same or greater integrity level as the applications whose mouse input should be observed or suppressed. Run from an elevated terminal when testing elevated applications.

## Run

Observe events without changing their delivery:

```powershell
cargo run --release
```

Observe and suppress standard mouse button down/up events:

```powershell
cargo run --release -- --block-mouse
```

Press `Ctrl+C` to stop. Use suppression carefully because the mouse buttons remain blocked until the process exits.

Example output:

```text
mouse button=x1 action=down x=1249 y=773
mouse button=x1 action=up x=1249 y=773
hid device=0x1234 name="...VID_1234..." report_size=4 report_count=1 data=[05 00 80 00]
```

## Capture paths

PressNots uses two complementary Windows input APIs:

1. A `WH_MOUSE_LL` low-level hook decodes left, right, middle, X1, and X2 mouse button down/up messages. The `--block-mouse` option can suppress only these decoded messages.
2. Raw Input discovers every attached non-keyboard HID top-level collection and registers its usage page and usage. Reports are printed as hexadecimal bytes so uncommon buttons are visible even before their device-specific format is known.

Raw Input registrations apply to usage classes. A device connected later is captured when its usage class was present at launch. Restart PressNots after attaching a device with a previously unseen usage class.

## Uncommon input coverage

The unit suite covers cases often omitted from mouse and HID implementations:

- Middle-button down and up.
- X1 and X2, including isolation of their identifier from the high word of `mouseData`.
- An unknown future X-button identifier, reported as `x-unknown` rather than incorrectly treated as X2.
- Wheel, horizontal-wheel, and movement messages being ignored as non-click input.
- Multiple HID reports delivered in one `WM_INPUT` message.
- Truncated, zero-length, zero-count, and multiplication-overflow HID payloads.
- High-bit button masks, active-low electrical semantics, report IDs, device filters, and short reports.

Run all checks with:

```powershell
cargo test
cargo fmt -- --check
cargo clippy -- -D warnings
```

The pure `decode_mouse_message`, `parse_hid_payload`, and `decode_hid_buttons` functions are the test boundaries. Win32 callbacks delegate to them, so tests exercise the same decoding used at runtime without installing global hooks.

## Add a standard Windows mouse button

When Windows exposes a new button as a distinct low-level mouse message:

1. Add its `WM_*DOWN` and `WM_*UP` constants beside the existing message constants in `src/main.rs`.
2. Add both cases to `decode_mouse_message` and choose a stable output name.
3. Add down/up assertions to the `tests` module.
4. Run the three checks above, then perform a real-device smoke test.

Do not classify wheel messages as clicks. A double-click is normally represented at this hook level by two down/up sequences rather than a separate low-level message.

## Add a vendor HID button

Vendor controls have no universal byte format. Obtain the report layout from the HID report descriptor, vendor documentation, or recordings made while pressing only that control. Then add an entry to `CUSTOM_HID_BUTTONS` in `src/main.rs`:

```rust
const CUSTOM_HID_BUTTONS: &[HidButtonDefinition] = &[HidButtonDefinition {
	name: "rear-paddle-4",
	device_name_contains: "VID_1234&PID_5678",
	report_id: Some(5),
	byte_offset: 2,
	bit_mask: 0x80,
	active_low: false,
}];
```

Field meanings:

- `name`: Stable text emitted in `hid-button` events.
- `device_name_contains`: A case-sensitive substring from the printed Raw Input device name. Include both VID and PID when possible to avoid matching unrelated devices.
- `report_id`: The expected first report byte, or `None` when the device has no report ID.
- `byte_offset`: Byte position in the complete report, including the report ID byte when present.
- `bit_mask`: The bit or bit group representing the button. Prefer one entry per independent button bit.
- `active_low`: `false` when a set bit means pressed; `true` when a cleared bit means pressed.

Mapped buttons emit only state transitions:

```text
hid-button device=0x1234 button=rear-paddle-4 action=down
hid-button device=0x1234 button=rear-paddle-4 action=up
```

Add a unit test using a local `HidButtonDefinition` array before putting the definition in `CUSTOM_HID_BUTTONS`. Test at least pressed, released, wrong report ID, wrong device, and a report shorter than `byte_offset`.

## Limits and safety

- There is no device-independent meaning for vendor-defined HID report bytes. A correct definition is necessarily specific to a device and firmware.
- Hardware hidden behind a vendor service, non-HID protocol, or proprietary driver may require that vendor's SDK or a custom driver.
- Windows secure desktops and higher-integrity processes can prevent a normal process from observing or suppressing input.
- Raw HID reports are observed but cannot be suppressed through Raw Input. Device-specific suppression generally requires a filter driver.
- PressNots deliberately excludes keyboard Raw Input and does not log keystrokes.
- The current output may contain repeated raw HID state reports; named custom button events are edge-filtered.

## Project layout

```text
PressNots/
|-- Cargo.toml       Package and release configuration
|-- README.md        Usage, architecture, extension, and test guide
`-- src/
	`-- main.rs      Win32 bindings, hooks, decoders, handlers, and tests
```

## Build a standalone executable

```powershell
cargo build --release
```

The executable is written to `target\release\pressnots.exe`. No Rust installation is required on the destination machine.