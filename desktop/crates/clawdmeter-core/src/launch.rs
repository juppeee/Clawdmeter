//! The path other programs should launch this app by: the login entry and the
//! Claude Code hooks store it, so it has to outlive this process.
//!
//! An AppImage runs from a `/tmp/.mount_*` dir that is gone after it exits;
//! the AppImage file itself (`$APPIMAGE`) is what stays. A Nix build runs
//! from `/nix/store/<hash>-…`, which an upgrade plus a garbage collection
//! removes; the profile's `bin/clawdmeter` symlink (`~/.nix-profile/bin`,
//! `/etc/profiles/per-user/<user>/bin`, `/run/current-system/sw/bin`) follows
//! upgrades. Anything else is launched from where it is.
//!
//! Some places can't be launched from later at all ([`Unstable`]): a macOS
//! app opened straight from its disk image or run translocated, and a Windows
//! exe run from inside a zip. Hooks and the login entry are not set up from
//! there, and entries that point there count as stale.

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

/// Why a launch path won't work later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unstable {
    /// On a mounted disk image (`/Volumes/…`): gone once it is ejected.
    DiskImage,
    /// macOS App Translocation: a downloaded app runs from a random,
    /// read-only copy that changes every launch.
    Translocated,
    /// A temporary folder, e.g. an exe run from inside a zip (Windows
    /// extracts it to `%TEMP%\Temp1_<name>.zip\`).
    Temporary,
}

impl Unstable {
    /// Where the path is, for "Runs <path>, <where>."
    pub fn location(self) -> &'static str {
        match self {
            Unstable::DiskImage => "which is on a disk image",
            Unstable::Translocated => "a temporary copy macOS made",
            Unstable::Temporary => "which is in a temporary folder",
        }
    }

    /// Where it runs from, for "Clawdmeter is running from <noun>."
    pub fn location_noun(self) -> &'static str {
        match self {
            Unstable::DiskImage => "its disk image",
            Unstable::Translocated => "a temporary copy macOS made",
            Unstable::Temporary => "a temporary folder",
        }
    }

    /// What to do about it.
    pub fn advice(self) -> &'static str {
        match self {
            Unstable::DiskImage | Unstable::Translocated => {
                "Move Clawdmeter to Applications first, then open it from there."
            }
            Unstable::Temporary => "Extract Clawdmeter from the zip first, then run it from there.",
        }
    }
}

/// [`unstable`] with the environment injected: `macos` turns on the disk
/// image and translocation checks, `temp_dirs` lists the temporary folders
/// (compared without case when `ignore_case`).
pub fn unstable_with(path: &Path, macos: bool, temp_dirs: &[PathBuf], ignore_case: bool) -> Option<Unstable> {
    let names: Vec<&OsStr> = path.components().map(|c| c.as_os_str()).collect();
    if macos {
        if names.len() > 2 && names[1] == "Volumes" && path.has_root() {
            return Some(Unstable::DiskImage);
        }
        if names.iter().any(|n| *n == "AppTranslocation") {
            return Some(Unstable::Translocated);
        }
    }
    if temp_dirs.is_empty() {
        return None;
    }
    let fold = |p: &Path| -> PathBuf {
        if ignore_case {
            PathBuf::from(p.to_string_lossy().to_lowercase())
        } else {
            p.to_path_buf()
        }
    };
    let folded = fold(path);
    let in_temp = temp_dirs.iter().any(|t| !t.as_os_str().is_empty() && folded.starts_with(fold(t)));
    // A folder named like an archive is an archive tool's extract-to-run.
    let in_zip = names.iter().rev().skip(1).any(|n| n.to_string_lossy().to_lowercase().ends_with(".zip"));
    (in_temp || in_zip).then_some(Unstable::Temporary)
}

/// Whether `path` (as hooks or the login entry would launch it) goes away
/// or changes. Checks the macOS cases on macOS and temporary folders on
/// Windows.
pub fn unstable(path: &Path) -> Option<Unstable> {
    let temps = if cfg!(windows) {
        let t = std::env::temp_dir();
        let canonical = std::fs::canonicalize(&t).ok();
        std::iter::once(t).chain(canonical).collect()
    } else {
        Vec::new()
    };
    let check = |p: &Path| unstable_with(p, cfg!(target_os = "macos"), &temps, cfg!(windows));
    check(path).or_else(|| std::fs::canonicalize(path).ok().and_then(|c| check(&c)))
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

    #[test]
    fn disk_images_and_translocation_on_macos() {
        let dmg = Path::new("/Volumes/Clawdmeter/Clawdmeter.app/Contents/MacOS/clawdmeter");
        assert_eq!(unstable_with(dmg, true, &[], false), Some(Unstable::DiskImage));
        assert_eq!(unstable_with(dmg, false, &[], false), None, "only on macOS");
        let moved =
            Path::new("/private/var/folders/xy/abc/T/AppTranslocation/1F2E/d/Clawdmeter.app/Contents/MacOS/clawdmeter");
        assert_eq!(unstable_with(moved, true, &[], false), Some(Unstable::Translocated));
        let installed = Path::new("/Applications/Clawdmeter.app/Contents/MacOS/clawdmeter");
        assert_eq!(unstable_with(installed, true, &[], false), None);
        assert_eq!(unstable_with(Path::new("/Volumes"), true, &[], false), None);
    }

    #[test]
    fn temporary_folders_and_zips() {
        let temp = [PathBuf::from("/Users/Me/AppData/Local/Temp")];
        let zip = Path::new("/users/me/appdata/local/temp/Temp1_clawdmeter-v2.0.0.zip/clawdmeter.exe");
        assert_eq!(unstable_with(zip, false, &temp, true), Some(Unstable::Temporary));
        let extracted_temp = Path::new("/Users/Me/AppData/Local/Temp/7zO1234/clawdmeter.exe");
        assert_eq!(unstable_with(extracted_temp, false, &temp, true), Some(Unstable::Temporary));
        assert_eq!(unstable_with(extracted_temp, false, &temp, false), Some(Unstable::Temporary));
        let elsewhere_zip = Path::new("/Users/Me/Downloads/clawdmeter.zip/clawdmeter.exe");
        assert_eq!(unstable_with(elsewhere_zip, false, &temp, true), Some(Unstable::Temporary));
        let installed = Path::new("/Users/Me/AppData/Local/Programs/Clawdmeter/clawdmeter.exe");
        assert_eq!(unstable_with(installed, false, &temp, true), None);
        assert_eq!(unstable_with(Path::new("/Users/Me/clawdmeter.zip"), false, &temp, true), None, "the file itself");
        assert_eq!(unstable_with(zip, false, &[], true), None, "not checked without temp dirs");
    }

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
