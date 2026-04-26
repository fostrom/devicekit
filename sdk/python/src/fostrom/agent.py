from __future__ import annotations

import contextlib
import json
import os
import platform
import subprocess
import sys
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path

from .errors import FostromError

PACKAGE_DIR = Path(__file__).parent
AGENT_PATH = PACKAGE_DIR / ".agent" / "fostrom-device-agent"


def agent_path() -> Path:
    return AGENT_PATH


def start_agent(
    fleet_id: str,
    device_id: str,
    device_secret: str,
    runtime_env: str | None = None,
    collect_telemetry: bool | int = True,
) -> None:
    env = {
        **os.environ,
        "FOSTROM_FLEET_ID": str(fleet_id),
        "FOSTROM_DEVICE_ID": str(device_id),
        "FOSTROM_DEVICE_SECRET": str(device_secret),
        "FOSTROM_SDK_MANIFEST": _build_sdk_manifest(runtime_env),
    }
    if collect_telemetry is False:
        env["FOSTROM_COLLECT_TELEMETRY"] = "false"
    elif isinstance(collect_telemetry, int) and collect_telemetry >= 15:
        env["FOSTROM_COLLECT_TELEMETRY"] = str(collect_telemetry)

    try:
        result = subprocess.run(
            [str(AGENT_PATH), "start"],
            capture_output=True,
            text=True,
            check=False,
            env=env,
        )
        out = (result.stdout or "").strip()
        err = (result.stderr or "").strip()

        if result.returncode == 0:
            if out.startswith("started:") or out.startswith("already_started:"):
                return
            # Treat any other zero-exit as acceptable
            return

        # Non-zero exit: parse structured error if present
        text = out if out else err
        if text:
            parts = text.split(":", 1)
            error = parts[0] if parts else "failed"
            msg = parts[1].strip() if len(parts) == 2 else "Failed to start Device Agent"
            raise FostromError(error, msg)
        raise FostromError("failed", "Failed to start Device Agent")
    except FileNotFoundError:
        raise FostromError("agent_not_found", "Fostrom Device Agent not found") from None


def stop_agent() -> None:
    with contextlib.suppress(Exception):
        _ = subprocess.run([str(AGENT_PATH), "stop"], check=False, capture_output=True)


def _build_sdk_manifest(runtime_env: str | None) -> str:
    impl_version = ".".join(str(x) for x in sys.implementation.version[:3])
    app_name, app_version = _detect_host_app()

    sdk_manifest: dict[str, object] = {
        "sdk_version": _sdk_version(),
        "python_version": platform.python_version(),
        "python_implementation": platform.python_implementation().lower(),
        "python_implementation_version": impl_version,
    }

    if runtime_env is not None and str(runtime_env).strip() != "":
        sdk_manifest["runtime_env"] = str(runtime_env)
    if app_name is not None:
        sdk_manifest["app_name"] = app_name
    if app_version is not None:
        sdk_manifest["app_version"] = app_version

    sdk_manifest = {k: v for k, v in sdk_manifest.items() if v is not None}
    return json.dumps({"sdk": "python", "sdk_manifest": sdk_manifest}, separators=(",", ":"))


def _sdk_version() -> str | None:
    try:
        return version("fostrom")
    except PackageNotFoundError:
        return None


def _detect_host_app() -> tuple[str | None, str | None]:
    try:
        start = Path(os.getcwd()).resolve()
    except OSError:
        return None, None

    for directory in (start, *start.parents):
        # Don't claim Fostrom's own packaging or anything inside site-packages.
        parts = set(directory.parts)
        if "site-packages" in parts or "dist-packages" in parts or "node_modules" in parts:
            return None, None

        pyproject = directory / "pyproject.toml"
        if pyproject.is_file():
            return _read_pyproject(pyproject)

    return None, None


def _read_pyproject(path: Path) -> tuple[str | None, str | None]:
    try:
        import tomllib
    except ModuleNotFoundError:
        return None, None

    try:
        with path.open("rb") as f:
            data = tomllib.load(f)
    except (OSError, ValueError):
        return None, None

    project = data.get("project") or {}
    name = project.get("name")
    ver = project.get("version")

    # Skip Fostrom's own pyproject if we somehow walked into it.
    if isinstance(name, str) and name.lower() == "fostrom":
        return None, None
    if not isinstance(name, str):
        name = None
    if not isinstance(ver, str):
        ver = None
    return name, ver
