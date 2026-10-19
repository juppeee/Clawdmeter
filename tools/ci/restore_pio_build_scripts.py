#!/usr/bin/env python3
"""Undo the pioarduino platform's in-place edits to the shared framework package.

For an env with `lib_ignore`, the pinned pioarduino platform rewrites
`framework-arduinoespressif32-libs/<mcu>/pioarduino-build.py` in
~/.platformio/packages to drop the ignored libraries' include paths (SD drops
`fatfs`, where `ff.h` lives). It first copies the original to
`pioarduino-build.py.<mcu>` and copies it back after `checkprogsize`, the end
of a full `pio run`. Anything that runs the build scripts without that target,
such as `pio project metadata` in merge_firmware.py, leaves the edit in place.
CI then caches the edited package, and the next env that needs the dropped
headers fails to compile with nothing in its own config to explain it.

This puts every such backup back, the same way the platform does. (It covers
the build-script edits only. `custom_component_remove` would delete include
directories from the package with no backup; nothing here uses it.)

The VS Code PlatformIO extension runs the same metadata dump, so a local build
can hit this too; run the script once to repair the local packages:

    python tools/ci/restore_pio_build_scripts.py
"""

import os
import shutil
import sys
from pathlib import Path

BUILD_SCRIPT = "pioarduino-build.py"


def packages_dir() -> Path:
    core = os.environ.get("PLATFORMIO_CORE_DIR")
    return (Path(core) if core else Path.home() / ".platformio") / "packages"


def restore(packages: Path) -> list[Path]:
    """Copy each `pioarduino-build.py.<mcu>` backup over its build script and
    delete the backup. Returns the build scripts that were restored."""
    restored = []
    # `-libs*` also matches a second installed version (`-libs@<version>`).
    for backup in sorted(packages.glob(f"framework-arduinoespressif32-libs*/*/{BUILD_SCRIPT}.*")):
        target = backup.with_name(BUILD_SCRIPT)
        shutil.copy2(backup, target)
        backup.unlink()
        restored.append(target)
    return restored


def main() -> int:
    for path in restore(packages_dir()):
        print(f"restored {path} from the platform's lib_ignore backup")
    return 0


if __name__ == "__main__":
    sys.exit(main())
