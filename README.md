# Clawdmeter

> Also check out [Beam](https://github.com/notaharness/beam)! A CLI that lets you pair your machines using a passkey and [@Tailscale's tailcat](https://tailscale.com/blog/tailcat).

<img src="assets/readme/waving.gif" width="120" align="right" alt="">

A small ESP32 dashboard I made for my desk to keep an eye on Claude Code usage.

It runs on a [Waveshare ESP32-S3-Touch-AMOLED-2.16](https://www.waveshare.com/esp32-s3-touch-amoled-2.16.htm?&aff_id=149786) and a few other boards, and pairs over Bluetooth with a small desktop app. The screen shows your session and weekly usage, and Clawd lives in the corner: he notices when you send a prompt, puts on a show when you've been busy for a while, dozes off as a face when nothing is happening, and walks on to wave at you when a Claude Code session is waiting for an answer. The side buttons send Space and Shift+Tab over BLE HID for Claude Code's voice mode and mode toggle.

<img width="1179" height="994" alt="Usage meter" src="https://github.com/user-attachments/assets/83e54aea-0932-428f-94aa-b3ede3a360aa" />

## What it does

|                     Meters                      |                      Show                       |                       Screensaver                       |                       Attention                       |
| :---------------------------------------------: | :---------------------------------------------: | :-----------------------------------------------------: | :---------------------------------------------------: |
| ![Meters](screenshots/director/01-home-meters.png) | ![Show](screenshots/director/09-show-scene.png) | ![Screensaver](screenshots/director/15-screensaver-face.png) | ![Attention](screenshots/director/20-attention-waves.png) |
| Session and weekly usage, Clawd in his seat | After five busy minutes | After ten quiet ones | A session needs you |

- **The device boots to the meters** and always comes back to them. A tap anywhere means "show me the meters".
- **Clawd reacts to what you do.** With the hooks installed (one click in the app), he points when you send a prompt, waves after a long turn, and plays one of Anthropic's official Clawd animations when you've been working hard. His mood follows how fast your session usage moved over the last 15 minutes.
- **Attention alerts.** When a session is waiting on a permission prompt or a question, a full-size Clawd walks on, waves and points, and the message scrolls on the status line. He leaves when you answer, when you tap the screen, or after 2 minutes. Turn on the beep in the app if you want a sound too.
- **Reset chime.** Optionally, a chime when your 5-hour session limit resets.
- **Stale data looks stale.** If the numbers stop arriving, the meters dim and the status line says so.

How the rooms and beats work is written up in [`docs/clawd-director.md`](docs/clawd-director.md).

## Hardware

Boards supported out of the box:

- [Waveshare ESP32-S3-Touch-AMOLED-2.16](https://www.waveshare.com/esp32-s3-touch-amoled-2.16.htm?&aff_id=149786)
- [Waveshare ESP32-C6-Touch-AMOLED-2.16](https://www.waveshare.com/esp32-c6-touch-amoled-2.16.htm?&aff_id=149786)
- [Waveshare ESP32-S3-Touch-AMOLED-1.8](https://www.waveshare.com/esp32-s3-touch-amoled-1.8.htm?&aff_id=149786)
- [Waveshare ESP32-C6-Touch-AMOLED-1.8](https://www.waveshare.com/esp32-c6-touch-amoled-1.8.htm?&aff_id=149786)
- [Waveshare ESP32-S3-Touch-AMOLED-2.06](https://www.waveshare.com/esp32-s3-touch-amoled-2.06.htm?&aff_id=149786)
- [Waveshare ESP32-S3-Touch-LCD-1.54](https://www.waveshare.com/esp32-s3-lcd-1.54.htm?sku=33869&aff_id=149786)
- [Waveshare ESP32-S3-Touch-LCD-4](https://www.waveshare.com/esp32-s3-touch-lcd-4.htm)

> Please check if a pull request exists for your alternative hardware port before opening a new one, providing QA feedback and testing on the same hardware is more valuable than duplicate pull requests.

**Porting to another board:** the firmware is a thin HAL with per-board folders under `firmware/src/boards/`. Drop in a new folder and a new PlatformIO env; the shared code never needs to change. See [`docs/porting/adding-a-board.md`](docs/porting/adding-a-board.md) for the walk-through and [`docs/porting/hal-contract.md`](docs/porting/hal-contract.md) for the interfaces a port must implement.

## Getting started

You need Claude Code installed and signed in to a subscription (Pro, Max, Team or Enterprise). The app reads your usage with `claude -p /usage`, which answers locally and costs nothing.

### 1. Install the desktop app

Download the archive for your OS from the [latest release](https://github.com/HermannBjorgvin/Clawdmeter/releases/latest):

- **Linux** (`clawdmeter-<version>-linux-x86_64.tar.gz`): unpack and put `clawdmeter` somewhere on your `PATH`, e.g. `~/.local/bin`. It needs BlueZ and a D-Bus session; the tray icon needs a StatusNotifierItem host (KDE, most Wayland/X11 panels; on GNOME the AppIndicator extension). Without one, the app shows a small status window instead.
- **macOS** (`clawdmeter-<version>-macos-universal.tar.gz`): unpack and move `Clawdmeter.app` to `/Applications`. The app isn't notarized, so the first time right-click it and choose **Open**. macOS then asks for Bluetooth access; allow it.
- **Windows** (`clawdmeter-<version>-windows-x86_64.zip`): unpack `clawdmeter.exe` somewhere permanent, e.g. `%LOCALAPPDATA%\Programs\Clawdmeter`, and run it. SmartScreen may warn about an unknown publisher the first time.

Run it with no arguments. It lives in the tray (menu bar on macOS): the menu shows the connection, the account and both meters, plus **Refresh now**, **Mute attention beep**, **Update firmware…**, **Flash a new device…**, **Settings…** and **Quit** (which stops the daemon; the device keeps showing its last numbers until they go stale).

### 2. Flash the board

Plug the board in over USB-C and choose **Flash a new device…** from the tray. The flasher asks the chip what it is, offers the matching board image from the latest release, and writes it. **Update firmware…** does the same for a board that's already running Clawdmeter. On Linux your user needs access to the serial port (the flasher tells you which group to join if it doesn't).

**Browser fallback:** each release also has one merged image per board, `<env>-<version>.bin`. In Chrome or Edge, open [Espressif's web flasher](https://espressif.github.io/esptool-js/), connect to the board, add that file at address `0x0`, and click **Program**.

### 3. Pair it

The device is a Bluetooth keyboard as far as your computer is concerned, so pair it once like any keyboard. That also makes the buttons work.

- **Linux:** `bluetoothctl`, then `scan on`, `pair <MAC>` and `trust <MAC>` when "Clawdmeter" shows up (or use your desktop's Bluetooth settings).
- **macOS:** System Settings → Bluetooth → **Connect** next to "Clawdmeter".
- **Windows:** Settings → Bluetooth & devices → Add device → Bluetooth → "Clawdmeter".

The app only ever talks to a Clawdmeter your computer is paired with, never to one that happens to be nearby. To pair with a different computer, hold the PWR button for 3 seconds and release: the device forgets its bond and advertises again.

### 4. Install the hooks

Open **Settings… → Hooks** and click **Install in N accounts**. You see the exact change to each account's `settings.json` and confirm it with **Save changes**; the old file is kept as `settings.json.bak`. There's also a **Send a test alert** button. The hooks run `clawdmeter hook …`, which appends one line to a log and exits; nothing from your prompts or tool calls is recorded. Without hooks the meters still work, but Clawd can't react to you and attention alerts don't fire.

### 5. Start at login

**Settings… → Background → Start at login** adds a login item: an XDG autostart entry on Linux, a LaunchAgent on macOS, an `HKCU\…\Run` value on Windows.

## Settings

Everything in the Settings window is stored in a plain config file, `~/.config/claude-usage-monitor/config` (`%LOCALAPPDATA%\Clawdmeter\config` on Windows). You can edit it by hand; the app re-reads it within a few seconds and keeps your comments.

| Key | Default | What it does |
| --- | --- | --- |
| `config_dirs` | `~/.claude` | Claude Code config folders to poll, comma-separated. With several (say a personal and a work plan), the device shows the one whose usage moved most recently. |
| `chime` | `off` | `on` = chime when the 5-hour session resets (boards with a speaker: S3 AMOLED-2.16 and 1.8, LCD-1.54). |
| `beep` | `off` | `on` = beep with attention alerts. |
| `clock` | `off` | `auto`, `12` or `24` shows a clock in place of the "Usage" title. |
| `poll_interval` | unset | Seconds between `/usage` polls. Unset, polling follows your activity: right after a prompt, every minute while Claude works, every 15 minutes when it's quiet. |
| `heartbeat_interval` | `60` | Seconds between keep-alive writes, so the device knows the numbers are current. |
| `claude_path` | found on `PATH` | Full path to `claude`, if the app can't find it. |
| `update_check` | `on` | Daily check for new firmware. |
| `release_repo` | `HermannBjorgvin/Clawdmeter` | Where the flasher looks for releases. |

Logs are in `~/.config/claude-usage-monitor/daemon.log`; **Settings… → Background** has a button to open it.

### Command line

```text
clawdmeter                      tray app (the normal way to run it)
clawdmeter --daemon             headless: no tray, no windows (servers)
clawdmeter --once               poll /usage once and print the payload, no Bluetooth
clawdmeter --window settings    open one window (settings, flasher or status)
clawdmeter --ports              list attached ESP32 USB ports
clawdmeter --info PORT          ask the firmware on PORT for its board and version
clawdmeter hook MODE [event]    the Claude Code hook (alert, clear or event)
```

`CLAWDMETER_LOG=debug` turns up logging, `CLAWDMETER_NO_TRAY=1` shows the status window instead of a tray icon, and `CLAWDMETER_BLE_ADDRESS=AA:BB:…` tells the app which paired device to try first.

## Upgrading from the old daemons

Earlier versions ran a bash daemon (Linux), a Python daemon (macOS) or a Python tray (Windows), set up with `install.sh`, `install-mac.sh` or `install-windows.ps1`. The app replaces all three and keeps your config file, your paired device and the hook state files.

1. Install the app as above. If an old daemon is still set to start at login, the app logs a warning, and **Settings… → Background** shows a **Turn it off** button. It stops the old service and removes its login entry:
   - Linux: `systemctl --user disable --now claude-usage-daemon` and deletes `~/.config/systemd/user/claude-usage-daemon.service`.
   - macOS: `launchctl unload -w` and deletes `~/Library/LaunchAgents/com.user.claude-usage-daemon.plist`.
   - Windows: deletes the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Clawdmeter` value if it still points at `pythonw.exe`. Quit the old tray icon yourself (or log out), since it may still be running.

   Running both at once makes them fight over the device.
2. **Settings… → Hooks** shows hooks that still run `attention-hook.sh` as out of date. **Update N accounts** rewrites them to `clawdmeter hook …` in place, after the same preview.
3. Delete your old checkout's `daemon/.venv` if you had one. Nothing else is left behind.

## How it works

<img src="assets/readme/magnifier.gif" width="150" align="right" alt="">

1. **The slow clock.** The app runs `claude --output-format json -p /usage` for each account. Claude Code answers from its own cache of your limits without sending a message, so polling can't start a window or spend anything. The app parses the session and weekly percentages and reset times (or the spend limit, for spend-capped accounts).
2. **The fast clock.** The Claude Code hooks append `<time> <event> <session>` to `~/.config/claude-usage-monitor/events.log`. The app follows that file and turns it into an activity level, the latest event and the number of active sessions. A prompt also triggers a fresh `/usage` poll.
3. **Attention.** On a Notification or AskUserQuestion, the hook writes the message to an `attention` file; the next prompt or tool call in that session clears it.
4. **The link.** The app finds the paired device over Bluetooth LE and writes one JSON payload to its data characteristic whenever something changes, plus a heartbeat every minute.
5. **The device.** The firmware parses the payload, updates the meters, and hands the signals to Clawd's director, which decides what he does next.
6. **The buttons** are independent of all this: they're HID keyboard reports to whatever window has focus.

## Physical buttons

| Button | Function |
| --- | --- |
| **BOOT** (GPIO 0 on S3 boards, GPIO 9 on C6) | Hold to send Space (Claude Code voice-mode push-to-talk). |
| **Secondary** (GPIO 18 on the S3 2.16, GPIO 10 on the C6 2.16, GPIO 5 on the LCD-1.54; the other boards have none) | Send Shift+Tab (Claude Code mode toggle). |
| **PWR** | On the stage: next scene. Elsewhere: cycle brightness. Hold 3 s and release: pairing mode. |

Every press also wakes Clawd from the screensaver. The LCD-4 has only BOOT; its PWR key is a hardware reset.

## BLE protocol

The device advertises a custom GATT service next to the standard HID keyboard service:

|                                | UUID                                   |
| ------------------------------ | -------------------------------------- |
| **Data service**               | `4c41555a-4465-7669-6365-000000000001` |
| RX characteristic (write)      | `4c41555a-4465-7669-6365-000000000002` |
| TX characteristic (notify)     | `4c41555a-4465-7669-6365-000000000003` |
| REQ characteristic (notify)    | `4c41555a-4465-7669-6365-000000000004` |
| **HID service**                | `00001812-0000-1000-8000-00805f9b34fb` |

The host writes one JSON object (at most 512 bytes) to RX; every write is a complete update. The device notifies `0x01` on REQ when it has no data yet, asking for a poll.

```json
{"s":45,"sr":120,"w":28,"wr":7200,"st":"allowed","acct":"pro","ok":true,"a":2,"e":"p","es":14,"n":1}
```

| Field | Meaning |
| --- | --- |
| `s`, `w` | Session and weekly % used. |
| `sr`, `wr` | Minutes until each resets; `-1` = window not started / unknown. |
| `st` | `allowed`, `limited` or `overage` (stored, not shown). |
| `acct` | `pro`, or `ent` for a spend-capped account: `s` is then the spend %, and `tp` (% of the billing period elapsed), `pd` (period length in days) and `rd` (reset date, e.g. `"Nov 1"`) fill the second card. |
| `t`, `tf` | Local wall-clock epoch and 12/24-hour format, when the clock is on. |
| `c` | `1` = the reset chime may sound. |
| `ok` | `false` alone = no data (every account logged out). |
| `b`, `m`, `x` | Attention: beep, message (ASCII), end the alert. |
| `a`, `e`, `es`, `n` | Fast clock: activity 0–3, latest hook event (`p` prompt, `s` stop, `b` back after a gap) and its sequence number, active sessions. |
| `rp` | `1` = a heartbeat replay, not a fresh poll; the rate math skips it. |

## Building from source

**Firmware** ([PlatformIO](https://docs.platformio.org/en/latest/core/installation/index.html)):

```bash
pio run -d firmware -e waveshare_amoled_216                                 # build one board
pio run -d firmware -e waveshare_amoled_216 -t upload --upload-port /dev/ttyACM0
pio run -d firmware                                                         # every board + the simulator
pio test -d firmware -e native                                              # host unit tests
```

The board envs are listed in `firmware/platformio.ini`. On macOS the port is `/dev/cu.usbmodem*`, on Windows `COMn`.

**Desktop app** (Rust stable, 1.90 or newer):

```bash
cd desktop
cargo run -p clawdmeter-app --release            # the tray app
cargo test --workspace
```

On Debian/Ubuntu install the build dependencies first:

```bash
sudo apt install pkg-config clang libclang-dev libdbus-1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev libvulkan-dev libfontconfig1-dev libfreetype-dev
```

The workspace has four crates: `clawdmeter-core` (config, `/usage` parsing, the hook files, cadence, payload; no I/O beyond files and the `claude` subprocess), `clawdmeter-ble` (the Bluetooth link), `clawdmeter-flash` (release download and flashing) and `clawdmeter-app` (the tray, the GPUI windows, the hook subcommand and installer, and the `clawdmeter` binary). Screenshots of the windows are in [`desktop/docs/screenshots/`](desktop/docs/screenshots/).

## Development

<img src="assets/readme/crab.gif" width="120" align="right" alt="">

- **Desktop simulator:** iterate on the UI without hardware. An SDL2 window runs the full firmware loop with scenario playback (`pio run -d firmware -e sim`, then `cd firmware && .pio/build/sim/program`). `SIM_FAST=1 SIM_SCENARIO=sim/director.jsonl` walks through every Clawd beat in two minutes. See [`SIM-USAGE.md`](SIM-USAGE.md).
- **Clawd's behaviour:** [`docs/clawd-director.md`](docs/clawd-director.md).
- **Splash animations:** Anthropic's official Clawd sprites, archived with provenance notes in [`research/clawd-official/`](research/clawd-official/); `node tools/convert_official_clawd.js` regenerates `firmware/src/splash_animations.h`. See [`tools/README.md`](tools/README.md).
- **Icons:** Lucide PNGs convert to LVGL C arrays with `tools/png_to_lvgl.js`. See [`tools/README.md`](tools/README.md).
- **Fonts:** the pre-compiled LVGL fonts and the LVGL 9 patching they need: [`docs/fonts.md`](docs/fonts.md).
- **Porting:** [`docs/porting/adding-a-board.md`](docs/porting/adding-a-board.md) and [`docs/porting/hal-contract.md`](docs/porting/hal-contract.md).
- **History:** [`docs/daemon-parity.md`](docs/daemon-parity.md) maps what the old bash and Python daemons did to where the app does it.

## Releasing

Push a version tag; CI does the rest.

```bash
git tag v1.9.0 && git push origin v1.9.0
```

[`.github/workflows/release.yml`](.github/workflows/release.yml) then:

1. Builds every hardware env in `firmware/platformio.ini` (everything except `platform = native`) with `-DCLAWD_VERSION="v1.9.0"`.
2. Merges bootloader, partition table, `boot_app0` and app into one image per board, `<env>-v1.9.0.bin`, flashed at offset `0x0` ([`tools/ci/merge_firmware.py`](tools/ci/merge_firmware.py)).
3. Builds the desktop app with `CLAWDMETER_VERSION=1.9.0` for Linux x86_64, macOS (a universal, ad-hoc signed `Clawdmeter.app`) and Windows.
4. Publishes a GitHub Release with the images, the app archives, `SHA256SUMS`, a `manifest.json` (env, chip, flash size, display name, sha256 per board; [`tools/ci/make_manifest.py`](tools/ci/make_manifest.py)) that the app's flasher reads, and one ESP Web Tools manifest per board, `webtools-<env>.json`.

A tag with a hyphen (`v1.9.0-rc.1`) publishes a prerelease, which the app's update check ignores. Adding a board needs a `DISPLAY_NAMES` entry in `make_manifest.py`; the tooling tests in `tools/ci/tests/` fail without one. The tag must look like `vMAJOR.MINOR.PATCH` with an optional `-suffix`, or the release run stops before building anything.

Every PR that touches `firmware/` or `tools/ci/` also builds all boards and uploads the merged images as workflow artifacts, runs the host unit tests and captures a simulator screenshot. PRs that touch `desktop/` run fmt, clippy and the workspace tests on Linux, and the core tests on macOS and Windows.

## Credits

- Pixel-art Clawd animations are Anthropic's official mascot art (claude.ai/code, Claude Code desktop), archived and converted by the tooling in `tools/` and `research/clawd-official/`.
- The original macOS daemon and flash helper were ported by [Chris Davidson (@lorddavidson)](https://github.com/lorddavidson). Thanks Chris!
- Lucide icon set ([lucide.dev](https://lucide.dev), MIT) for the Bluetooth, battery and USB glyphs.
- Anthropic brand fonts (Tiempos Text, Styrene B); see the licensing warning below.

## Licensing gray area warning

The software in this repository uses and adheres to the Anthropic brand guidelines and uses the same proprietary fonts that Anthropic has a license for but this software uses without permission as well as using assets from Anthropic such as the copyrighted Clawd mascot so even though the code in this repo is non-proprietary I will not license it myself under a copyleft license since this repo includes proprietary fonts and copyrighted assets. Please be aware of this if you fork or copy the code from this repo. **You have been warned!**
