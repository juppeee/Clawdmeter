//! The path other programs should launch this app by: the login entry and the
//! Claude Code hooks store it, so it has to outlive this process.
//!
//! An AppImage runs from a `/tmp/.mount_*` dir that is gone after it exits;
//! the AppImage file itself (`$APPIMAGE`) is what stays. A Nix build runs
//! from `/nix/store/<hash>-…`, which an upgrade plus a garbage collection
//! removes; the profile's `bin/clawdmeter` symlink (`~/.nix-profile/bin`,
//! `/etc/profiles/per-user/<user>/bin`, `/run/current-system/sw/bin`) follows
//! upgrades. Anything else is launched from where it is.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

const NIX_STORE: &str = "/nix/store";

/// The binary's file name on this OS.
pub fn binary_name() -> String {
    format!("clawdmeter{}", std::env::consts::EXE_SUFFIX)
}

/// [`stable_launch_path`] with the environment injected: `appimage` is
/// `$APPIMAGE`, `current` the running executable, `path_var` `$PATH`, and
/// `is_file` tells whether a candidate exists.
pub fn stable_launch_path_with(
    appimage: Option<&OsStr>,
    current: &Path,
    path_var: Option<&OsStr>,
    is_file: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    if let Some(app) = appimage.filter(|a| !a.is_empty()) {
        return PathBuf::from(app);
    }
    if current.starts_with(NIX_STORE) {
        let name = binary_name();
        let found = path_var
            .into_iter()
            .flat_map(std::env::split_paths)
            .filter(|d| d.is_absolute() && !d.starts_with(NIX_STORE))
            .map(|d| d.join(&name))
            .find(|p| is_file(p));
        if let Some(p) = found {
            return p;
        }
    }
    current.to_path_buf()
}

/// Where the login entry and hooks should point to launch this app.
pub fn stable_launch_path() -> io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    Ok(stable_launch_path_with(
        std::env::var_os("APPIMAGE").as_deref(),
        &current,
        std::env::var_os("PATH").as_deref(),
        &|p| p.is_file(),
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn exists<'a>(files: &'a [&'a str]) -> impl Fn(&Path) -> bool + 'a {
        move |p| files.iter().any(|f| Path::new(f) == p)
    }

    const STORE_EXE: &str = "/nix/store/abc123-clawdmeter-0.4.0/bin/clawdmeter";

    #[test]
    fn appimage_wins() {
        let p = stable_launch_path_with(
            Some(OsStr::new("/home/me/Apps/Clawdmeter.AppImage")),
            Path::new("/tmp/.mount_ClawdmXYZ/usr/bin/clawdmeter"),
            Some(OsStr::new("/usr/bin")),
            &exists(&["/usr/bin/clawdmeter"]),
        );
        assert_eq!(p, Path::new("/home/me/Apps/Clawdmeter.AppImage"));
    }

    #[test]
    fn empty_appimage_is_ignored() {
        let p = stable_launch_path_with(Some(OsStr::new("")), Path::new("/usr/bin/clawdmeter"), None, &|_| true);
        assert_eq!(p, Path::new("/usr/bin/clawdmeter"));
    }

    #[test]
    fn nix_store_prefers_the_first_profile_bin_on_path() {
        let path = "/nix/store/zzz-coreutils/bin:/home/me/.nix-profile/bin:/etc/profiles/per-user/me/bin:/run/current-system/sw/bin";
        let p = stable_launch_path_with(
            None,
            Path::new(STORE_EXE),
            Some(OsStr::new(path)),
            &exists(&[
                "/nix/store/zzz-coreutils/bin/clawdmeter",
                "/etc/profiles/per-user/me/bin/clawdmeter",
                "/run/current-system/sw/bin/clawdmeter",
            ]),
        );
        assert_eq!(p, Path::new("/etc/profiles/per-user/me/bin/clawdmeter"));
    }

    #[test]
    fn nix_store_skips_store_and_relative_path_entries() {
        let p = stable_launch_path_with(
            None,
            Path::new(STORE_EXE),
            Some(OsStr::new("bin:/nix/store/abc123-clawdmeter-0.4.0/bin")),
            &|_| true,
        );
        assert_eq!(p, Path::new(STORE_EXE));
    }

    #[test]
    fn nix_store_without_a_profile_keeps_the_store_path() {
        let p = stable_launch_path_with(None, Path::new(STORE_EXE), Some(OsStr::new("/usr/bin")), &exists(&[]));
        assert_eq!(p, Path::new(STORE_EXE));
        assert_eq!(stable_launch_path_with(None, Path::new(STORE_EXE), None, &|_| true), Path::new(STORE_EXE));
    }

    #[test]
    fn other_installs_launch_from_where_they_are() {
        // On PATH elsewhere doesn't matter outside the store.
        let p = stable_launch_path_with(
            None,
            Path::new("/home/me/.cargo/bin/clawdmeter"),
            Some(OsStr::new("/usr/bin")),
            &|_| true,
        );
        assert_eq!(p, Path::new("/home/me/.cargo/bin/clawdmeter"));
    }
}
