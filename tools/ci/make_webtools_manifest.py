#!/usr/bin/env python3
"""Emit ESP Web Tools manifests (the browser-flasher fallback) from manifest.json.

    python3 tools/ci/make_webtools_manifest.py --dist dist [--base-url URL]

ESP Web Tools picks a build by chip family only, so several boards sharing one
chip (four S3 boards, two C6 boards) cannot live in one manifest. This writes
one manifest per board, dist/webtools-<env>.json, each with a single build
whose single part is the merged image at offset 0:

    {"name": "Clawdmeter — <display name>", "version": "v1.9.0",
     "new_install_prompt_erase": true,
     "builds": [{"chipFamily": "ESP32-S3",
                 "parts": [{"path": "<env>-v1.9.0.bin", "offset": 0}]}]}

`path` is resolved relative to the manifest URL; pass --base-url to make it
absolute (e.g. when the manifests are served from a Pages site and the images
from the Release).
"""

import argparse
import json
import sys
from pathlib import Path

# esptool chip name -> ESP Web Tools chipFamily
CHIP_FAMILIES = {
    "esp32": "ESP32",
    "esp32s2": "ESP32-S2",
    "esp32s3": "ESP32-S3",
    "esp32c3": "ESP32-C3",
    "esp32c6": "ESP32-C6",
    "esp32h2": "ESP32-H2",
}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dist", type=Path, required=True)
    ap.add_argument("--manifest", type=Path, help="default: <dist>/manifest.json")
    ap.add_argument("--base-url", default="", help="prefix for part paths")
    args = ap.parse_args()

    manifest = json.loads((args.manifest or args.dist / "manifest.json").read_text())
    base = args.base_url.rstrip("/") + "/" if args.base_url else ""
    for board in manifest["boards"]:
        family = CHIP_FAMILIES.get(board["chip"])
        if family is None:
            print(f"error: no ESP Web Tools family for chip {board['chip']}", file=sys.stderr)
            return 1
        web = {
            "name": f"Clawdmeter — {board['display_name']}",
            "version": manifest["version"],
            "new_install_prompt_erase": True,
            "builds": [{
                "chipFamily": family,
                "parts": [{"path": base + board["file"], "offset": board["offset"]}],
            }],
        }
        out = args.dist / f"webtools-{board['env']}.json"
        out.write_text(json.dumps(web, indent=2, ensure_ascii=False) + "\n")
        print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
