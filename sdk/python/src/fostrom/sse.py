from __future__ import annotations

import contextlib
import socket
import threading
import time
from typing import Callable


def _send_request(sock: socket.socket, headers: dict[str, str]) -> None:
    req = [
        "GET /events HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Accept: text/event-stream\r\n",
        "Cache-Control: no-cache\r\n",
    ]
    for k, v in headers.items():
        req.append(f"{k}: {v}\r\n")
    req.append("\r\n")
    sock.sendall("".join(req).encode("latin-1"))


def _read_until(sock: socket.socket, token: bytes) -> bytes:
    data = bytearray()
    while token not in data:
        chunk = sock.recv(1024)
        if not chunk:
            break
        data += chunk
    return bytes(data)


def _parse_events(buffer: str, chunk: str, handler: Callable[[dict[str, object]], None]) -> str:
    buffer += chunk
    lines = buffer.split("\n")
    buffer = lines.pop() or ""
    event: dict[str, object] = {}
    for raw in lines:
        line = raw.rstrip("\r")
        if line == "":
            # dispatch
            if "data" in event and isinstance(event["data"], str):
                import json

                with contextlib.suppress(Exception):
                    event["data"] = json.loads(event["data"])  # type: ignore[assignment]
            if event.get("event"):
                handler(event)
            event = {}
        elif line.startswith("data: "):
            prev = event.get("data")
            prev_str = prev if isinstance(prev, str) else ""
            event["data"] = prev_str + line[6:]
        elif line.startswith("event: "):
            event["event"] = line[7:]
        elif line.startswith("id: "):
            with contextlib.suppress(Exception):
                event["timestamp"] = int(line[4:])
    return buffer


class SSEThread(threading.Thread):
    def __init__(
        self, headers: dict[str, str], on_event: Callable[[dict[str, object]], None]
    ) -> None:
        super().__init__(daemon=True)
        self._headers = headers
        self._on_event = on_event
        self._stop_event = threading.Event()

    def stop(self) -> None:
        self._stop_event.set()

    def run(self) -> None:
        buffer = ""
        while not self._stop_event.is_set():
            sock: socket.socket | None = None
            try:
                sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                sock.connect("/tmp/fostrom/agent.sock")
                _send_request(sock, self._headers)
                # read headers
                _ = _read_until(sock, b"\r\n\r\n")
                # stream body
                while not self._stop_event.is_set():
                    chunk = sock.recv(2048)
                    if not chunk:
                        break
                    try:
                        decoded = chunk.decode("utf-8", errors="ignore")
                    except Exception:
                        decoded = chunk.decode("latin-1", errors="ignore")
                    buffer = _parse_events(buffer, decoded, self._on_event)
            except Exception:
                time.sleep(0.5)
            finally:
                with contextlib.suppress(Exception):
                    if sock is not None:
                        sock.close()
