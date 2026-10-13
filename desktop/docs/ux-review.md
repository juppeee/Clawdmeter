# Clawdmeter UX review

Reviewed the supplied `ux-shots` set: Overview (running and stopped), Device,
Flash (connect, identify, firmware, progress), Accounts, Hooks and preview,
Settings (named Background in those captures), all setup steps, device meters,
Clawd motion sheets, and the owner's annotated `paste-*.png` crops. Findings
are checked against app and firmware sources. These are recommendations;
this document does not imply that the implementation or refreshed screenshots
already include them.

The owner has already assigned the Settings rename, refresh icon, removal of
Overview mute, sidebar redesign, creator link, and new config-directory location.
Those changes are assumed here. Apply the accompanying [copy deck](copy-deck.md)
without redoing the other player's implementation work.

## Priorities

| Priority | Recommendation | Reason |
| --- | --- | --- |
| 1 | Make connection, reading age, and retry state distinct. | Bluetooth can be connected while usage is old. “Refresh now” cannot bypass retry delays. |
| 2 | Simplify Settings: CLI path under Advanced; heartbeat interval config-only. | Neither control helps the normal setup path. A heartbeat does not fetch fresh usage. |
| 3 | Make setup report verified outcomes and respect the selected board. | Skipped steps are not success; LCD-4 has no software-controlled PWR button. |
| 4 | Explain hook contents accurately before installation. | The event log has metadata, but attention alerts also store notification or question text. |
| 5 | Give flashing one clear action per stage, with recovery instructions only after failure. | Current progress copy invites unplugging while a write may still be running. |

## Navigation and window

Keep the six-page sidebar and a single window. Overview is the place to read
usage; Device owns display and sound; Flash owns USB installation; Accounts
owns usage sources; Hooks owns integration; Settings owns app behavior.
Use the owner's **Created by @HermannBjorgvin** footer link. Keep version and
setup restart secondary so the footer does not compete with navigation.

The screenshots use small body text and dense descriptions in fixed-width
rows. Let descriptions wrap below their labels and keep toggles aligned in
a stable control column. Avoid shrinking text to fit. At the minimum window
size, scroll page content while keeping the sidebar and wizard actions visible.
Show a short explanation when navigation is disabled during a firmware write.

Badges should mean an action is available, not count routine conditions.
An optional hook installation need not look like a persistent error. Preserve
text labels alongside color for connection, warning, and failure states.

## Overview

**Evidence:** `overview.png`, `overview-not-running.png`, refresh and mute crops.

The main hierarchy is sound: connection, account/age, then meters. The update
banner and two full settings rows consume more space than the usage itself.
The owner is already replacing refresh with an icon and removing mute.

- Keep refresh beside the reading age, with an accessible name **Refresh usage**.
  A tooltip can say **Checks when the retry delay allows.** Show request feedback;
  if waiting, expose the retry time instead of appearing unresponsive.
- Use **Updated {age}** for the last successful usage fetch, not the last
  heartbeat. Keep a connected device's stale data visible but clearly marked.
- Separate missing Bluetooth hardware from a disabled adapter. `no_adapter`
  supports **Bluetooth unavailable**, not the more specific **Bluetooth is off**.
- Collapse the stopped-state header and notice into one explanation with the
  launch command or an app-launch action. The current state repeats itself.
- Keep an available firmware update secondary to usage. Avoid “about a minute”
  unless a measured estimate supports it for all targets and download speeds.
- Match device labels: **Session**, **Weekly**. For spend-only accounts, use
  **Spending** and the billing period rather than calling spend a session.
  This needs account-kind data in the desktop snapshot, not just a text edit.

## Device

**Evidence:** `device.png` and the board-card crop.

The full Waveshare product name overwhelms the card. Lead with the short
model name and SoC where needed to distinguish C6; keep the full product name
in details. The illustration helps identify the board and should stay.

Separate **Bluetooth connected** from **USB not connected**. The latter is
normal during daily use, not a warning. When board/version data came from
cache, label it **Last detected** so a swapped board is not misidentified.
Keep a single update action beside version information.

**Minimal display** needs three explicit choices: **Board default**, **On**,
**Off**. “Auto” implies adaptive behavior that does not exist. Explain that
it shows meters and a still Clawd, and label the USB requirement next to the
disabled control. A missing response alone does not prove old firmware;
use **Couldn't read display mode. Reconnect USB or update the firmware.**

Group Clock and Minimal display under display settings; group the two sound
controls together. Explain that sound depends on firmware support for the
board, not merely a fitted speaker. Tray mute lasts until restart. The chime
is triggered by an observed reset, not guaranteed to sound at the exact
five-hour boundary while disconnected.

Rename **Forget this device** to **Clear saved device**. It clears the app's
address cache and searches for a paired device; it does not remove OS pairing
or the board's bond. Put it below the routine controls with that explanation.

## Flash

**Evidence:** `flash-connect.png`, `flash-identify.png`, `flash-firmware.png`,
`flash-progress.png`, and the equivalent wizard captures.

Connect: lead with **Connect your board** and **Use a USB data cable.**
Keep the board shop collapsed. “Listening on USB” describes an implementation;
**Waiting for USB** describes the state. Avoid the unsupported claim that
charge-only cables are the most common failure.

Identify: keep board art and require an explicit choice when the chip is
ambiguous. Give S3 and C6 visible identities. A detected chip narrows the
choices; it does not identify the panel. If firmware already identified the
board, use a confirmation heading instead of asking the same question.
When current, make Continue primary and reinstall secondary.

Firmware: show model, version, and checksum result before the primary
**Install {version}** action. Put image filename, source folder/repository,
and port under details. USB paths are useful for diagnosis but need not
share the hierarchy of board identity.

Progress: show the write percentage and current phase. Default helper text
should only say **Keep the USB cable connected until installation finishes.**
Show BOOT/reconnect advice after a failed operation and when the port is released.
Keep the detailed log available, preferably in a disclosure. Do not call an
active-but-slow write “stopped” based on a timer alone.

Done: use **Firmware installed**. Show **Bluetooth connected** if it already
is; otherwise offer pairing instructions. A successful update does not
require the owner to pair again. Separate **Flash another board** from
**Device settings**.

## Accounts

**Evidence:** `accounts.png`.

The first paragraph assumes users know `CLAUDE_CONFIG_DIR`. Start with the
normal case: **Your default Claude Code account is in ~/.claude. Add another
config folder if you use a separate work or personal account.** Put the
variable and advanced launch examples in help or docs.

Use **Add account folder…** so the picker’s purpose is explicit. The selected
account follows increases in session usage, not every interaction. Explain
that selection precisely in secondary text rather than saying “whichever
you used most recently.” A directory alone does not prove an account is signed in.

Keep **On device** once per row; cut the duplicate “Shown on the device.”
For retrying accounts, retain the last reading and state the retry time. Keep
sign-in problems separate from temporary failures. The final account cannot
be removed; explain the disabled action. Removing a folder stops monitoring
it; it does not delete the folder, sign out, or remove installed hooks.

## Hooks and change preview

**Evidence:** `hooks.png`, `hooks-preview.png`.

The paragraph mixes purpose, privacy, implementation, and cadence. Split it
into a short purpose sentence and a concrete data note. The current “only
event type and session ID” claim omits timestamps and alert/question text.
The “every 2 minutes” no-hooks claim disagrees with the scheduler's five-minute
default. Do not embed cadence in this page; say the meters work without hooks.

Use **Review installation** before a preview and **Install hooks** to apply.
For removal, use **Review removal** then **Remove hooks**. Avoid identical
Install actions at both the row and batch level when only one account exists.
The preview should identify files and explain backups, including that a new
file has no previous version to back up. Preserve the reformatting notice.

Keep the JSON diff available for technical users, but precede it with the
account count and purpose. If multiple files are written and a later one
fails, report which were saved and which failed; the operation is not atomic.
This requires state in the implementation, not a copy-only replacement.

**Send test alert** verifies the app/device path, not hook installation.
Report **Test alert queued. Check the device.** until delivery is known.
Disable it when the background process is stopped, or explicitly state that
there is no process to deliver it. Do not promise a large Clawd on C6 or
minimal-display boards.

## Settings

**Evidence:** `background.png`, CLI and keep-alive crops.

Lead with Start at login. Keep normal usage checks automatic. If the interval
selector stays visible, label custom choices **While working: 2 min** and
**While working: 5 min**; they do not replace quiet-account scheduling or
retry delays. The config reference can carry the full cadence table.

**CLI path: move to Advanced.** Label it **Claude Code executable** and add
**Used to read your account's usage.** Show the resolved path as selectable
text. Surface a missing executable outside the disclosure, with **Set
claude_path in the config file** and a direct config action.

**Keep-alive: remove the GUI control; retain the config key.** The 30/60-second
choice offers no clear user outcome. Both resend cached values; neither
refreshes usage. If a diagnostics view needs it, use **Device heartbeat** and
**Resends cached readings between usage checks. Default: 60 seconds.** Keep
technical tuning out of normal settings.

Use **Open log** and **Open config** instead of two context-free Open buttons.
Group those with the CLI path under Advanced. Keep Quit at the bottom with
**Stops usage updates and closes Clawdmeter.** Promote a detected legacy
service conflict as an actionable warning: **Disable legacy service**.
Report disable failures instead of treating the button press as success.

## First-run setup

| Step | Finding | Recommendation |
| --- | --- | --- |
| Welcome | Two paragraphs explain the mascot but omit prerequisites. | Lead with **Set up Clawdmeter**. State that setup installs firmware, pairs Bluetooth, and optionally adds hooks. Mention signed-in Claude Code and a USB data cable. |
| Board | Expanded shopping cards distract from connection and fall below the viewport. | Keep the shop collapsed, use the same board picker as Flash, and preserve visible footer actions. |
| Firmware current | “Which board is this?” conflicts with a detected board and current firmware. | Confirm the model and version; Continue is primary, reinstall secondary. |
| Flash | Recovery instructions appear during an active write. | Keep the cable-connected instruction; show recovery only after failure. Lock exit and navigation only while writing. |
| Pair | Universal PWR advice is wrong for LCD-4. | Show instructions for the selected board. For unknown boards, qualify PWR advice. Do not invent an LCD-4 bond-reset gesture. |
| Hooks | Privacy claim is incomplete; purpose is abstract. | Use the same purpose/data note and review flow as the Hooks page. Hooks are optional. |
| Summary | “You're all set” appears even when flash and hooks were skipped. | Use **Setup summary**. Report installed/current/not installed, connected/not connected, and hooks installed in N of M accounts. Offer links to unfinished steps. |

Green step dots currently reflect progression, which can include skipped
steps. Show skipped steps neutrally. Use **Continue without pairing** when
advancing disconnected; do not label it a completed pairing step. Use
**Open Clawdmeter** as the final action. Tray instructions must be conditional
on tray availability; without a tray, closing the window quits.

## Tray

Keep status and reading age above actions. Use **Refresh usage** consistently
with Overview and **Quit Clawdmeter** instead of “stops the daemon.” Explain
mute lifetime in Device settings. Keep firmware update and Settings actions
short. Use the same stale and sign-in terms as the window; a generic colored
icon should not carry the only error information.

## Device display and Clawd

**Evidence:** `device-meters-live.png`, `35-peek-*`, and `36-face-motion-sheet.png`.

Use **Session** instead of **Current** to match the desktop. Preserve **Weekly**,
short reset countdowns, and a clearly nonnumeric placeholder for unknown usage.
For spend-only accounts, say **of spending limit**, since the period can be a
day or week as well as a month.

**Owner decision: keep the whimsical status-verb rotation.** Claude Code's
thinking verbs mean all is well on the device; they do not assert that a
model request is currently running. Keep the spinner and ellipsis styling.
The same indicator shows **Waiting** without a connection, **No data** before
a reading, and **Stale** when a retained reading is old. Preserve the existing
**Listening** alternation in the stale/no-data state. Minimal display keeps
its fixed status text instead of rotating verbs.

This is decided, not an open copy recommendation. Keep the verb list,
including the longer words, and check its rendering on the smallest panel.

Pairing copy must name the actual gesture only on capable boards. Keep each
line short: **Pair again / Hold PWR for 3s / Then release**. On LCD-4, show
**Pair via Bluetooth / Choose Clawdmeter / On your computer** for initial
pairing; recovery from an existing bond needs an engineering answer.
Do not turn a hardware reset into a claimed unpair action.

The motion sheets preserve one character and return paths to the meters.
No new mascot behavior is needed for this copy pass. Check long alerts,
12-hour clock text, and spend/reset labels at 240×240, 368×448, 410×502,
and 480×480 after applying the deck. ASCII-only device fonts constrain copy.

## Decisions and remaining verification

The owner has decided to retain the whimsical thinking verbs as the device's
all-is-well indicator, with Waiting/Stale/No data for other states. The copy
deck marks these strings and their animation format as keep. The remaining
recommendations include hiding heartbeat tuning, moving CLI diagnostics to
Advanced, and making setup summaries reflect verified outcomes.

Engineering must establish the supported LCD-4 recovery path for a bond to
another computer before documenting one. Refreshed screenshot review and
native-size device captures remain necessary after implementation. This
review used the supplied captures and source inspection; it did not open
serial ports or invoke Claude Code against an account.
