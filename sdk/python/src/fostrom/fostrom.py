from __future__ import annotations

import logging
import os
from typing import Any, Callable, Mapping, cast

from .agent import start_agent
from .agent import stop_agent as agent_stop
from .errors import FostromError
from .http_unix import request as unix_request
from .mailbox import Mail
from .sse import SSEThread
from .utils import parse_bool, validate_pulse_name


class Fostrom:
    def __init__(self, config: Mapping[str, Any]) -> None:
        fleet_id = config.get("fleet_id")
        device_id = config.get("device_id")
        device_secret = config.get("device_secret")
        if not fleet_id:
            raise ValueError("[Fostrom] Fleet ID required.")
        if not device_id:
            raise ValueError("[Fostrom] Device ID required.")
        if not device_secret:
            raise ValueError("[Fostrom] Device Secret required.")

        self._fleet_id: str = str(fleet_id)
        self._device_id: str = str(device_id)
        self._device_secret: str = str(device_secret)
        self._log: bool = bool(config.get("log", True))
        # runtime env support: prefer config["env"], then config["runtime_env"], then PYTHON_ENV
        env_from_cfg = config.get("env") or config.get("runtime_env")
        self._runtime_env: str | None = (
            str(env_from_cfg) if env_from_cfg is not None else os.environ.get("PYTHON_ENV")
        )
        self._stop_agent_on_exit: bool = bool(config.get("stop_agent_on_exit", False))
        self._logger = logging.getLogger("fostrom")

        # Event handlers (can be reassigned by user)
        self.on_mail: Callable[[Mail], None] = self._default_mail_handler
        self.on_connected: Callable[[], None] = self._default_connected_handler
        self.on_unauthorized: Callable[[str, int], None] = self._default_unauthorized_handler
        self.on_reconnecting: Callable[[str, int], None] = self._default_reconnecting_handler

        self._sse_thread: SSEThread | None = None

    # ---------------
    # Public API
    # ---------------
    def start(self) -> None:
        start_agent(self._fleet_id, self._device_id, self._device_secret, self._runtime_env)
        self._start_sse()
        return None

    def send_datapoint(self, name: str, payload: dict[str, Any]) -> None:
        validate_pulse_name(name)
        _ = self._post_json(f"/pulse/datapoint/{name}", payload)

    def send_msg(self, name: str, payload: dict[str, Any] | None) -> None:
        validate_pulse_name(name)
        _ = self._post_json(f"/pulse/msg/{name}", payload)

    def mailbox_status(self) -> dict[str, Any]:
        res = self._head("/mailbox/next")
        h = res.headers
        empty = parse_bool(h.get("x-mailbox-empty"))
        if empty:
            return {"mailbox_size": 0, "next_mail_id": None, "next_mail_name": None}
        return {
            "mailbox_size": int(h.get("x-mailbox-size", "0") or 0),
            "next_mail_id": int(h.get("x-mail-id", "0") or 0),
            "next_mail_name": h.get("x-mail-name"),
        }

    def next_mail(self) -> Mail | None:
        res = self._get("/mailbox/next")
        h = res.headers
        empty = parse_bool(h.get("x-mailbox-empty"))
        if empty:
            return None
        mailbox_size = int(h.get("x-mailbox-size", "0") or 0)
        mail_id = int(h.get("x-mail-id", "0") or 0)
        name = h.get("x-mail-name", "")
        has_payload = parse_bool(h.get("x-mail-has-payload"))
        payload = res.json if has_payload else None
        return Mail(self, mail_id, name, payload, mailbox_size)

    def mail_op(self, operation: str, mail_id: int) -> None:
        if operation not in ("ack", "reject", "requeue"):
            raise ValueError("Invalid mailbox operation")
        res = self._put(f"/mailbox/{operation}/{mail_id}")
        more = parse_bool(res.headers.get("x-mail-available"))
        if more:
            mail = self.next_mail()
            if mail is not None:
                self._deliver_mail(mail)

    def shutdown(self, stop_agent: bool | None = None) -> None:
        if self._sse_thread is not None:
            try:
                self._sse_thread.stop()
                # Give the thread a brief chance to exit
                self._sse_thread.join(timeout=2.0)
            finally:
                self._sse_thread = None

        do_stop = self._stop_agent_on_exit if stop_agent is None else bool(stop_agent)
        if do_stop:
            agent_stop()

    @staticmethod
    def stop_agent() -> None:
        agent_stop()

    # Context manager support
    def __enter__(self) -> "Fostrom":  # pragma: no cover - simple passthrough
        return self

    def __exit__(self, exc_type, exc, tb) -> None:  # pragma: no cover - simple passthrough
        try:
            self.shutdown()
        except Exception:
            pass

    # ---------------
    # Defaults
    # ---------------
    def _default_mail_handler(self, mail: Mail) -> None:
        if self._log:
            self._logger.warning(
                "[Fostrom] Received Mail [%s -> ID %s] (Mailbox Size: %s)\n"
                "Warning: Auto-Acknowledging Mail. Define a handler via `fostrom.on_mail = ...`",
                mail.name,
                mail.id,
                mail.mailbox_size,
            )
        mail.ack()

    def _default_connected_handler(self) -> None:
        if self._log:
            self._logger.info("[Fostrom] Connected")

    def _default_unauthorized_handler(self, reason: str, after_ms: int) -> None:
        if self._log:
            after_s = (after_ms or 0) // 1000
            self._logger.critical(
                "[Fostrom] Unauthorized: %s. Reconnecting in %s seconds...",
                reason,
                after_s,
            )

    def _default_reconnecting_handler(self, reason: str, after_ms: int) -> None:
        if self._log:
            after_s = (after_ms or 0) // 1000
            self._logger.error(
                "[Fostrom] Failed to connect: %s. Reconnecting in %s seconds...",
                reason,
                after_s,
            )

    # ---------------
    # Internals
    # ---------------
    def _headers(self) -> dict[str, str]:
        return {
            "X-Fleet-ID": self._fleet_id,
            "X-Device-ID": self._device_id,
        }

    def _get_status(self) -> dict[str, Any]:
        res = self._get("/")
        obj = res.json
        if obj is None:
            raise FostromError("req_failed", "Invalid status response from Device Agent")
        return obj

    def _start_sse(self) -> None:
        if self._sse_thread is not None:
            return

        def on_event(evt: dict[str, object]) -> None:
            et = evt.get("event")
            data_obj = evt.get("data")
            if et == "connected":
                self.on_connected()
                return
            if et == "disconnected":
                if isinstance(data_obj, dict):
                    d = cast(dict[str, object], data_obj)
                    err_val = d.get("error", None)
                    err = str(err_val) if err_val is not None else ""
                    # Parse reason once for robustness (do not change callback API)
                    reason, _msg = self._parse_error(err)
                    after_val = d.get("reconnecting_in_ms", 0)
                    if isinstance(after_val, (int, float)) or (
                        isinstance(after_val, str) and after_val.isdigit()
                    ):
                        after = int(after_val)  # type: ignore[arg-type]
                    else:
                        after = 0
                    if reason == "unauthorized":
                        self.on_unauthorized(err, after)
                    else:
                        self.on_reconnecting(err, after)
                return
            if et == "new_mail":
                mail = self.next_mail()
                if mail is not None:
                    self._deliver_mail(mail)

        headers = self._headers()
        headers.update(
            {
                "Accept": "text/event-stream",
                "Connection": "keep-alive",
            }
        )
        self._sse_thread = SSEThread(headers, on_event)
        self._sse_thread.start()

    def _deliver_mail(self, mail: Mail) -> None:
        try:
            self.on_mail(mail)
        except Exception as _e:  # pragma: no cover - safety guard
            if self._log:
                self._logger.exception(
                    "[Fostrom] on_mail handler raised; auto-rejecting mail %s (%s)",
                    mail.id,
                    mail.name,
                )
            try:
                mail.reject()
            except Exception:
                # Swallow errors from rejection to avoid crashing event loop
                pass

    @staticmethod
    def _parse_error(err: str) -> tuple[str, str]:
        parts = err.split(":", 1)
        reason = parts[0].strip() if parts else ""
        msg = parts[1].strip() if len(parts) == 2 else ""
        return reason, msg

    # HTTP helpers
    def _get(self, path: str):
        hdrs = self._headers()
        hdrs["Accept"] = "application/json"
        res = unix_request("GET", path, hdrs, None)
        self._raise_if_error(res)
        return res

    def _head(self, path: str):
        hdrs = self._headers()
        res = unix_request("HEAD", path, hdrs, None)
        # Even for HEAD, server may respond with non-2xx; handle as errors with any JSON
        if res.status < 200 or res.status >= 300:
            obj = res.json
            if obj and "error" in obj:
                raise FostromError(obj.get("error", "request_failed"), obj.get("msg", ""))
            raise FostromError("request_failed", "Communicating with the Device Agent failed")
        return res

    def _put(self, path: str):
        hdrs = self._headers()
        hdrs["Accept"] = "application/json"
        res = unix_request("PUT", path, hdrs, None)
        self._raise_if_error(res)
        return res

    def _post_json(self, path: str, payload: dict[str, Any] | None):
        hdrs = self._headers()
        hdrs["Content-Type"] = "application/json; charset=utf-8"
        hdrs["Accept"] = "application/json"
        body = json_dumps_bytes(payload)
        res = unix_request("POST", path, hdrs, body)
        self._raise_if_error(res)
        return res

    @staticmethod
    def _raise_if_error(res) -> None:
        if res.status < 200 or res.status >= 300:
            obj = res.json
            if obj and "error" in obj:
                raise FostromError(obj.get("error", "request_failed"), obj.get("msg", ""))
            raise FostromError("request_failed", "Communicating with the Device Agent failed")


def json_dumps_bytes(obj: dict[str, Any] | None) -> bytes:
    import json

    return json.dumps(obj, separators=(",", ":")).encode("utf-8")
