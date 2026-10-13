# Clawdmeter copy deck

Source inventory for the app UI, tray, CLI output and diagnostics, plus device on-screen text and simulator-facing copy. Locations are relative to the repository root and reflect the review snapshot; match the current text if another player has moved a line. Repeated literals retain separate rows. An unchanged new-text cell is an explicit keep decision, not unreviewed copy.

Apply prose only. Do not replace protocol keys, enum serialization, hook event names, URLs, filenames, IDs, or format placeholders. The app forwards some errors from core/flash crates; those defining locations appear below as well. Dynamic account paths, versions, OS errors, alert messages and diff contents are runtime data, not fixed copy. Keep that data intact.

Rows marked LOGIC or CONDITIONAL need the condition described in the note. REMOVE means remove the UI element, not display that word. Normal button labels and proper product names are intentionally retained where already clear. Device proposals use ASCII and need native-size rendering after application; no firmware or Rust source is edited by this deck.

## Decided device status behavior

The owner has chosen to keep Claude Code's whimsical thinking verbs as the
all-is-well indicator. The same status line shows Waiting, Stale, or No data
for other states, with the existing Listening alternation and animation
format. Minimal display retains fixed status text. These are keep decisions,
not proposals to replace the verbs with Connected.

## Shared rules

- Use Settings in navigation; preserve any legacy CLI aliases.
- “Updated” means last successful usage fetch. A heartbeat is a cached replay.
- Use an ellipsis for an operation in progress in the desktop app. Owner decision: preserve the device status spinner, thinking-verb rotation, and ellipsis as its all-is-well indicator.
- Keep errors factual, with a next step when one is known. Do not infer rate limiting from every missing response.
- Keep device Waiting/Stale/No data labels and the existing thinking verbs. Test the longest verbs at 240×240; do not shorten the decided verb list in this copy pass.


## desktop/crates/clawdmeter-app/src/main.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/main.rs:16 | clawdmeter - Claude Code usage on your desk<br><br>USAGE:<br>    clawdmeter [OPTIONS]<br><br>With no options, runs the tray app: the daemon plus a tray icon (or the app<br>window on desktops without a tray).<br><br>OPTIONS:<br>    --daemon          Run headless: poll usage, follow hooks, drive the device over BLE<br>    --window [PAGE]   Open the app window on a page: overview (default), device,<br>                      flash, accounts, hooks, settings or setup<br>      [--manifest P]  flash page: use a local release manifest.json instead of GitHub<br>    --once            Poll /usage for every account once, print the payload, exit (no BLE)<br>    --ports           List attached Espressif USB ports<br>    --info PORT       Ask the firmware on PORT for its env/version<br>    hook MODE [event] Claude Code hook (MODE: alert, clear or event); reads the<br>                      hook JSON on stdin, prints nothing, always exits 0<br>    -V, --version<br>    -h, --help<br><br>ENVIRONMENT:<br>    CLAWDMETER_LOG       error&#124;warn&#124;info&#124;debug (default info)<br>    CLAWDMETER_NO_TRAY   1 = show the app window instead of a tray icon<br>    POLL_INTERVAL        default poll interval (the config file overrides it)<br> | clawdmeter - Claude Code usage on your desk<br><br>USAGE:<br>    clawdmeter [OPTIONS]<br><br>With no options, runs the tray app: usage updates plus a tray icon (or the app<br>window on desktops without a tray).<br><br>OPTIONS:<br>    --daemon          Run headless: poll usage, follow hooks, send updates over Bluetooth LE<br>    --window [PAGE]   Open the app window on a page: overview (default), device,<br>                      flash, accounts, hooks, settings or setup<br>      [--manifest P]  flash page: use a local release manifest.json instead of GitHub<br>    --once            Poll /usage for every account once, print the payload, exit (no BLE)<br>    --ports           List attached Espressif USB ports<br>    --info PORT       Ask the firmware on PORT for its env/version<br>    hook MODE [event] Claude Code hook (MODE: alert, clear or event); reads the<br>                      hook JSON on stdin, prints nothing, always exits 0<br>    -V, --version<br>    -h, --help<br><br>ENVIRONMENT:<br>    CLAWDMETER_LOG       error&#124;warn&#124;info&#124;debug (default info)<br>    CLAWDMETER_NO_TRAY   1 = show the app window instead of a tray icon<br>    POLL_INTERVAL        default poll interval (the config file overrides it)<br> | Keep CLI syntax and environment variable names. Page label is Settings; preserve background alias if supported. |
| desktop/crates/clawdmeter-app/src/main.rs:79 | --window takes one of: overview, device, flash, accounts, hooks, settings, setup | --window takes one of: overview, device, flash, accounts, hooks, settings, setup | Show canonical page names, preserve accepted aliases in parsing. |
| desktop/crates/clawdmeter-app/src/main.rs:88 | {}  {:04x}:{:04x}  {} | {}  {:04x}:{:04x}  {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:95 | env={} ver={} rev={} link={} | env={} ver={} rev={} link={} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:99 | no reply (not running Clawdmeter firmware, or too old for &#96;info&#96;) | No firmware reply. Check USB and whether the board supports the info command. | No reply alone does not establish the cause. |
| desktop/crates/clawdmeter-app/src/main.rs:103 | {e} | {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:108 | --info needs a port | --info needs a port | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:113 | clawdmeter {} | clawdmeter {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:117 | {USAGE} | {USAGE} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:121 | unknown option {other}\n\n{USAGE} | unknown option {other}\n\n{USAGE} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:133 | the old Clawdmeter daemon is still set to start at login ({}); it will fight this app for the device. Turn it off on the app's Settings page | A legacy Clawdmeter service is enabled ({}). Disable it in Settings so only one process connects to the device. | Console warning; page rename. |
| desktop/crates/clawdmeter-app/src/main.rs:145 | Clawdmeter is already running (tray app or --daemon); not starting a second daemon | Clawdmeter is already running (tray app or --daemon); not starting a second daemon | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:149 | instance lock unavailable ({e}); continuing | instance lock unavailable ({e}); continuing | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:156 | runtime: {e} | runtime: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/main.rs:161 | === Clawdmeter daemon {} === | === Clawdmeter daemon {} === | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/art.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/art.rs:64 | Connected | Connected | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/art.rs:65 | Stale | Usage out of date | Desktop label; firmware has its own short variant below. |
| desktop/crates/clawdmeter-app/src/art.rs:66 | Searching | Looking for device | Tray label only; never change serialized state identifiers. |
| desktop/crates/clawdmeter-app/src/art.rs:67 | Signed out | Signed out | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/art.rs:68 | Needs you | Claude needs you | Tray state label. |

## desktop/crates/clawdmeter-app/src/onboarding.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/onboarding.rs:17 | on | on | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/errors.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/errors.rs:10 | No permission to write there. | Write permission denied. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/errors.rs:11 | The folder doesn't exist. | The file or folder was not found. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/errors.rs:12 | That disk is read-only. | The disk is read-only. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/errors.rs:13 | The disk is full. | The disk is full. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/errors.rs:27 | Couldn't save {what} ({}). {} | Couldn't save {what} ({}). {} | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/daemon.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/daemon.rs:104 | tokio runtime | tokio runtime | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:107 | spawn daemon thread | spawn daemon thread | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:120 | claude CLI not found (set claude_path in the config) | claude CLI not found (set claude_path in the config) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:131 | config unreadable ({e}); using defaults | config unreadable ({e}); using defaults | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:135 | accounts: {} | accounts: {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:149 | claude CLI: {} | claude CLI: {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:150 | claude CLI not found; set claude_path in {} | claude CLI not found; set claude_path in {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:202 | daemon stopped | daemon stopped | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:225 | hook {:?} {} | hook {:?} {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:238 | refresh requested by device | refresh requested by device | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:246 | {}: {e} | {}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:254 | polling /usage ({:?}): {} account(s) | polling /usage ({:?}): {} account(s) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:264 | connected to {} ({}) | connected to {} ({}) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:279 | {e}; retrying in {wait}s | {e}; retrying in {wait}s | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:282 | connect task failed: {e} | connect task failed: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:293 | connected at the OS level | connected at the OS level | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:296 | device {} {how}; reconnecting now | device {} {how}; reconnecting now | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:314 | device disconnected, reconnecting... | device disconnected, reconnecting... | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:330 | Sending | Sending | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:331 | Heartbeat | Heartbeat | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:332 | Attention | Attention | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:333 | Event | Event | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:334 | No data | No data | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:336 | {label}: {} | {label}: {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:343 | write failed: {e} | write failed: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:370 | config: {e} | config: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:381 | {}: spend limit {}% used | {}: spend limit {}% used | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:386 | {}: session {}%{}, week {}% | {}: session {}%{}, week {}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:389 |  (window not started) |  (window not started) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:393 | {}: {e} | {}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/daemon.rs:404 | {} | {} | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/lib.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/lib.rs:97 | Overview | Overview | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:98 | Device | Device | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:99 | Flash | Flash | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:100 | Accounts | Accounts | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:101 | Hooks | Hooks | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:102 | Settings | Settings | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:103 | Setup | Setup | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:121 | {s} s ago | {s} s ago | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:122 | {} min ago | {} min ago | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/lib.rs:123 | {} h ago | {} h ago | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/boards.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/boards.rs:30 | AMOLED 2.16 | AMOLED 2.16 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:31 | Waveshare ESP32-S3-Touch-AMOLED-2.16 | Waveshare ESP32-S3-Touch-AMOLED-2.16 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:35 | 480×480 · 3 buttons | 480×480 · 3 buttons | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:36 | ESP32-S3 | ESP32-S3 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:40 | AMOLED 1.8 | AMOLED 1.8 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:41 | Waveshare ESP32-S3-Touch-AMOLED-1.8 | Waveshare ESP32-S3-Touch-AMOLED-1.8 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:45 | 368×448 · 2 buttons | 368×448 · 2 buttons | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:46 | ESP32-S3 | ESP32-S3 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:50 | AMOLED 2.06 | AMOLED 2.06 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:51 | Waveshare ESP32-S3-Touch-AMOLED-2.06 | Waveshare ESP32-S3-Touch-AMOLED-2.06 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:55 | 410×502 · watch | 410×502 · watch | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:56 | ESP32-S3 | ESP32-S3 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:60 | LCD 1.54 | LCD 1.54 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:61 | Waveshare ESP32-S3-Touch-LCD-1.54 | Waveshare ESP32-S3-Touch-LCD-1.54 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:65 | 240×240 · SPI TFT | 240×240 · SPI TFT | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:66 | ESP32-S3 | ESP32-S3 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:70 | LCD 4\" | LCD 4\" | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:71 | Waveshare ESP32-S3-Touch-LCD-4 | Waveshare ESP32-S3-Touch-LCD-4 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:75 | 480×480 · RGB TFT | 480×480 · RGB TFT | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:76 | ESP32-S3 | ESP32-S3 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:80 | AMOLED 2.16 (C6) | AMOLED 2.16 (C6) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:81 | Waveshare ESP32-C6-Touch-AMOLED-2.16 | Waveshare ESP32-C6-Touch-AMOLED-2.16 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:85 | 480×480 · ESP32-C6 | 480×480 · ESP32-C6 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:86 | ESP32-C6 | ESP32-C6 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:90 | AMOLED 1.8 (C6) | AMOLED 1.8 (C6) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:91 | Waveshare ESP32-C6-Touch-AMOLED-1.8 | Waveshare ESP32-C6-Touch-AMOLED-1.8 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:95 | 368×448 · ESP32-C6 | 368×448 · ESP32-C6 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/boards.rs:96 | ESP32-C6 | ESP32-C6 | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/ui/overview.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:87 | {s} s | {s} s | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:88 | {} min | {} min | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:89 | {} min {} s | {} min {} s | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:90 | {} h {} min | {} h {} min | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:131 | Bluetooth is off | Bluetooth unavailable | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:132 | Connected | Connected | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:133 | Waiting for fresh numbers | Usage out of date | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:134 | Looking for your Clawdmeter | Looking for Clawdmeter | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:135 | Signed out of Claude Code | Signed out of Claude Code | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:136 | Claude needs you | Claude needs you | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:143 | {p}% | {p}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:144 | – | – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:171 | Not running | Not running | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:171 | No live usage until Clawdmeter starts. | Start Clawdmeter to update usage. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:174 | Run claude in a terminal to sign in again. | Open Claude Code in a terminal and sign in. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:175 | {} · updated {} | {} · updated {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:177 | No usage yet | No usage data yet | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:211 | Not started | Not started | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:216 | Session | Session | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:217 | Weekly | Weekly | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:222 | Firmware v{v} is out. Updating takes about a minute over USB. | Firmware v{v} is available. Connect USB to update. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:226 | Update firmware | Update firmware | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:243 | Checking… | Checking… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:244 | Can check again in {} | Can check again in {} | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/ui/settings.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:173 | Couldn't read {}: {e} | Couldn't read {}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:186 | Couldn't find the Clawdmeter program itself. | Couldn't locate the Clawdmeter executable. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:270 | minimal {mode} | minimal {mode} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:282 | The device didn't answer. Unplug it, plug it back in and try again. | No response from the device. Reconnect USB and try again. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:295 | Add Claude config folder | Add Claude Code account folder | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:326 | {f}: {e} | {f}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:332 | Already up to date. | No changes needed. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:344 | Couldn't write {what}. {} | Couldn't write {what}. {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:350 | 1 account | 1 account | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:350 | {done} accounts | {done} accounts | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:355 | Hooks set up in {accounts}. The old file is saved as settings.json.bak. | Hooks installed in {accounts}. Existing files were backed up to settings.json.bak. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:358 | Hooks removed from {accounts}. The old file is saved as settings.json.bak. | Hooks removed from {accounts}. Existing files were backed up to settings.json.bak. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:367 | Test alert from Clawdmeter settings | Clawdmeter test alert | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:371 | Alert queued, but Clawdmeter isn't running, so nothing will show it. | Clawdmeter isn't running. Start it, then send another test alert. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:373 | Test alert sent. Clawd should come find you; tap the device to dismiss. | Test alert queued. Check the device; tap it to dismiss. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:374 | Couldn't send the test alert. {} | Couldn't send the test alert. {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:396 | Board not identified yet | Board not identified | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:400 | Clawdmeter isn't running | Clawdmeter isn't running | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:402 | Connected over Bluetooth | Bluetooth connected | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:403 | Bluetooth is off | Bluetooth unavailable | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:404 | Connecting… | Connecting… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:405 | Looking for it over Bluetooth | Looking for Clawdmeter | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:409 | Checking USB… | Checking USB… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:410 | On USB at {p} | USB: {p} | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:411 | Plug it in over USB to identify it | Connect USB to identify the board. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:412 | Not on USB | USB not connected | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:416 | Firmware v{} · v{} is out | Firmware v{} · v{} available | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:418 | Firmware v{} | Firmware v{} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:419 | Firmware version unknown | Firmware version unknown | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:426 | Update firmware | Update firmware | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:427 | Flash firmware… | Flash firmware… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:443 | Update available | Update available | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:460 | Device | Device | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:469 | A still Clawd and the meters, no animation. Currently {}. | Meters and a still Clawd. Minimal display is {}. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:469 | on | on | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:469 | off | off | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:471 | Asking the device… | Reading display mode… | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:472 | This firmware is too old for minimal mode. Update it first. | Couldn't read display mode. Reconnect USB or update the firmware. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:474 | A still Clawd and the meters, no animation. Changed over USB, so plug the device in. | Meters and a still Clawd. Connect USB to change this setting. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:479 | Minimal display | Minimal display | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:484 | Auto | Board default | Only minimal-display Auto. Clock Auto keeps its meaning. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:484 | on | on | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:484 | On | On | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:484 | off | off | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:484 | Off | Off | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:489 | on | on | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:489 | on | on | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:490 | off | off | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:490 | off | off | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:504 | Clock | Clock | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:505 | Shows the time at the top of the screen. Auto follows this computer's 12/24-hour setting. | Replaces the Usage title with a clock. Auto follows this computer's time format. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:510 | Off | Off | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:511 | Auto | Auto | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:512 | 12 h | 12 h | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:513 | 24 h | 24 h | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:530 | Beep when Claude needs you | Attention beep | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:531 | Plays on boards with a speaker. You can mute it for a while from the tray or the Overview. | Sound for attention alerts on supported boards. Mute from the tray until restart. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:541 | Chime when the session resets | Session reset chime | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:542 | One short chime when your 5-hour window rolls over. | Sound when a session reset is detected, on supported boards. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:554 | Check for firmware updates | Check for firmware updates | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:555 | Looks for new firmware on GitHub once a day and tells you here and in the tray. | Checks GitHub daily. Updates appear here and in the tray. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:566 | Forget this device | Clear saved device | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:567 | Drops the remembered address and picks up whichever paired Clawdmeter is around. Use it after swapping boards. | Clears the app's saved address and searches for a paired Clawdmeter. Bluetooth pairing stays in place. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:568 | Forget | Clear | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:578 | Accounts | Accounts | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:581 | Add folder… | Add account folder… | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:590 | Each Claude Code config folder is an account: the CLAUDE_CONFIG_DIR you run claude with. Clawdmeter checks all of them; the device shows whichever you used most recently. | Add a Claude Code config folder for each account you want to monitor. The device follows the account whose session usage increased most recently. | Add default-folder guidance; see UX review. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:603 | Retrying | Retrying | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:604 | Problem | Needs attention | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:605 | Shown on the device | REMOVE duplicate subtitle | Retain On device badge. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:605 | On device | On device | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:606 | Checked, not shown | Not selected for the device | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:607 | Not checked while Clawdmeter is stopped | Start Clawdmeter to check usage. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:633 | Remove | Remove | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:655 | Hooks | Hooks | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:657 | Hooks let Clawd react while you work: he notices prompts, keeps up with tools, and comes to find you when Claude Code is waiting. Each account gets five small entries in its settings.json that record only the event type and session ID. Without them Clawdmeter still works; it just checks every 2 minutes. | Hooks let Clawd react to activity and show alerts. The event log stores timestamps, event types, and session IDs. Alerts also store notification or question text for the device. Usage meters work without hooks. | Privacy correction; remove incorrect 2-minute cadence claim. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:668 | All {} hooks set up | {} hooks installed | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:669 | Installed | Installed | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:670 | Remove | Review removal | Row action opens preview. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:673 | Not set up. Hooks you added yourself stay as they are. | Clawdmeter hooks are not installed. Other hooks are kept. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:674 | Not installed | Not installed | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:675 | Install | Review install | Row action opens preview. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:678 | Uses the old attention-hook.sh script. Updating switches to the built-in hook. | Uses attention-hook.sh. Update to the built-in Clawdmeter hooks. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:679 | Old script | Legacy hooks | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:680 | Update | Review update | Row action opens preview. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:683 | Points at {}, not this copy of Clawdmeter. | Uses another Clawdmeter executable: {}. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:684 | Out of date | Out of date | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:685 | Update | Review update | Row action opens preview. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:688 | Some hooks are missing. | Some hooks are missing. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:689 | Incomplete | Incomplete | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:690 | Update | Review update | Row action opens preview. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:692 | Can't read | Unreadable | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:743 | Review the change to 1 file | Review 1 file | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:744 | Review the changes to {n} files | Review {n} files | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:745 | Review the removal | Review hook removal | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:749 | Save changes | Install hooks | Only the HookAction::Install preview branch; removal keeps Remove hooks. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:750 | Remove hooks | Remove hooks | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:765 | Cancel | Cancel | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:773 | Nothing is written until you save. The old file is kept as settings.json.bak. | Review before applying. Existing files are backed up to settings.json.bak. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:813 | {file} (new file) | {file} (new file) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:814 | {file} (also re-indented) | {file} (also re-indented) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:825 | 1 account | 1 account | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:825 | {n} accounts | {n} accounts | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:826 | Update {accounts} | Review updates for {accounts} | Opens preview. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:826 | Install in {accounts} | Review installation in {accounts} | Opens preview, does not write yet. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:844 | Send a test alert | Send test alert | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:867 | Settings | Settings | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:869 | Start at login | Start at login | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:870 | Starts Clawdmeter in the tray when you log in. | Opens Clawdmeter when you sign in. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:881 | Couldn't change Start at login. {} | Couldn't change Start at login. {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:895 | Automatic | Automatic | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:896 | Every 2 min | While working: 2 min | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:897 | Every 5 min | While working: 5 min | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:902 | Every {n} s | While working: {n} s | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:909 | Usage checks | Usage checks | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:910 | Automatic checks every 3 minutes while you work and every 15 minutes otherwise (every 5 minutes without hooks); other accounts every 10 minutes. Claude Code's usage service is rate-limited, so an account is never checked twice within 2 minutes. Checks never start a session window. | Checks every 3 minutes while working, then less often when quiet. Retry delays apply to all checks, including manual refresh. | Put full cadence in docs. Custom choices affect working and no-hooks cadence, not quiet/other accounts. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:936 | Keep-alive | Device heartbeat | REMOVE normal GUI control; config remains. New text is for an optional Advanced diagnostics view. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:937 | Resends the last numbers so the device doesn't mark them stale between checks. | Resends cached readings between usage checks. Default: 60 seconds. | REMOVE normal GUI control; config remains. New text is for an optional Advanced diagnostics view. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:938 | 60 s | 60 s | REMOVE normal GUI control; config remains. New text is for an optional Advanced diagnostics view. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:938 | 30 s | 30 s | REMOVE normal GUI control; config remains. New text is for an optional Advanced diagnostics view. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:948 | Claude Code CLI | Claude Code executable | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:950 | Using {} | Usage is read with {}. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:951 | Not found. Set claude_path in the config file. | Claude Code wasn't found. Set claude_path in the config file. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:952 | Shown while Clawdmeter is running. | Start Clawdmeter to detect Claude Code. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:959 | Log file | Log file | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:961 | Open | Open log | Name the destination; confirm context if line numbers moved. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:966 | Config file | Config file | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:968 | Open | Open config | Name the destination; confirm context if line numbers moved. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:973 | Quit Clawdmeter | Quit Clawdmeter | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:974 | Stops the background part and closes this window. The device keeps its last numbers until they go stale. | Stops usage updates and closes Clawdmeter. The device retains its last reading. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:975 | Quit | Quit | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:985 | The old background service is still installed | Legacy service detected | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:987 | {} talks to the device too, and the two will fight over it. | Disable {} so only one Clawdmeter process connects to the device. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:990 | Turn it off | Disable legacy service | Replace copy; preserve format arguments. |

### Hooks: alert types (round 9)

Final copy for the alert-type controls. These rows supersede any earlier
wording for this block. Keep the existing event mapping and defaults;
Idle sessions remains off by default. Alert expiry and dismissal belong
with alert behavior, not this selection block, so omit them here.

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:989 | Alert me for | Alert types | Section heading. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:990 | Applies to every account. A turned-off kind never reaches the device or the tray. | Applies to all accounts, on the device and in the tray. | Shared scope; checkbox state explains inclusion. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:980 | Permission requests | Tool approvals | Permission notification label. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:980 | A tool is waiting for your approval. | Claude needs permission to run a tool. | One-line description. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:981 | Questions | Questions | Keep the direct label. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:981 | Claude asks you to choose an answer. | Claude has a question for you. | Does not imply answers must be selected from a list. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:982 | Waiting for input | Idle sessions | Distinguishes idle reminders from questions and permission requests. Off by default. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:982 | The prompt has sat idle for a while. | Claude has been waiting for your next prompt. | No unverified timeout promise. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:983 | MCP input requests | MCP requests | Keep MCP for this technical audience. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:983 | An MCP server asks you for details. | An MCP server needs information from you. | One-line description. |

## desktop/crates/clawdmeter-app/src/ui/decorations.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/decorations.rs:21 | Wayland | Wayland | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/decorations.rs:62 | wayland: {} server-side decorations | wayland: {} server-side decorations | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/decorations.rs:62 | compositor offers | compositor offers | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/ui/mod.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:104 | the Clawdmeter window is already open; bringing it to the front | the Clawdmeter window is already open; bringing it to the front | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:109 | window lock unavailable ({e}); continuing | window lock unavailable ({e}); continuing | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:130 | bundled fonts not registered ({e}); using system fonts | bundled fonts not registered ({e}); using system fonts | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:141 | Clawdmeter | Clawdmeter | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:149 | couldn't open the window: {e} | couldn't open the window: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:224 | Clawdmeter | Clawdmeter | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:285 | {}: Clawdmeter isn't running | {}: Clawdmeter isn't running | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:289 | command {}: {e} | command {}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:295 | Clawdmeter isn't running, so there's no live status. Start it from your app menu, or run clawdmeter. | Start Clawdmeter from your app menu, or run clawdmeter in a terminal. | Replace copy; preserve format arguments. |

## desktop/crates/clawdmeter-app/src/ui/widgets.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/widgets.rs:293 | {id}-{i} | {id}-{i} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/widgets.rs:294 | {id}-{i} | {id}-{i} | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/ui/flasher.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:29 | Connect | Connect | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:29 | Identify | Identify | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:29 | Firmware | Firmware | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:29 | Flash | Flash | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:29 | Set up | Set up | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:47 | GitHub | GitHub | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:64 | Couldn't open {}. {} | Couldn't open {}. {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:67 | {}: {e} | {}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:68 | That isn't a Clawdmeter release manifest.json. | This file isn't a valid Clawdmeter release manifest. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:233 | Buy ↗ | Buy ↗ | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:363 | release check for {repo}: {e} ({}) | release check for {repo}: {e} ({}) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:367 | {repo} hasn't published any firmware yet. | No firmware release was found in {repo}. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:391 | Choose a release manifest.json | Choose a Clawdmeter manifest.json | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:446 | the board runs {}, which this release doesn't have | This release has no firmware for the detected board: {}. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:478 | This board has an {}{}. None of the boards in this release match it. | This board has an {}{}. None of the boards in this release match it. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:480 |  with {m} MB of flash |  with {m} MB of flash | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:516 | {} isn't next to the manifest.json. | Place {} in the same folder as manifest.json. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:518 | Couldn't read {}. {} | Couldn't read {}. {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:558 | Board on {port} | USB port: {port} | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:559 | {} ({:.1} MB), checksum verified | {} ({:.1} MB), checksum verified | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:565 | Restarting the board into its bootloader… | Restarting the board into its bootloader… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:571 | Writing {:.1} MB… | Writing {:.1} MB… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:574 | Checking what was written… | Verifying firmware… | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:575 | Restarting into the new firmware | Restarting the board… | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:595 | Done | Firmware installed | Use only after confirmed successful completion; step breadcrumb can be Done. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:604 | Stopped. | Installation stopped. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:678 | Checking GitHub for firmware… | Checking for firmware… | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:684 | {msg} If you have a release folder, open its manifest.json to flash from it. | {msg} If you have a release folder, open its manifest.json to flash from it. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:694 | Open manifest.json… | Open manifest.json… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:697 | Check again | Check again | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:713 | Plug in your Clawdmeter | Connect your board | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:716 | Use a USB-C data cable. Charge-only cables are the most common reason nothing shows up. | Connect the board with a USB data cable. A charge-only cable won't work. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:718 | Listening on USB | Waiting for USB | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:732 | Hide the boards | Hide supported boards | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:732 | Don't have one yet? See the boards it runs on | View supported boards | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:745 | Running Clawdmeter {} | Running Clawdmeter {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:747 | {} · {}{} | {} · {}{} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:748 | ESP32 | ESP32 | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:749 | {m} MB | {m} MB | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:749 | ? MB | ? MB | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:750 |  · PSRAM |  · PSRAM | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:759 | Which board is this? | Choose your board | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:762 | Asking the board… | Identifying board… | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:773 | Asking the board who it is. If it isn't running Clawdmeter yet, it restarts briefly into its bootloader. | Checking board details. A board without Clawdmeter firmware may restart into its bootloader. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:779 | It already runs the latest Clawdmeter firmware ({latest}), so there's nothing to flash. | Firmware is up to date ({latest}). Continue, or reinstall if needed. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:781 | It already runs Clawdmeter, so it told us exactly which board it is. | Board identified from its Clawdmeter firmware. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:782 | Only one board in this release fits this chip. | One board in this release matches the detected chip. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:783 | The chip narrows it to {n} boards. Pick the one on your desk. | {n} boards match this chip. Select the model on your desk. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:846 | Flash anyway | Reinstall firmware | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:852 | Continue | Continue | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:858 | Flash it again | Reinstall firmware | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:858 | Continue | Continue | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:879 | Back | Back | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:900 | – | – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:902 | Downloading… {:.0}% | Downloading… {:.0}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:903 | Checksum verified | Checksum verified | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:904 | Failed | Failed | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:905 | – | – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:908 | Getting the firmware | Downloading firmware | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:909 | Ready to flash | Ready to install | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:910 | Couldn't get the firmware | Couldn't download firmware | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:929 | Board | Board | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:931 | Firmware | Firmware | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:932 | {ver} · {} | {ver} · {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:935 | From | Source | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:936 | Image | Verification | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:937 | USB | USB | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:953 | Back | Back | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:960 | Flash {ver} | Install {ver} | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:968 | Flashing failed | Firmware installation failed | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:970 | Writing firmware | Writing firmware | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:972 | Done | Firmware installed | Use only after confirmed successful completion; step breadcrumb can be Done. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1000 | {:.0}% | {:.0}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1006 | Keep it plugged in. If it stops, unplug it, hold BOOT while you plug it back in, and retry. | Keep the USB cable connected until installation finishes. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1015 | Start over | Start over | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1021 | Retry | Retry | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1038 | Flashed. Now pair it. | Firmware installed | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1041 | It's restarting and will show up in your Bluetooth settings as Clawdmeter. Pair it once; after that the app finds it on its own. | The board is restarting. If it is not already paired, choose Clawdmeter in your Bluetooth settings. | Do not ask already-paired users to pair again. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1045 | Connected | Connected | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1047 | Waiting for it to connect | Waiting for Bluetooth connection | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1053 | Flash another | Flash another board | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1062 | Device settings | Device settings | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1088 | Flash | Flash | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:1095 | Start over | Start over | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/ui/setup.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:31 | Welcome | Welcome | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:31 | Your board | Your board | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:31 | Flash | Flash | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:31 | Pair | Pair | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:31 | Hooks | Hooks | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:31 | Done | Done | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:117 | couldn't record that setup is done: {e} | couldn't record that setup is done: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:192 | Meet Clawdmeter | Set up Clawdmeter | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:195 | A small screen for your desk that shows your Claude Code usage. Clawd lives on it, reacts while you work, and comes to find you when a session is waiting on you. | See your Claude Code usage on your desk. Clawd reacts to activity and alerts you when a session needs an answer. | Welcome purpose. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:200 | Setup takes a few minutes: put the firmware on your board, pair it over Bluetooth, and let Claude Code tell Clawd what's going on. | Connect your board with a USB data cable. Setup installs firmware, pairs Bluetooth, and optionally adds Claude Code hooks. Claude Code must be installed and signed in. | State prerequisites; no duration promise. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:207 | Open System Settings → Bluetooth and click Connect next to Clawdmeter. | Open System Settings → Bluetooth and click Connect next to Clawdmeter. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:209 | Open Settings → Bluetooth &amp; devices → Add device → Bluetooth, and choose Clawdmeter. | Open Settings → Bluetooth &amp; devices → Add device → Bluetooth, and choose Clawdmeter. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:211 | Open your desktop's Bluetooth settings and pair Clawdmeter. From a terminal: bluetoothctl, then scan on, pair &lt;address&gt; and trust &lt;address&gt;. | Open your desktop's Bluetooth settings and pair Clawdmeter. From a terminal: bluetoothctl, then scan on, pair &lt;address&gt; and trust &lt;address&gt;. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:215 | Connected | Connected | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:216 | Waiting for Clawdmeter to connect | Waiting for Bluetooth connection | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:220 | Clawdmeter's background part isn't running, so this step can't see the connection. Pair it anyway; the app finds it once it runs. | The background process isn't running. Pair the device, then start Clawdmeter to check the connection. | Do not imply setup verified a connection. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:232 | Pair it over Bluetooth | Pair over Bluetooth | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:235 | Your computer sees the device as a Bluetooth keyboard; that's also what makes its buttons work. Pair it once and the app finds it on its own after that. | Pair Clawdmeter in your computer's Bluetooth settings. It appears as a keyboard so its buttons can send key presses. | Explain keyboard identity without repeating connection status. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:240 | Paired with another computer before? Hold its PWR button for 3 seconds and let go to make it pairable again. | On boards with a software-controlled PWR button, hold PWR for 3 seconds, then release to clear pairing. | CONDITIONAL: omit for LCD-4. Its PWR key is reset, not unpair. See review. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:253 | Let Clawd follow along | Add Claude Code hooks | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:256 | Claude Code hooks tell Clawd when you send a prompt and when a session is waiting on you. They record only the event type and session ID. | Hooks let Clawd react to activity and show alerts. They store event metadata locally, plus notification or question text for alerts. | Same privacy model as Hooks page. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:262 | your account | your account | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:262 | all {installed} accounts | all {installed} accounts | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:265 | Already set up in {accounts} | Hooks installed in {accounts} | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:276 | Board flashed with the latest firmware | Latest firmware installed | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:277 | Board already on the latest firmware | Firmware up to date | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:278 | Board not flashed (the Flash page does it any time) | Firmware not installed in this setup. Use Flash to install it. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:281 | Connected over Bluetooth | Bluetooth connected | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:283 | Not connected yet (pair it whenever you're ready) | Not connected. Pair in Bluetooth settings. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:287 | Claude Code hooks set up | Hooks installed in {installed} of {total} accounts | LOGIC: existing condition only checks installed &gt; 0. Add total and format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:289 | No hooks yet (the Hooks page adds them) | Hooks not installed. Add them from Hooks. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:301 | You're all set | Setup summary | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:305 | Clawdmeter keeps running in the tray. Open this window from there to see your usage, update the firmware or change settings. | Clawdmeter keeps running in the tray. Open it there to view usage or settings. | CONDITIONAL: without tray use “Keep this window open to update usage.” |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:335 | Back | Back | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:361 | Get started | Start setup | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:370 | Skip this step | Skip this step | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:384 | Not now | Skip hooks | Make the skipped step explicit. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:384 | Next | Continue | CONDITIONAL: Pair while disconnected → Continue without pairing; otherwise Continue. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:397 | Finish | Open Clawdmeter | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/ui/setup.rs:422 | Skip setup | Skip setup | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/ui/app.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/app.rs:193 | Claude Code | Claude Code | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/app.rs:214 | Run setup again | Run setup again | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/app.rs:223 | Created by | Created by | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/ui/app.rs:253 | Version {} | Version {} | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/tray/mod.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:81 | Clawdmeter · Starting | Clawdmeter · Starting | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:82 | No data yet | No usage data yet | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:83 | Session  – | Session  – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:84 | Weekly  – | Weekly  – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:85 | Refresh now | Refresh usage | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:86 | Mute attention beep | Mute attention beep | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:87 | Open Clawdmeter | Open Clawdmeter | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:88 | Flash firmware… | Flash firmware… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:89 | Settings… | Settings… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:90 | Quit (stops the daemon) | Quit Clawdmeter | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:115 | Bluetooth off | Bluetooth unavailable | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:117 | Clawdmeter · {label} | Clawdmeter · {label} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:120 | {a} · updated {} | {a} · updated {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:122 | Run claude to sign in again | Open Claude Code to sign in | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:123 | No data yet | No usage data yet | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:126 | Session  {p}% | Session  {p}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:127 | Session  not started | Session  not started | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:128 | Session  – | Session  – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:131 | Weekly  {p}% | Weekly  {p}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:132 | Weekly  – | Weekly  – | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:137 | Update firmware to {}… | Update firmware to {}… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:138 | Flash firmware… | Flash firmware… | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:156 | Clawdmeter · {} | Clawdmeter · {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:158 |  · Session {s}% · Weekly {w}% |  · Session {s}% · Weekly {w}% | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:189 | couldn't switch the window to {}: {e} | couldn't switch the window to {}: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:196 | couldn't open the window: {e} | couldn't open the window: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:273 | window command: {} | window command: {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:310 | status file: {e} | status file: {e} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:330 | Clawdmeter | Clawdmeter | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:340 | tray icon unavailable ({e}); showing the window instead | tray icon unavailable ({e}); showing the window instead | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:376 | latest firmware release: {} | latest firmware release: {} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:383 | update check: {e} ({}) | update check: {e} ({}) | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:430 | Clawdmeter is already running; opening its window | Clawdmeter is already running; opening its window | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:436 | instance lock unavailable ({e}); continuing | instance lock unavailable ({e}); continuing | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:444 | === Clawdmeter {} === | === Clawdmeter {} === | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:449 | dropped {} command(s) from a previous run | dropped {} command(s) from a previous run | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/tray/mod.rs:455 | no tray (CLAWDMETER_NO_TRAY is set, or no StatusNotifierWatcher on the session bus: GNOME needs the AppIndicator extension); showing the window instead | no tray (CLAWDMETER_NO_TRAY is set, or no StatusNotifierWatcher on the session bus: GNOME needs the AppIndicator extension); showing the window instead | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/hooks/install.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/hooks/install.rs:187 | settings.json isn't valid JSON, so it was left alone. Fix it and try again. | settings.json contains invalid JSON. Fix the file, then retry. Nothing was written. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/hooks/install.rs:189 | settings.json has an unexpected layout ({w}), so it was left alone. | Unsupported settings.json layout: {w}. Nothing was written. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-app/src/hooks/install.rs:208 | the top level isn't an object | the top level isn't an object | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/hooks/install.rs:225 | \"hooks\" isn't an object | \"hooks\" isn't an object | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-app/src/hooks/install.rs:229 | a hook event isn't a list | a hook event isn't a list | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-app/src/hooks/mod.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/hooks/mod.rs:119 | {now_unix} {} {}\n | {now_unix} {} {}\n | Keep: concise label, technical output, or runtime format. |

## firmware/src/ble.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/ble.cpp:7 | Clawdmeter | Clawdmeter | Keep; preserve numeric formats and glyphs. |
| firmware/src/ble.cpp:324 | Anthropic | Clawdmeter | BLE manufacturer metadata should identify this project, not imply Anthropic made the hardware. |

## firmware/src/clawd_director.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/clawd_director.cpp:665 | Test: Clawd needs you | Clawdmeter test alert | Short device copy. Render at 240×240 before applying. |

## firmware/src/splash.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/splash.cpp:381 | no animations loaded\n\nrun tools/convert_official_clawd.js | Animations missing\nRebuild firmware | Short device copy. Render at 240×240 before applying. |

## firmware/src/ui.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/ui.cpp:268 | Accomplishing | Accomplishing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:268 | Elucidating | Elucidating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:268 | Perusing | Perusing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:269 | Actioning | Actioning | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:269 | Enchanting | Enchanting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:269 | Philosophising | Philosophising | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:270 | Actualizing | Actualizing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:270 | Envisioning | Envisioning | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:270 | Pondering | Pondering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:271 | Baking | Baking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:271 | Finagling | Finagling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:271 | Pontificating | Pontificating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:272 | Booping | Booping | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:272 | Flibbertigibbeting | Flibbertigibbeting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:272 | Processing | Processing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:273 | Brewing | Brewing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:273 | Forging | Forging | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:273 | Puttering | Puttering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:274 | Calculating | Calculating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:274 | Forming | Forming | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:274 | Puzzling | Puzzling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:275 | Cerebrating | Cerebrating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:275 | Frolicking | Frolicking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:275 | Reticulating | Reticulating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:276 | Channelling | Channelling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:276 | Generating | Generating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:276 | Ruminating | Ruminating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:277 | Churning | Churning | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:277 | Germinating | Germinating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:277 | Scheming | Scheming | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:278 | Clauding | Clauding | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:278 | Hatching | Hatching | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:278 | Schlepping | Schlepping | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:279 | Coalescing | Coalescing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:279 | Herding | Herding | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:279 | Shimmying | Shimmying | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:280 | Cogitating | Cogitating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:280 | Honking | Honking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:280 | Shucking | Shucking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:281 | Combobulating | Combobulating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:281 | Hustling | Hustling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:281 | Simmering | Simmering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:282 | Computing | Computing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:282 | Ideating | Ideating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:282 | Smooshing | Smooshing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:283 | Concocting | Concocting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:283 | Imagining | Imagining | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:283 | Spelunking | Spelunking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:284 | Conjuring | Conjuring | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:284 | Incubating | Incubating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:284 | Spinning | Spinning | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:285 | Considering | Considering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:285 | Inferring | Inferring | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:285 | Stewing | Stewing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:286 | Contemplating | Contemplating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:286 | Jiving | Jiving | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:286 | Sussing | Sussing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:287 | Cooking | Cooking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:287 | Manifesting | Manifesting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:287 | Synthesizing | Synthesizing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:288 | Crafting | Crafting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:288 | Marinating | Marinating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:288 | Thinking | Thinking | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:289 | Creating | Creating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:289 | Meandering | Meandering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:289 | Tinkering | Tinkering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:290 | Crunching | Crunching | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:290 | Moseying | Moseying | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:290 | Transmuting | Transmuting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:291 | Deciphering | Deciphering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:291 | Mulling | Mulling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:291 | Unfurling | Unfurling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:292 | Deliberating | Deliberating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:292 | Mustering | Mustering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:292 | Unravelling | Unravelling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:293 | Determining | Determining | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:293 | Musing | Musing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:293 | Vibing | Vibing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:294 | Discombobulating | Discombobulating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:294 | Noodling | Noodling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:294 | Wandering | Wandering | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:295 | Divining | Divining | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:295 | Percolating | Percolating | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:295 | Whirring | Whirring | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:296 | Doing | Doing | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:296 | Wibbling | Wibbling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:297 | Effecting | Effecting | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:297 | Wizarding | Wizarding | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:298 | Working | Working | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:298 | Wrangling | Wrangling | Keep: owner decision. Claude Code's thinking verbs rotate as the all-is-well indicator, not proof of an active model request. |
| firmware/src/ui.cpp:310 | --- | --- | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:312 | Resets in %dm | Resets in %dm | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:314 | Resets in %dh %dm | Resets in %dh %dm | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:316 | Resets in %dd %dh | Resets in %dd %dh | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:402 | ---% | -- | Short device copy. Render at 240×240 before applying. |
| firmware/src/ui.cpp:414 | --- | --- | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:435 | To pair | Pair again | CONDITIONAL: PWR-capable boards only. LCD-4 initial pairing lines: Pair via Bluetooth / Choose Clawdmeter / On your computer. No invented bond-reset gesture. |
| firmware/src/ui.cpp:441 | hold the power button | Hold PWR for 3s | CONDITIONAL: PWR-capable boards only. LCD-4 initial pairing lines: Pair via Bluetooth / Choose Clawdmeter / On your computer. No invented bond-reset gesture. |
| firmware/src/ui.cpp:447 | for 3 seconds, then release | Then release | CONDITIONAL: PWR-capable boards only. LCD-4 initial pairing lines: Pair via Bluetooth / Choose Clawdmeter / On your computer. No invented bond-reset gesture. |
| firmware/src/ui.cpp:465 | Usage | Usage | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:483 | Current | Session | Short device copy. Render at 240×240 before applying. |
| firmware/src/ui.cpp:489 | % | % | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:495 | of your monthly budget | of spending limit | Short device copy. Render at 240×240 before applying. |
| firmware/src/ui.cpp:508 | Weekly | Weekly | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:604 | Usage | Usage | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:612 | Spending | Spending | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:620 | Current | Session | Short device copy. Render at 240×240 before applying. |
| firmware/src/ui.cpp:631 | Under pace | Under pace | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:633 | 788c5d | 788c5d | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:635 | Over pace | Over pace | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:635 | c0392b | c0392b | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:637 | On pace | On pace | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:637 | d97757 | d97757 | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:641 | %d | %d | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:645 | %d%% | %d%% | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:655 | Period | Period | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:656 | %d%% | %d%% | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:662 | #%s %s# - #faf9f5 Resets %s# | #%s %s# - #faf9f5 Resets %s# | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:667 | %d%% | %d%% | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:723 | %d:%02d %s | %d:%02d %s | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:724 | AM | AM | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:724 | PM | PM | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:726 | %02d:%02d | %02d:%02d | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:736 | Waiting | Waiting | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:737 | Stale | Stale | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:737 | No data | No data | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:737 | Connected | Connected | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:741 | %s %s | %s %s | Keep; preserve numeric formats and glyphs. |
| firmware/src/ui.cpp:761 | Waiting | Waiting | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:763 | Stale | Stale | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:763 | No data | No data | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:763 | Listening | Listening | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:765 | Connected | Connected | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |
| firmware/src/ui.cpp:772 | %s %s\xE2\x80\xA6 | %s %s\xE2\x80\xA6 | Keep: owner decision. Preserve the existing connection/data state text, alternation, and status animation format. |

## firmware/src/boards/sim/sim_main.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/boards/sim/sim_main.cpp:12 | Clawdmeter simulator\n | Clawdmeter simulator\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:13 |   mouse          touch (on the meters he reacts; elsewhere: show me the meters)\n |   mouse          touch (on the meters he reacts; elsewhere: show me the meters)\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:14 |   space          play/pause scenario    left/right step    1-9 jump\n |   space          play/pause scenario    left/right step    1-9 jump\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:15 |   d              toggle BLE link\n |   d              toggle BLE link\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:16 |   b / n (hold)   BOOT / secondary button    p  PWR button\n |   b / n (hold)   BOOT / secondary button    p  PWR button\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:17 |   c              toggle charging            - / =  battery down/up\n |   c              toggle charging            - / =  battery down/up\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:18 |   u              toggle no battery (USB power only)\n |   u              toggle no battery (USB power only)\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:19 |   e / t          hook event: prompt / stop  a  cycle activity 0-3\n |   e / t          hook event: prompt / stop  a  cycle activity 0-3\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:20 |   m              attention alert / clear    z  toggle minimal mode\n |   m              attention alert / clear    z  toggle minimal mode\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:21 |   s              screenshot                 esc  quit\n\n |   s              screenshot                 esc  quit\n\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/sim_main.cpp:26 | [sim] SIM_FAST: compressed director timings\n | [sim] SIM_FAST: compressed director timings\n | Keep; preserve numeric formats and glyphs. |

## firmware/src/boards/sim/ble_sim.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/boards/sim/ble_sim.cpp:57 | state %d | state %d | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:76 | [sim] scenario: %s\n | [sim] scenario: %s\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:84 | [sim] no scenario file found — using built-in states\n | [sim] no scenario file found — using built-in states\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:91 | Clawdmeter sim — %s[%d/%d] %s %s | Clawdmeter sim — %s[%d/%d] %s %s | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:92 | (disconnected)  | (disconnected)  | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:116 | Clawdmeter (sim) | Clawdmeter (sim) | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:119 | [sim] pair gesture completed — bonds cleared\n | [sim] pair gesture completed — bonds cleared\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:138 | [sim] payload NACKed — check the scenario JSON\n | [sim] payload NACKed — check the scenario JSON\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:143 | [sim] HID press key=0x%02X mod=0x%02X\n | [sim] HID press key=0x%02X mod=0x%02X\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:145 | [sim] HID release\n | [sim] HID release\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:204 | [sim] inject hook event '%c' (es %lu)\n | [sim] inject hook event '%c' (es %lu)\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:211 | [sim] inject activity a=%d\n | [sim] inject activity a=%d\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:215 | Bash: pio run -e waveshare_amoled_216 | Bash: pio run -e waveshare_amoled_216 | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/ble_sim.cpp:221 | [sim] inject attention %s\n | [sim] inject attention %s\n | Keep; preserve numeric formats and glyphs. |

## firmware/src/boards/sim/display.cpp

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| firmware/src/boards/sim/display.cpp:33 |  %d |  %d | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:35 |  &#124; cols |  &#124; cols | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:39 |  %d |  %d | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:43 | [%lu] edge: %s [%s]\n | [%lu] edge: %s [%s]\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:60 | SDL_Init failed: %s\n | SDL_Init failed: %s\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:63 | Clawdmeter sim | Clawdmeter sim | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:68 | SDL window/renderer failed: %s\n | SDL window/renderer failed: %s\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:136 | screenshot: %s\n | screenshot: %s\n | Keep; preserve numeric formats and glyphs. |
| firmware/src/boards/sim/display.cpp:137 | screenshot failed: %s\n | screenshot failed: %s\n | Keep; preserve numeric formats and glyphs. |

## desktop/crates/clawdmeter-core/src/usage.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-core/src/usage.rs:199 | Trying again in about {m} minute{}. | Trying again in about {m} minute{}. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-core/src/usage.rs:201 | Retrying. | Retrying. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-core/src/usage.rs:204 | Couldn't run the claude command. Is Claude Code installed? | Couldn't run Claude Code. Check its installation and the claude_path setting. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-core/src/usage.rs:205 | Claude Code took too long to answer. {again} | Claude Code took too long to answer. {again} | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-core/src/usage.rs:206 | Claude Code answered with something unexpected. {again} | Couldn't read Claude Code's usage response. {again} | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-core/src/usage.rs:207 | Not logged in. Run claude in a terminal and log in. | Sign in to Claude Code for this account folder. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-core/src/usage.rs:208 | Not on a Claude subscription, so there are no limits to show. | This account doesn't report subscription usage limits. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-core/src/usage.rs:210 | Claude Code's usage service is rate-limiting this account. {again} | No usage data was returned. {again} | LOGIC: NoUsageLines uses this neutral message; confirmed RateLimited uses 'Usage requests are temporarily limited. {again}'. Split the shared branch. |

## desktop/crates/clawdmeter-flash/src/device.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-flash/src/device.rs:75 | No permission to open {port}. Add yourself to the dialout group (uucp on Arch): sudo usermod -aG dialout $USER, then log out and back in. | No permission to open {port}. Add yourself to the dialout group (uucp on Arch): sudo usermod -aG dialout $USER, then log out and back in. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:78 | No permission to open {port}. | Permission denied for {port}. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-flash/src/device.rs:80 | {port} is in use by another program, like a serial monitor. Close it and try again. | {port} is in use. Close the serial monitor or other app using it, then retry. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-flash/src/device.rs:82 | The board disconnected. Plug it back in and try again. | The board disconnected. Reconnect USB and retry. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-flash/src/device.rs:84 | The board didn't respond. Unplug it, hold BOOT while you plug it back in, then try again. | The board didn't respond. Unplug it, hold BOOT while you plug it back in, then try again. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:88 | This board has an {}, but that firmware is for an {}. Go back and pick your board again. | This board has an {}, but that firmware is for an {}. Go back and pick your board again. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:92 | Couldn't talk to the board ({e}). | Couldn't talk to the board ({e}). | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:93 | Flashing stopped ({e}). Unplug the board, plug it back in and retry. | Flashing stopped ({e}). Unplug the board, plug it back in and retry. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:94 | {p} isn't a USB serial port. | {p} isn't a USB serial port. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:97 | The firmware was written, but the board didn't answer after restarting. Unplug it and plug it back in; if it still doesn't come up, flash it again. | The firmware was written, but the board didn't answer after restarting. Unplug it and plug it back in; if it still doesn't come up, flash it again. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/device.rs:102 | The board restarted, but it reports {} firmware instead of {expected}. Flash it again. | The board restarted, but it reports {} firmware instead of {expected}. Flash it again. | Keep: concise label, technical output, or runtime format. |

## desktop/crates/clawdmeter-flash/src/release.rs

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-flash/src/release.rs:76 | No firmware has been published yet. | No firmware release was found. | A missing or inaccessible repository can also return 404. |
| desktop/crates/clawdmeter-flash/src/release.rs:77 | Couldn't reach GitHub. Check your internet connection and try again. | Couldn't reach GitHub. Check your internet connection and try again. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/release.rs:79 | GitHub is limiting requests from this network right now. Try again in a few minutes. | GitHub is limiting requests from this network. Try again later. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-flash/src/release.rs:81 | GitHub didn't answer properly. Try again in a minute. | Couldn't read the release response from GitHub. Try again later. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-flash/src/release.rs:82 | This release's manifest.json is damaged or from a newer app. | Couldn't read this release's manifest.json. Check that the app supports this release. | Replace copy; preserve format arguments. |
| desktop/crates/clawdmeter-flash/src/release.rs:83 | This release is missing {n}. | This release is missing {n}. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/release.rs:86 | {file} doesn't match its checksum, so it wasn't used. Try again; if it keeps happening, the release is broken. | Checksum mismatch for {file}. Download it again before flashing. | A repeated mismatch does not prove the release is broken. Keep verified-download requirement. |
| desktop/crates/clawdmeter-flash/src/release.rs:89 | No permission to read or write that file. | No permission to read or write that file. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/release.rs:90 | File not found. | File not found. | Keep: concise label, technical output, or runtime format. |
| desktop/crates/clawdmeter-flash/src/release.rs:91 | {e} | {e} | Keep: concise label, technical output, or runtime format. |

## Small labels and accessibility additions

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/app.rs:232 | @HermannBjorgvin | @HermannBjorgvin | Keep the creator handle and its existing link; joins Created by. |
| desktop/crates/clawdmeter-app/src/ui/app.rs:183 | 1 | 1 | Keep action badge; use sparingly as described in the review. |
| desktop/crates/clawdmeter-app/src/ui/flasher.rs:56 | v{v} | v{v} | Keep version prefix. |
| desktop/crates/clawdmeter-app/src/ui/overview.rs:258 | Refresh icon without text | Refresh usage | Add accessible name and tooltip if supported by the existing GPUI widget API. Do not invent a custom tooltip hack. |
| desktop/crates/clawdmeter-app/src/ui/mod.rs:252 | Close icon without text | Close window | Accessible name; preserve the existing close behavior. |
| desktop/crates/clawdmeter-app/src/lib.rs:113 | ~{rest} | ~{rest} | Keep home-directory abbreviation for displayed paths. |

## Runtime content and coverage boundaries

| File:line | Current text | New text | Note |
| --- | --- | --- | --- |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:692 | HookStatus error text | Preserve the diagnostic; apply the hooks/install.rs rows above. | Dynamic error, not an additional fixed string. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:603 | AccountError.message | Use the core usage error rows above. | Keep retry timing and account folder context. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:284 | DeviceError rendered through Display | Use the flash device error rows above. | Details should be available without replacing the short primary message. |
| desktop/crates/clawdmeter-app/src/ui/settings.rs:797 | File contents in the hook diff | Keep file contents verbatim. | Do not copyedit JSON keys or user-authored hooks. |
| desktop/crates/clawdmeter-app/src/logger.rs:23 | error: / warning: | Keep prefixes. | Log severity markers, with timestamp and dynamic message. |

Internal IDs, IPC command/state tokens, font names, include paths, Bluetooth UUIDs,
JSON keys, tests, and character-animation identifiers are not display copy.
The fixed text in app/firmware display paths is listed above; board product
names remain exact identifiers. Firmware board-driver serial diagnostics are
engineering output outside the on-screen deck. Keep their register addresses,
chip names, and error codes. Copy changes must not rename the Bluetooth device,
hook commands, or CLI options.
