#!/bin/bash
# Claude Usage Tracker Daemon (BLE)
# Reads Claude Code OAuth token, polls usage via API, sends to ESP32 over BLE GATT.
# Auto-connects and reconnects to the Claude Controller BLE device.
# Dependencies: curl, awk, bluetoothctl

DEVICE_NAME="Claude Controller"
DEVICE_MAC="${DEVICE_MAC:-}"  # auto-discovered if empty
SERVICE_UUID="4c41555a-4465-7669-6365-000000000001"
RX_CHAR_UUID="4c41555a-4465-7669-6365-000000000002"
POLL_INTERVAL=30
SAVED_MAC_FILE="$HOME/.config/claude-usage-monitor/ble-address"
DBUS_DEST="org.bluez"

log() {
    echo "[$(date '+%H:%M:%S')] $1"
}

read_token() {
    grep -o '"accessToken":"[^"]*"' "$HOME/.claude/.credentials.json" | cut -d'"' -f4
}

# Convert MAC to D-Bus path: AA:BB:CC:DD:EE:FF -> dev_AA_BB_CC_DD_EE_FF
mac_to_dbus_path() {
    local adapter
    adapter=$(busctl call org.bluez / org.freedesktop.DBus.ObjectManager GetManagedObjects 2>/dev/null | grep -o '/org/bluez/hci[0-9]' | head -1)
    adapter=${adapter:-/org/bluez/hci0}
    echo "${adapter}/dev_$(echo "$1" | tr ':' '_')"
}

# Check if device is connected via D-Bus
is_connected() {
    local path
    path=$(mac_to_dbus_path "$DEVICE_MAC")
    busctl get-property "$DBUS_DEST" "$path" org.bluez.Device1 Connected 2>/dev/null | grep -q "true"
}

# Load saved MAC address
load_mac() {
    if [ -n "$DEVICE_MAC" ]; then return 0; fi
    if [ -f "$SAVED_MAC_FILE" ]; then
        DEVICE_MAC=$(cat "$SAVED_MAC_FILE")
        [ -n "$DEVICE_MAC" ] && return 0
    fi
    return 1
}

# Save MAC for fast reconnect
save_mac() {
    mkdir -p "$(dirname "$SAVED_MAC_FILE")"
    echo "$DEVICE_MAC" > "$SAVED_MAC_FILE"
}

# Scan for Claude Controller
scan_for_device() {
    log "Scanning for '$DEVICE_NAME'..."
    # Start LE scan
    bluetoothctl scan le &>/dev/null &
    local scan_pid=$!
    sleep 8
    kill "$scan_pid" 2>/dev/null
    wait "$scan_pid" 2>/dev/null

    # Find the device
    local found
    found=$(bluetoothctl devices 2>/dev/null | grep "$DEVICE_NAME" | awk '{print $2}')
    if [ -n "$found" ]; then
        DEVICE_MAC="$found"
        save_mac
        log "Found: $DEVICE_MAC"
        return 0
    fi
    return 1
}

# Connect to the device
connect_device() {
    log "Connecting to $DEVICE_MAC..."

    # Trust first (allows auto-reconnect)
    bluetoothctl trust "$DEVICE_MAC" &>/dev/null

    # Connect
    bluetoothctl connect "$DEVICE_MAC" &>/dev/null
    sleep 2

    if is_connected; then
        log "Connected"
        return 0
    fi
    log "Connection failed"
    return 1
}

# Find the GATT characteristic handle via D-Bus
find_rx_char_path() {
    local dev_path
    dev_path=$(mac_to_dbus_path "$DEVICE_MAC")

    # Search for our RX characteristic UUID in the device's GATT tree
    busctl tree "$DBUS_DEST" 2>/dev/null | grep -o "${dev_path}/service[0-9a-f]*/char[0-9a-f]*" | while read -r char_path; do
        local uuid
        uuid=$(busctl get-property "$DBUS_DEST" "$char_path" org.bluez.GattCharacteristic1 UUID 2>/dev/null | tr -d '"' | awk '{print $2}')
        if [ "$uuid" = "$RX_CHAR_UUID" ]; then
            echo "$char_path"
            return 0
        fi
    done
}

# Write data to the RX characteristic via D-Bus
write_gatt() {
    local char_path="$1"
    local data="$2"

    # Convert string to byte array for D-Bus: "hi" -> 0x68 0x69
    local bytes=""
    for ((i = 0; i < ${#data}; i++)); do
        local byte
        byte=$(printf "0x%02x" "'${data:$i:1}")
        bytes="$bytes $byte"
    done
    local count=${#data}

    busctl call "$DBUS_DEST" "$char_path" org.bluez.GattCharacteristic1 \
        WriteValue "aya{sv}" "$count" $bytes 0 2>/dev/null
}

poll() {
    local token
    token=$(read_token) || { log "Error: could not read token"; return 1; }
    local now
    now=$(date +%s)

    local headers
    headers=$(curl -s -D - -o /dev/null \
        "https://api.anthropic.com/v1/messages" \
        -H "Authorization: Bearer $token" \
        -H "anthropic-version: 2023-06-01" \
        -H "anthropic-beta: oauth-2025-04-20" \
        -H "Content-Type: application/json" \
        -H "User-Agent: claude-code/2.1.5" \
        -d '{"model":"claude-haiku-4-5-20251001","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}' \
        2>/dev/null) || { log "Error: API call failed"; return 1; }

    local s5h_util s5h_reset s7d_util s7d_reset status
    s5h_util=$(echo "$headers" | grep -i "anthropic-ratelimit-unified-5h-utilization" | tr -d '\r' | awk '{print $2}')
    s5h_reset=$(echo "$headers" | grep -i "anthropic-ratelimit-unified-5h-reset" | tr -d '\r' | awk '{print $2}')
    s7d_util=$(echo "$headers" | grep -i "anthropic-ratelimit-unified-7d-utilization" | tr -d '\r' | awk '{print $2}')
    s7d_reset=$(echo "$headers" | grep -i "anthropic-ratelimit-unified-7d-reset" | tr -d '\r' | awk '{print $2}')
    status=$(echo "$headers" | grep -i "anthropic-ratelimit-unified-5h-status" | tr -d '\r' | awk '{print $2}')

    s5h_util=${s5h_util:-0}
    s5h_reset=${s5h_reset:-0}
    s7d_util=${s7d_util:-0}
    s7d_reset=${s7d_reset:-0}
    status=${status:-unknown}

    local payload
    payload=$(awk -v u5="$s5h_util" -v r5="$s5h_reset" -v u7="$s7d_util" -v r7="$s7d_reset" -v st="$status" -v now="$now" \
        'BEGIN {
            sp = sprintf("%.0f", u5 * 100);
            sr = (r5 - now) / 60; sr = sr > 0 ? sprintf("%.0f", sr) : 0;
            wp = sprintf("%.0f", u7 * 100);
            wr = (r7 - now) / 60; wr = wr > 0 ? sprintf("%.0f", wr) : 0;
            printf "{\"s\":%s,\"sr\":%s,\"w\":%s,\"wr\":%s,\"st\":\"%s\",\"ok\":true}", sp, sr, wp, wr, st;
        }')

    log "Sending: $payload"
    write_gatt "$RX_CHAR_PATH" "$payload" || { log "Write failed"; return 1; }
    return 0
}

cleanup() {
    log "Daemon stopped"
    exit 0
}

trap cleanup INT TERM

log "=== Claude Usage Tracker Daemon (BLE) ==="
log "Poll interval: ${POLL_INTERVAL}s"

BACKOFF=1

while true; do
    # Find the device
    if ! load_mac; then
        scan_for_device || {
            log "Device not found, retrying in ${BACKOFF}s..."
            sleep "$BACKOFF"
            BACKOFF=$((BACKOFF < 60 ? BACKOFF * 2 : 60))
            continue
        }
    fi

    # Connect if not connected
    if ! is_connected; then
        connect_device || {
            log "Retrying in ${BACKOFF}s..."
            sleep "$BACKOFF"
            BACKOFF=$((BACKOFF < 60 ? BACKOFF * 2 : 60))
            continue
        }
    fi

    # Find the GATT characteristic
    RX_CHAR_PATH=$(find_rx_char_path)
    if [ -z "$RX_CHAR_PATH" ]; then
        log "Error: RX characteristic not found, retrying..."
        sleep 5
        continue
    fi
    log "GATT RX path: $RX_CHAR_PATH"

    BACKOFF=1  # reset backoff on successful connection

    # Poll loop
    while is_connected; do
        poll || log "Poll failed, will retry"
        sleep "$POLL_INTERVAL"
    done

    log "Device disconnected, reconnecting..."
    sleep 2
done
