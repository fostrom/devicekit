"""Integration tests mirroring the Elixir SDK checks."""
# ruff: noqa: E402

from __future__ import annotations

import os
import subprocess
import sys
import time
import unittest
from contextlib import contextmanager
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_PATH = PROJECT_ROOT / "src"
SRC_STR = str(SRC_PATH)
if SRC_PATH.exists() and SRC_STR not in sys.path:
    sys.path.insert(0, SRC_STR)

from fostrom import Fostrom, Mail
from fostrom.agent import agent_path
from fostrom.errors import FostromError
from fostrom.http_unix import request as unix_request

FLEET_ID = "FOSTROM0"
DEVICE_ID = "SANDBOX001"
DEVICE_SECRET = "FOS-TESTFLIGHTCONNFOSTROM0SANDBOX001"
AGENT_SOCKET = Path("/tmp/fostrom/agent.sock")


def _device_agent_binary() -> Path:
    bin_path = agent_path()
    if not bin_path.exists():
        raise unittest.SkipTest(
            "Device Agent binary not found; run the SDK build to bundle it before testing."
        )
    return bin_path


def _device_agent_version() -> str:
    bin_path = _device_agent_binary()
    result = subprocess.run(
        [str(bin_path), "version"],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def _wait_for_socket_absence(timeout: float = 5.0) -> None:
    deadline = time.time() + timeout
    while AGENT_SOCKET.exists() and time.time() < deadline:
        time.sleep(0.1)


def _wait_for_agent_response(timeout: float = 10.0):
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            resp = unix_request("GET", "/")
        except FostromError as exc:  # Agent not ready yet
            last_error = exc
            time.sleep(0.1)
            continue

        if resp.status == 200:
            return resp
        time.sleep(0.1)

    if last_error is not None:
        raise AssertionError(f"Device Agent did not become ready: {last_error}")
    raise AssertionError("Device Agent did not respond with 200 OK in time")


@contextmanager
def _temporary_env(key: str, value: str):
    previous = os.environ.get(key)
    os.environ[key] = value
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = previous


def test_device_agent_not_running_initially() -> None:
    """Ensure the agent is not running before the SDK starts it."""
    _device_agent_binary()
    Fostrom.stop_agent()
    _wait_for_socket_absence()

    if AGENT_SOCKET.exists():
        raise unittest.SkipTest(
            "Device Agent socket still present after stop; environment already running agent."
        )

    try:
        unix_request("GET", "/")
    except FostromError:
        return
    raise AssertionError("Device Agent responded even though it should be stopped")


def test_start_fostrom_sdk() -> None:
    """Start the SDK, then validate Device Agent headers via the Unix socket."""
    _device_agent_binary()
    Fostrom.stop_agent()
    _wait_for_socket_absence()
    agent_version = _device_agent_version()

    with _temporary_env("FOSTROM_LOCAL_MODE", "true"):
        app = Fostrom(
            {
                "fleet_id": FLEET_ID,
                "device_id": DEVICE_ID,
                "device_secret": DEVICE_SECRET,
                "stop_agent_on_exit": True,
            }
        )

        def handle_mail(mail: Mail) -> None:
            print(f"test: received_mail {mail.mailbox_size} {mail.name} {mail.id}")
            mail.ack()

        def handle_connected() -> None:
            print("test: connected")

        def handle_unauthorized(reason: str, after: int) -> None:
            print(f"test: unauthorized {reason} {after}")

        def handle_reconnecting(reason: str, after: int) -> None:
            print(f"test: connect_failed {reason} {after}")

        app.on_mail = handle_mail
        app.on_connected = handle_connected
        app.on_unauthorized = handle_unauthorized
        app.on_reconnecting = handle_reconnecting

        try:
            app.start()
            resp = _wait_for_agent_response()

            assert resp.status == 200
            headers = resp.headers

            assert headers.get("x-powered-by") == "Fostrom"
            assert headers.get("x-protocol") == "Moonlight"
            assert headers.get("x-protocol-version") == "1"
            assert headers.get("x-api-version") == "1"

            expected_agent_version = agent_version.removeprefix("v")
            assert headers.get("x-agent-version") == expected_agent_version
            assert headers.get("server") == f"Fostrom-Device-Agent/{agent_version}"
            assert headers.get("x-fleet-id") == FLEET_ID
            assert headers.get("x-device-id") == DEVICE_ID

            payload = resp.json
            assert isinstance(payload, dict)
        finally:
            app.shutdown(stop_agent=True)
            _wait_for_socket_absence()
