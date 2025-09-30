from __future__ import annotations

from .errors import FostromError


def parse_bool(value: str | None) -> bool:
    if value is None:
        return False
    s = value.strip().lower()
    return s in ("true", "1", "yes")


def validate_pulse_name(name: str) -> None:
    if len(name) == 0 or len(name) > 255:
        raise FostromError("invalid_name", "Pulse name must be 1..255 characters")
    for ch in name:
        if not (ch.isalnum() or ch in ("_", "-")):
            raise FostromError("invalid_name", "Pulse name may contain only A-Za-z0-9_-")
