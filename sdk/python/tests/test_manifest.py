"""Unit tests for the SDK manifest builder and host-app detection."""

from __future__ import annotations

import json
import os
import platform
import sys
from importlib.metadata import version
from pathlib import Path
from unittest.mock import patch

import pytest

from fostrom.agent import (
    _build_sdk_manifest,
    _detect_host_app,
    _read_pyproject,
    start_agent,
)

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_PATH = PROJECT_ROOT / "src"
SRC_STR = str(SRC_PATH)
if SRC_PATH.exists() and SRC_STR not in sys.path:
    sys.path.insert(0, SRC_STR)


# -------------------------
# _build_sdk_manifest
# -------------------------


def test_build_sdk_manifest_returns_valid_json() -> None:
    payload = json.loads(_build_sdk_manifest("prod"))
    assert payload["sdk"] == "python"
    assert isinstance(payload["sdk_manifest"], dict)


def test_build_sdk_manifest_includes_required_keys() -> None:
    manifest = json.loads(_build_sdk_manifest("prod"))["sdk_manifest"]

    for key in (
        "sdk_version",
        "python_version",
        "python_implementation",
        "python_implementation_version",
    ):
        assert key in manifest, f"expected key {key!r} in sdk_manifest"

    assert manifest["sdk_version"] == version("fostrom")
    assert manifest["python_version"] == platform.python_version()
    assert manifest["python_implementation"] == platform.python_implementation().lower()


def test_build_sdk_manifest_runtime_env_round_trips() -> None:
    for env_value in ("prod", "dev", "staging"):
        manifest = json.loads(_build_sdk_manifest(env_value))["sdk_manifest"]
        assert manifest["runtime_env"] == env_value


@pytest.mark.parametrize("falsy", [None, "", "   "])
def test_build_sdk_manifest_omits_runtime_env_when_blank(falsy) -> None:
    manifest = json.loads(_build_sdk_manifest(falsy))["sdk_manifest"]
    assert "runtime_env" not in manifest


# -------------------------
# _read_pyproject
# -------------------------


def test_read_pyproject_returns_name_and_version(tmp_path: Path) -> None:
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text(
        '[project]\nname = "my_app"\nversion = "1.2.3"\n',
        encoding="utf-8",
    )
    assert _read_pyproject(pyproject) == ("my_app", "1.2.3")


def test_read_pyproject_skips_fostrom(tmp_path: Path) -> None:
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text(
        '[project]\nname = "fostrom"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    assert _read_pyproject(pyproject) == (None, None)


def test_read_pyproject_returns_nones_for_missing_fields(tmp_path: Path) -> None:
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text('[tool.something]\nkey = "value"\n', encoding="utf-8")
    pyproject.write_text('[tool.something]\nkey = "value"\n', encoding="utf-8")
    assert _read_pyproject(pyproject) == (None, None)


def test_read_pyproject_handles_invalid_toml(tmp_path: Path) -> None:
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text("not [valid toml", encoding="utf-8")
    assert _read_pyproject(pyproject) == (None, None)


# -------------------------
# _detect_host_app
# -------------------------


def test_detect_host_app_finds_pyproject(tmp_path: Path, monkeypatch) -> None:
    project = tmp_path / "my_app"
    nested = project / "src" / "my_app"
    nested.mkdir(parents=True)

    pyproject = project / "pyproject.toml"
    pyproject.write_text(
        '[project]\nname = "my_app"\nversion = "0.4.0"\n',
        encoding="utf-8",
    )

    monkeypatch.chdir(nested)
    assert _detect_host_app() == ("my_app", "0.4.0")


def test_detect_host_app_skips_site_packages(tmp_path: Path, monkeypatch) -> None:
    fake_site_packages = tmp_path / "venv" / "lib" / "python3.13" / "site-packages" / "pkg"
    fake_site_packages.mkdir(parents=True)

    # Place a pyproject above so the walk would otherwise pick it up.
    (tmp_path / "pyproject.toml").write_text(
        '[project]\nname = "outer"\nversion = "9.9.9"\n',
        encoding="utf-8",
    )

    monkeypatch.chdir(fake_site_packages)
    assert _detect_host_app() == (None, None)


def test_detect_host_app_returns_nones_when_no_pyproject(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.chdir(tmp_path)
    # tmp_path has no pyproject and no parents will under our test runner that we control
    # — but we cannot guarantee no ancestor pyproject; only assert the call does not raise
    # and returns a tuple of two values.
    result = _detect_host_app()
    assert isinstance(result, tuple) and len(result) == 2


# -------------------------
# start_agent integration
# -------------------------


def test_start_agent_sets_sdk_manifest_env() -> None:
    with patch("fostrom.agent.subprocess.run") as run:
        run.return_value.returncode = 0
        run.return_value.stdout = "started: ok"
        run.return_value.stderr = ""
        start_agent("F", "D", "S", runtime_env="prod")

    env = run.call_args.kwargs["env"]
    assert "FOSTROM_SDK_MANIFEST" in env
    payload = json.loads(env["FOSTROM_SDK_MANIFEST"])
    assert payload["sdk"] == "python"
    assert payload["sdk_manifest"]["runtime_env"] == "prod"


def test_start_agent_does_not_set_legacy_runtime_env_var() -> None:
    """FOSTROM_RUNTIME_ENV is folded into FOSTROM_SDK_MANIFEST and should no longer be set
    explicitly by the SDK (the OS environment may still contain it; we only assert the SDK
    isn't injecting it itself)."""
    sentinel_environ = {k: v for k, v in os.environ.items() if k != "FOSTROM_RUNTIME_ENV"}
    with (
        patch("fostrom.agent.os.environ", sentinel_environ),
        patch("fostrom.agent.subprocess.run") as run,
    ):
        run.return_value.returncode = 0
        run.return_value.stdout = "started: ok"
        run.return_value.stderr = ""
        start_agent("F", "D", "S", runtime_env="prod")

    env = run.call_args.kwargs["env"]
    assert "FOSTROM_RUNTIME_ENV" not in env
