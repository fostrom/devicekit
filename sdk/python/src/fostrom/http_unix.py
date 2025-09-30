from __future__ import annotations

import contextlib
import json
import socket

from .errors import FostromError


class Response:
    def __init__(self, status: int, headers: dict[str, str], body: bytes) -> None:
        self.status = status
        # Normalize header keys to lowercase
        self.headers = {k.lower(): v for k, v in headers.items()}
        self.body = body

    @property
    def text(self) -> str:
        try:
            return self.body.decode("utf-8", errors="strict")
        except Exception:
            return self.body.decode("utf-8", errors="ignore")

    @property
    def json(self) -> dict | None:
        ct = self.headers.get("content-type", "").lower()
        if "application/json" in ct and self.body:
            try:
                return json.loads(self.text)
            except json.JSONDecodeError:
                return None
        return None


def _read_line(sock: socket.socket) -> bytes:
    data = bytearray()
    while True:
        ch = sock.recv(1)
        if not ch:
            break
        data += ch
        if data.endswith(b"\r\n"):
            break
    return bytes(data)


def _read_headers(sock: socket.socket) -> tuple[int, dict[str, str]]:
    # Read status line
    status_line = _read_line(sock)
    if not status_line:
        raise FostromError("req_failed", "Empty response from Device Agent")
    # Example: HTTP/1.1 200 OK
    parts = status_line.decode("latin-1").strip().split(" ")
    status = 0
    if len(parts) >= 2:
        try:
            status = int(parts[1])
        except ValueError:
            status = 0

    headers: dict[str, str] = {}
    while True:
        line = _read_line(sock)
        if line in (b"\r\n", b""):
            break
        # name: value
        try:
            s = line.decode("latin-1").rstrip("\r\n")
            name, value = s.split(":", 1)
        except Exception:
            continue
        headers[name.strip()] = value.strip()

    return status, headers


def request(
    method: str,
    path: str,
    headers: dict[str, str] | None = None,
    body: bytes | None = None,
) -> Response:
    if method not in ("GET", "POST", "PUT", "DELETE", "HEAD"):
        raise ValueError(f"Unsupported method: {method}")

    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.connect("/tmp/fostrom/agent.sock")
        # Build request
        req = [f"{method} {path} HTTP/1.1\r\n", "Host: localhost\r\n"]
        # Inject headers
        if headers:
            for k, v in headers.items():
                req.append(f"{k}: {v}\r\n")

        if body is not None and method != "HEAD":
            req.append(f"Content-Length: {len(body)}\r\n")
        req.append("\r\n")
        s.sendall("".join(req).encode("latin-1"))
        if body is not None and method != "HEAD":
            s.sendall(body)

        status, resp_headers = _read_headers(s)
        # For event streams, server may omit Content-Length and keep alive; this helper
        # is not used for SSE.
        length_str = resp_headers.get("Content-Length") or resp_headers.get("content-length")
        body_bytes = b""
        if method != "HEAD":
            if length_str is not None:
                try:
                    to_read = int(length_str)
                except ValueError:
                    to_read = 0
                while to_read > 0:
                    chunk = s.recv(min(8192, to_read))
                    if not chunk:
                        break
                    body_bytes += chunk
                    to_read -= len(chunk)
            else:
                # Read until EOF
                while True:
                    chunk = s.recv(8192)
                    if not chunk:
                        break
                    body_bytes += chunk
        return Response(status, resp_headers, body_bytes)
    except OSError as e:
        raise FostromError(
            "req_failed", f"Communicating with the Device Agent failed: {e}"
        ) from None
    finally:
        with contextlib.suppress(Exception):
            s.close()
