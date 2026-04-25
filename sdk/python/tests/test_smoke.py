"""Smoke tests for the Python Fostrom SDK."""

from __future__ import annotations

from importlib.metadata import version
from pathlib import Path
from unittest.mock import patch

import pytest

import fostrom
from fostrom import Fostrom
from fostrom.agent import start_agent

_BASE_CONFIG = {
    "fleet_id": "FOSTROM0",
    "device_id": "SANDBOX001",
    "device_secret": "FOS-TESTFLIGHTCONNFOSTROM0SANDBOX001",
}


def test_import() -> None:
    """Test that the package can be imported."""
    assert fostrom is not None

    from fostrom import Fostrom, FostromError, Mail

    assert Fostrom is not None
    assert FostromError is not None
    assert Mail is not None


def test_version() -> None:
    """Ensure __version__ is same as pyproject.toml"""
    assert hasattr(fostrom, "__version__")
    assert version("fostrom") == fostrom.__version__


def test_device_agent_downloaded() -> None:
    """Test that the device agent is downloaded."""
    PACKAGE_DIR = Path(__file__).parent.parent / "src" / "fostrom"
    AGENT_PATH = PACKAGE_DIR / ".agent" / "fostrom-device-agent"
    assert AGENT_PATH.exists()


# -------------------------
# collect_telemetry config
# -------------------------


@pytest.mark.parametrize(
    "overrides,expected",
    [
        ({}, True),
        ({"collect_telemetry": True}, True),
        ({"collect_telemetry": False}, False),
        ({"collect_telemetry": 120}, 120),
    ],
    ids=["default", "true", "false", "interval"],
)
def test_collect_telemetry_stored_on_instance(overrides, expected) -> None:
    app = Fostrom({**_BASE_CONFIG, **overrides})
    assert app._collect_telemetry == expected


@pytest.fixture
def patched_run():
    """Stub subprocess.run with a successful response and yield the mock."""
    with patch("fostrom.agent.subprocess.run") as run:
        run.return_value.returncode = 0
        run.return_value.stdout = "started: ok"
        run.return_value.stderr = ""
        yield run


@pytest.mark.parametrize(
    "kwargs,expected",
    [
        ({}, None),
        ({"collect_telemetry": True}, None),
        ({"collect_telemetry": False}, "false"),
        ({"collect_telemetry": 15}, "15"),
        ({"collect_telemetry": 90}, "90"),
        ({"collect_telemetry": 14}, None),
        ({"collect_telemetry": 0}, None),
        ({"collect_telemetry": -1}, None),
    ],
    ids=["default", "true", "false", "boundary_15", "above_15", "below_15", "zero", "negative"],
)
def test_start_agent_collect_telemetry_env(patched_run, kwargs, expected) -> None:
    start_agent("F", "D", "S", **kwargs)
    env = patched_run.call_args.kwargs["env"]
    if expected is None:
        assert "FOSTROM_COLLECT_TELEMETRY" not in env
    else:
        assert env["FOSTROM_COLLECT_TELEMETRY"] == expected
