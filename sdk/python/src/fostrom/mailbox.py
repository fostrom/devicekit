from __future__ import annotations

from typing import Protocol


class Mail:
    def __init__(
        self,
        instance: FostromProtocol,
        mail_id: int,
        name: str,
        payload: dict | None,
        mailbox_size: int,
    ) -> None:
        self._instance = instance
        self.id = mail_id
        self.name = name
        self.payload = payload
        self.mailbox_size = mailbox_size

    def ack(self) -> None:
        self._instance.mail_op("ack", self.id)

    def reject(self) -> None:
        self._instance.mail_op("reject", self.id)

    def requeue(self) -> None:
        self._instance.mail_op("requeue", self.id)


class FostromProtocol(Protocol):
    def mail_op(self, operation: str, mail_id: int) -> None: ...
