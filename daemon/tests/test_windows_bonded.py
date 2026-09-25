#!/usr/bin/env python3
"""Unit tests for the bonded-device address fallback — BLE-04.

A Clawdmeter that is paired AND connected to Windows (as a bonded HID
keyboard) no longer advertises, so BleakScanner.find_device_by_name() never
returns it. The daemon must then connect directly by the device's address,
which it recovers from the Windows PnP instance id.

These tests cover the pure parsing seam — recovering a canonical BLE MAC
("AA:BB:CC:DD:EE:FF") from a PnP instance id string.

Run: python -m pytest daemon/tests/test_windows_bonded.py -x -q
"""
import asyncio
from unittest.mock import MagicMock, patch

import pytest

import daemon.claude_usage_daemon_windows as win
from daemon.claude_usage_daemon_windows import (
    _mac_from_pnp_instance_id,
    acquire_target,
)


def _run(coro):
    return asyncio.run(coro)


def test_recovers_mac_from_standard_bthle_instance_id():
    instance_id = r"BTHLE\DEV_98A316A5D706\7&B8081D1&0&98A316A5D706"
    assert _mac_from_pnp_instance_id(instance_id) == "98:A3:16:A5:D7:06"


def test_uppercases_lowercase_hex():
    instance_id = r"BTHLE\DEV_aabbccddeeff\7&x&0&aabbccddeeff"
    assert _mac_from_pnp_instance_id(instance_id) == "AA:BB:CC:DD:EE:FF"


def test_returns_none_when_no_dev_token_present():
    assert _mac_from_pnp_instance_id(r"USB\VID_1234&PID_5678\ABC") is None


def test_returns_none_for_empty_string():
    assert _mac_from_pnp_instance_id("") is None


def test_ignores_short_hex_run_that_is_not_a_mac():
    # DEV_ must be followed by exactly 12 hex digits to be a BLE MAC.
    assert _mac_from_pnp_instance_id(r"BTHLE\DEV_98A3\junk") is None


# ---------------------------------------------------------------------------
# acquire_target: only ever targets the device bonded to THIS machine; it never
# scans for a nearby device by name (there is no scan fallback).
# ---------------------------------------------------------------------------

def test_acquire_target_uses_bonded_address():
    """The bonded address is wrapped in a BLEDevice and returned.

    A BLEDevice (not a bare string) is required so WinRT skips its advertisement
    scan and connects directly to the bonded device by address.
    """
    from bleak.backends.device import BLEDevice

    with patch("daemon.claude_usage_daemon_windows.discover_bonded_addresses",
               return_value=["98:A3:16:A5:D7:06"]):
        result = _run(acquire_target())

    assert isinstance(result, BLEDevice)
    assert result.address == "98:A3:16:A5:D7:06"


def test_acquire_target_returns_none_when_not_bonded():
    """No bonded device -> None so the caller backs off; never scans by name."""
    with patch("daemon.claude_usage_daemon_windows.discover_bonded_addresses", return_value=[]):
        result = _run(acquire_target())

    assert result is None


# ---------------------------------------------------------------------------
# Several paired boards: an unreachable one must not hold the daemon hostage
# ---------------------------------------------------------------------------
# Boards advertise their own name suffix, so a desk can hold several. Taking
# the first PnP row meant one powered-off board kept the daemon retrying it
# forever while a reachable one sat beside it — two hours of TimeoutError in
# the field.

def test_acquire_target_walks_candidates_after_failures():
    """Each failed attempt steps to the next paired board, then wraps around."""
    pair = ["AA:AA:AA:AA:AA:AA", "BB:BB:BB:BB:BB:BB"]
    win._candidate_index = 0        # test hook: start the walk at the top
    with patch("daemon.claude_usage_daemon_windows.discover_bonded_addresses", return_value=pair):
        first = _run(acquire_target())
        win.rotate_candidate()
        second = _run(acquire_target())
        win.rotate_candidate()
        third = _run(acquire_target())

    assert first.address == pair[0]
    assert second.address == pair[1]
    assert third.address == pair[0]        # wraps, so neither board is stranded


def test_acquire_target_stays_put_without_rotation():
    """No failure, no move: a dropped link reconnects to the same board."""
    pair = ["AA:AA:AA:AA:AA:AA", "BB:BB:BB:BB:BB:BB"]
    win._candidate_index = 0        # test hook: start the walk at the top
    win.rotate_candidate()                  # pretend board 2 is the working one
    with patch("daemon.claude_usage_daemon_windows.discover_bonded_addresses", return_value=pair):
        a = _run(acquire_target())
        b = _run(acquire_target())

    assert a.address == pair[1]
    assert b.address == pair[1]


# ---------------------------------------------------------------------------
# The `device` config line orders the candidates
# ---------------------------------------------------------------------------

def test_device_preference_sorts_match_first(monkeypatch):
    """A configured suffix is tried first; the others stay as fallbacks."""
    rows = [
        r"BTHLE\DEV_A4CB8FB1AFA1\9&1&A4CB8FB1AFA1",
        r"BTHLE\DEV_2884854BF629\9&1&2884854BF629",
    ]
    monkeypatch.setattr(win, "read_device_preference", lambda: "F629")
    monkeypatch.setattr(win.os.environ, "get", lambda *a: None)
    monkeypatch.setattr(win.sys, "platform", "win32")
    fake = MagicMock()
    fake.stdout = "\n".join(rows)
    monkeypatch.setattr(win.subprocess, "run", lambda *a, **k: fake)

    found = win.discover_bonded_addresses()

    assert found[0] == "28:84:85:4B:F6:29"   # preferred one first
    assert len(found) == 2                   # the other is kept, not hidden


def test_device_preference_typo_keeps_every_candidate(monkeypatch):
    """A suffix that matches nothing must degrade to 'wrong order', never to
    'no device at all' — otherwise a typo bricks the daemon silently."""
    rows = [
        r"BTHLE\DEV_A4CB8FB1AFA1\9&1&A4CB8FB1AFA1",
        r"BTHLE\DEV_2884854BF629\9&1&2884854BF629",
    ]
    monkeypatch.setattr(win, "read_device_preference", lambda: "ZZZZ")
    monkeypatch.setattr(win.os.environ, "get", lambda *a: None)
    monkeypatch.setattr(win.sys, "platform", "win32")
    fake = MagicMock()
    fake.stdout = "\n".join(rows)
    monkeypatch.setattr(win.subprocess, "run", lambda *a, **k: fake)

    found = win.discover_bonded_addresses()

    assert len(found) == 2


def test_no_scan_by_name_path_remains():
    """Regression guard: the by-name scan fallback and its config flag are gone."""
    assert not hasattr(win, "scan_for_device")
    assert not hasattr(win, "read_system_peripheral_only")
