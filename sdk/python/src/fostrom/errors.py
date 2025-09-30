from __future__ import annotations


class FostromError(Exception):
    """Custom exception for Fostrom SDK errors."""

    error: str
    message: str

    def __init__(self, error: str, message: str) -> None:
        self.error = error
        self.message = message
        formatted = f"[Fostrom Error] {error}: {message}"
        super().__init__(formatted)

    def __str__(self) -> str:  # pragma: no cover - same as message
        return f"[Fostrom Error] {self.error}: {self.message}"
