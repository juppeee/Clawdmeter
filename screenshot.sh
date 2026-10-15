#!/bin/bash
# Take a screenshot from the Waveshare AMOLED display via LVGL snapshot.
# Usage: ./screenshot.sh [output.png] [port]
# Default port: /dev/cu.usbmodem101 on macOS, /dev/ttyACM0 on Linux.

OUTPUT="${1:-screenshot.png}"
if [ -z "$2" ]; then
    case "$(uname -s)" in
        Darwin) PORT="/dev/cu.usbmodem101" ;;
        *)      PORT="/dev/ttyACM0" ;;
    esac
else
    PORT="$2"
fi

# Use pio's bundled python if pyserial isn't on the system python.
PY="python3"
if ! python3 -c "import serial" 2>/dev/null; then
    if [ -x "$HOME/.platformio/penv/bin/python" ]; then
        PY="$HOME/.platformio/penv/bin/python"
    fi
fi

TMPRAW=$(mktemp /tmp/screenshot_XXXXXX.raw)
TMPDIMS=$(mktemp /tmp/screenshot_XXXXXX.dims)
trap "rm -f '$TMPRAW' '$TMPDIMS'" EXIT

echo "Taking screenshot from $PORT..."

"$PY" - "$PORT" "$TMPRAW" "$TMPDIMS" << 'PYEOF'
import serial, sys

port_path, raw_path, dims_path = sys.argv[1], sys.argv[2], sys.argv[3]

port = serial.Serial(port_path, 115200, timeout=10)
port.reset_input_buffer()
port.write(b"screenshot\n")
port.flush()

while True:
    line = port.readline().decode("utf-8", errors="replace").strip()
    if line.startswith("SCREENSHOT_START"):
        parts = line.split()
        w, h, raw_size = int(parts[1]), int(parts[2]), int(parts[3])
        break
    if line == "SCREENSHOT_ERR":
        print("Device reported screenshot error", file=sys.stderr)
        sys.exit(1)

data = b""
while len(data) < raw_size:
    chunk = port.read(min(4096, raw_size - len(data)))
    if not chunk:
        print(f"Timeout: got {len(data)} of {raw_size} bytes", file=sys.stderr)
        sys.exit(1)
    data += chunk

with open(raw_path, "wb") as f:
    f.write(data)
with open(dims_path, "w") as f:
    f.write(f"{w}x{h}\n")

for _ in range(10):
    line = port.readline().decode("utf-8", errors="replace").strip()
    if line == "SCREENSHOT_END":
        break

port.close()
print(f"Captured {w}x{h} ({len(data)} bytes)")
PYEOF

if [ $? -ne 0 ]; then
    echo "Screenshot capture failed"
    exit 1
fi

DIMS=$(cat "$TMPDIMS")
ffmpeg -y -f rawvideo -pixel_format rgb565le -video_size "$DIMS" \
    -i "$TMPRAW" -update 1 -frames:v 1 "$OUTPUT" 2>/dev/null || true


if [ -f "$OUTPUT" ]; then
    echo "Saved: $OUTPUT ($DIMS)"
else
    echo "Error: conversion failed"
    exit 1
fi
