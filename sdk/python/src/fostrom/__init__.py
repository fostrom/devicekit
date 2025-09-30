import subprocess
import warnings
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path

from .errors import FostromError
from .fostrom import Fostrom
from .mailbox import Mail

try:
    __version__ = version("fostrom")
except PackageNotFoundError:
    __version__ = "unknown"


PACKAGE_DIR = Path(__file__).parent
AGENT_PATH = PACKAGE_DIR / ".agent" / "fostrom-device-agent"
SCRIPT_PATH = PACKAGE_DIR / "dl-agent.sh"


def ensure_agent() -> None:
    if not AGENT_PATH.exists():
        print("Downloading Fostrom Device Agent...")
        _ = subprocess.run(["sh", str(SCRIPT_PATH), ".agent"], cwd=PACKAGE_DIR, check=True)


try:
    ensure_agent()
except Exception as e:
    warnings.warn(f"Failed to download Fostrom Device Agent: {e}", stacklevel=2)

__all__ = ["__version__", "Fostrom", "FostromError", "Mail"]
