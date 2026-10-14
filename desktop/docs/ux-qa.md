# Refreshed desktop UX QA

Reviewed all 24 PNGs in `desktop/docs/screenshots/` from `4e9a028` against
[the UX review](ux-review.md) and [copy deck](copy-deck.md), with the applied
copy commits `28defe4` and `8732211`. Checked relevant source branches to
separate visible issues from states the captures do not exercise. Source
paths below are under `desktop/crates/clawdmeter-app/src/`.

The main changes are applied correctly: sidebar hierarchy and creator link,
Settings naming, refresh wait/checking feedback, no Overview mute row,
accurate hook privacy text, Board default for minimal display, Advanced
rather than exposed CLI/heartbeat controls, safe flash-progress guidance,
and neutral skipped steps in setup. No blocking text overlap is visible at
the captured 880×640 size.

## Fixes, in priority order

| Priority | File / screen | Problem | Concrete fix |
| --- | --- | --- | --- |
| P1 | `ui/setup.rs` (`done`, `hooks`, `completed`); setup Hooks / summary | **Source-confirmed edge case, not pictured:** `total = installed + pending` excludes unreadable account settings because `needs_install()` returns false for them. One installed account plus one unreadable account can report “1 of 1” and all hooks complete. | Count all configured accounts. Track unreadable separately; show “Hooks installed in 1 of 2 accounts. Couldn't read 1 account.” Keep the Hooks step incomplete and expose the error instead of the all-installed shortcut. |
| P1 | `ui/settings.rs` (`settings_pane`); `settings.png`, `settings-advanced.png` | A missing Claude Code executable is visible only after opening Advanced. The pictured remedy says to edit config, but Open config is disabled when the file does not exist. | Surface missing-executable guidance outside Advanced while keeping a healthy resolved path inside it. If config is absent, offer Create config (using the normal config writer), then Open config. Do not leave the only stated remedy inaccessible. |
| P2 | `ui/settings.rs` (`settings_pane`); both Settings captures | “Checks every 3 minutes while working” remains static even when a 2- or 5-minute override is selected. The screenshot's Automatic state is correct; the alternate states would contradict the selection. | Use “Automatic checks every 3 minutes while working, then less often when quiet. Retry delays apply to every check.” Or derive the description from the selected value. |
| P2 | `ui/flasher.rs` (`identify_view`); `flash-identify.png` | Fixed 146 px cards produce a 3+1 grid in the sidebar page, leaving Continue almost against the bottom edge. The same four choices fit in one row in setup because it has more width. | Use a balanced 2×2 layout at this content width, or responsive card widths. Preserve the four-across wizard layout and keep the actions clearly separated below the grid. Use the existing layout API. |
| P2 | `ui/settings.rs` (`hooks_pane`); `hooks.png`, `setup-5-hooks.png` | A single account has both Review install in its row and Review installation in 1 account below it. These open the same preview and compete as entry points. | For one pending account, keep one primary “Review installation” action and omit the duplicate row action. Retain per-account actions and the batch action when multiple accounts need changes. Preview's Install hooks / Cancel distinction is correct. |
| P2 | `ui/settings.rs` (`device_pane`); `device.png` | The board card presents cached identity as current even with USB disconnected. The long product name dominates the card; it also repeats update availability in a badge and version line. | Label cached identity/version “Last detected”; use the short board name as the heading, with SoC where needed to distinguish C6. Put the full product name in details and keep one update-availability indicator plus its action. |
| P2 | `ui/flasher.rs` (`done_view`); `flash-done.png` | The success page says “The board is restarting” while the badge already says Connected. The last progress label “Set up” also suggests another installation task remains. | When connected, say “Firmware installed. Bluetooth connected.” Otherwise give pairing guidance without an ongoing restart claim. Rename the final progress label to Done. |
| P2 | `ui/settings.rs` (`accounts_pane`); `accounts.png` | Account selection and retry copy are accurate, but the page still assumes the reader knows which folder to add. Remove does not explain whether files or hooks are affected. | Add “The default Claude Code folder is ~/.claude.” Give Remove concise help: “Stops monitoring this folder. Files and installed hooks stay in place.” Explain the disabled action when only one account remains. |
| P3 | `ui/flasher.rs` (`firmware_view`); `flash-firmware.png` | The filename and demo-release path have nearly the same emphasis as the chosen board and version. They make the confirmation card read like a diagnostic dump. | Keep board, version, and checksum result in the main card. Put filename, source, and USB path in a Details disclosure. Keep Install as the sole primary action. |
| P3 | `ui/overview.rs`; `overview-not-running.png` | “Start Clawdmeter to update usage” and the notice immediately below repeat the same instruction. | Keep the Not running heading and one actionable sentence: “Start Clawdmeter from your app menu, or run clawdmeter in a terminal.” Remove the redundant subtitle. |

## Coverage and remaining checks

| Captures inspected | Result |
| --- | --- |
| `overview`, `overview-refresh-wait`, `overview-refresh-checking`, `overview-gnome-header`, `overview-not-running` | Usage remains primary; refresh feedback fits beside the title. GNOME header and close control fit. Only stopped-state duplication above. |
| `device`, `accounts`, `settings`, `settings-advanced` | Copy is applied and rows align. Remaining issues listed above. Device's lower controls are below the captured viewport. |
| `flash-connect`, `flash-identify`, `flash-firmware`, `flash-progress`, `flash-done` | Connect and progress instructions are correct; picker, confirmation hierarchy, and completion wording need the listed adjustments. |
| `hooks`, `hooks-preview` | Privacy, backup, new-file notice, and apply/cancel copy are correct. Duplicate entry actions remain. The diff continues below the viewport. |
| `setup-1-welcome`, `setup-2-board-shop`, `setup-2-board`, `setup-2-up-to-date`, `setup-3-flash`, `setup-4-pair`, `setup-5-hooks`, `setup-6-done` | Welcome prerequisites fit; current firmware has the right heading and action priority; progress blocks exit; pairing and summary text match the pictured state. Shop content scrolls under the fixed footer by design; source provides a scroll container. Hooks issues above still apply. |

README screenshot references already resolve: it embeds `overview.png` and
`flash-identify.png`, neither renamed. No README edit is needed.

This was screenshot and source QA, not an interactive run. Before closing
visual QA, capture the scrolled Device controls, lower board-shop cards, and
hook diff footer; exercise missing CLI/config, unreadable account settings,
non-Automatic cadence, and no-tray setup summary. Confirm keyboard focus and
scroll reachability in the running app. Current source includes board-aware
pairing hints and icon tooltips, but these screenshots do not prove those
alternate states or interactions. Device status verbs remain an owner-decided
keep; this desktop pass does not reopen that decision.
