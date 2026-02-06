from __future__ import annotations

import sys
from pathlib import Path

from fostrom.sse import _parse_events

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_PATH = PROJECT_ROOT / "src"
SRC_STR = str(SRC_PATH)
if SRC_PATH.exists() and SRC_STR not in sys.path:
    sys.path.insert(0, SRC_STR)


def test_parse_events_handles_split_frame_with_state() -> None:
    received: list[dict[str, object]] = []
    buffer = ""
    event: dict[str, object] = {}

    # Split one SSE frame over two chunks to simulate socket fragmentation.
    buffer, event = _parse_events(buffer, "event: new_mail\n", received.append, event)
    buffer, event = _parse_events(buffer, "\n", received.append, event)

    assert buffer == ""
    assert event == {}
    assert received == [{"event": "new_mail"}]
