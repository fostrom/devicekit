import subprocess
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path

from .errors import FostromError
from .fostrom import Fostrom
from .mailbox import Mail

PACKAGE_DIR = Path(__file__).parent
AGENT_PATH = PACKAGE_DIR / ".agent" / "fostrom-device-agent"
SCRIPT_PATH = PACKAGE_DIR / "dl-agent.sh"


def ensure_agent() -> None:
    try:
        _ = subprocess.run(["sh", str(SCRIPT_PATH), ".agent"], cwd=PACKAGE_DIR, check=True)
    except OSError:
        raise FostromError("agent_not_found", "dl-agent.sh not found") from None
    except subprocess.CalledProcessError:
        raise FostromError("agent_download_failed", "Failed to download Device Agent") from None

    if not AGENT_PATH.exists():
        raise FostromError("agent_not_found", "Fostrom Device Agent not found")


try:
    __version__ = version("fostrom")
except PackageNotFoundError:
    __version__ = "unknown"

ensure_agent()

__all__ = ["__version__", "Fostrom", "FostromError", "Mail"]
