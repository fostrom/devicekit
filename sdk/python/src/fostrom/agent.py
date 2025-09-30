from __future__ import annotations

import contextlib
import os
import subprocess
from pathlib import Path

from .errors import FostromError

PACKAGE_DIR = Path(__file__).parent
AGENT_PATH = PACKAGE_DIR / ".agent" / "fostrom-device-agent"


def agent_path() -> Path:
    return AGENT_PATH


def start_agent(
    fleet_id: str, device_id: str, device_secret: str, runtime_env: str | None = None
) -> None:
    env = {
        **os.environ,
        "FOSTROM_FLEET_ID": str(fleet_id),
        "FOSTROM_DEVICE_ID": str(device_id),
        "FOSTROM_DEVICE_SECRET": str(device_secret),
    }
    if runtime_env is not None and str(runtime_env).strip() != "":
        env["FOSTROM_RUNTIME_ENV"] = str(runtime_env)

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
