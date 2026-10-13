//! The hook script the Hooks tab wires into each account's settings.json.
//!
//! The app carries `daemon/attention-hook.sh` inside the binary and writes it
//! to the state dir, so the hooks point at a stable path that survives the
//! app moving or updating. The daemon tails what that script writes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clawdmeter_core::hooks::HOOK_SCRIPT;
use clawdmeter_core::Paths;

/// The script, as shipped in this build.
pub const SCRIPT: &str = include_str!("../../../../daemon/attention-hook.sh");

/// Where the installed copy lives: `<state>/attention-hook.sh`.
pub fn script_path(paths: &Paths) -> PathBuf {
    paths.state_dir.join(HOOK_SCRIPT)
}

/// Write (or refresh) the script; returns its path.
pub fn install_script(paths: &Paths) -> io::Result<PathBuf> {
    let path = script_path(paths);
    fs::create_dir_all(&paths.state_dir)?;
    if fs::read_to_string(&path).ok().as_deref() != Some(SCRIPT) {
        let tmp = path.with_extension("sh.tmp");
        fs::write(&tmp, SCRIPT)?;
        set_executable(&tmp)?;
        fs::rename(&tmp, &path)?;
    }
    Ok(path)
}

#[cfg(unix)]
fn set_executable(p: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(p, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_p: &Path) -> io::Result<()> {
    Ok(())
}

/// The script needs `jq` to read the hook JSON. Look for it where the script
/// does (PATH plus the Homebrew prefixes it prepends).
pub fn jq_available() -> bool {
    let exe = if cfg!(windows) { "jq.exe" } else { "jq" };
    let extra = ["/opt/homebrew/bin", "/usr/local/bin"];
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain(extra.iter().map(PathBuf::from))
        .any(|d| d.join(exe).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_executable_copy() {
        let dir = std::env::temp_dir().join(format!("clawdmeter-hookscript-{}", std::process::id()));
        let paths = Paths::under(&dir);
        let p = install_script(&paths).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), SCRIPT);
        assert!(p.ends_with(HOOK_SCRIPT));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o111, 0o111);
        }
        // Idempotent.
        install_script(&paths).unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn script_supports_event_mode() {
        assert!(SCRIPT.contains("events.log"), "hook script logs events for the fast clock");
    }
}
