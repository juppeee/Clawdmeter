#!/bin/bash
# Flash helper for TinyUSB mode firmware.
# The built-in JTAG bootloader doesn't auto-reset with TinyUSB, so we
# trigger a reset via DTR/RTS on the CDC serial port before flashing.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PORT="${1:-/dev/ttyACM0}"

echo "=== Flashing Claude Usage Tracker ==="
echo "Port: $PORT"
echo ""

# Build first
echo "[1/3] Building firmware..."
cd "$SCRIPT_DIR/firmware"
~/.platformio/penv/bin/pio run

# Reset the device into bootloader via DTR/RTS
echo "[2/3] Resetting device into bootloader..."
python3 -c "
import serial, time
port = serial.Serial('$PORT', 115200)
port.dtr = False; port.rts = True; time.sleep(0.1)
port.dtr = True; port.rts = False; time.sleep(0.05)
port.dtr = False; time.sleep(0.1); port.close()
"
sleep 0.5

# Flash with no_reset (device is already in bootloader)
echo "[3/3] Flashing..."
python3 ~/.platformio/packages/tool-esptoolpy/esptool.py \
    --chip esp32s3 \
    --port "$PORT" \
    --before no_reset \
    --baud 921600 \
    write_flash -z \
    0x0 .pio/build/sc01plus/bootloader.bin \
    0x8000 .pio/build/sc01plus/partitions.bin \
    0xe000 ~/.platformio/packages/framework-arduinoespressif32/tools/partitions/boot_app0.bin \
    0x10000 .pio/build/sc01plus/firmware.bin

echo ""
echo "=== Done! Device will reboot automatically. ==="
