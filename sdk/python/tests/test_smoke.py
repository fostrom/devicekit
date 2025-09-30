"""Smoke tests for the Python Fostrom SDK."""

from __future__ import annotations

from importlib.metadata import version
from pathlib import Path

import fostrom


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
    vsn = fostrom.__version__
    assert version("fostrom") == vsn


def test_device_agent_downloaded() -> None:
    """Test that the device agent is downloaded."""
    PACKAGE_DIR = Path(__file__).parent.parent / "src" / "fostrom"
    AGENT_PATH = PACKAGE_DIR / ".agent" / "fostrom-device-agent"
    assert AGENT_PATH.exists()
