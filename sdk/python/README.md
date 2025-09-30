# Fostrom Device SDK for Python

[Fostrom](https://fostrom.io) is an IoT Cloud Platform built for developers. Monitor and control your fleet of devices, from microcontrollers to industrial IoT. Designed to be simple, secure, and fast. Experience first-class tooling with Device SDKs, type-safe schemas, programmable actions, and more.

The Fostrom Device SDK for Python works with Python 3.10+ and helps you quickly integrate, start monitoring, and controlling your IoT devices in just a few lines of code.

## Installation

```bash
pip install fostrom
```

## Quick Start

```python
from fostrom import Fostrom, Mail

# Create SDK instance
fostrom = Fostrom({
    "fleet_id": "<your-fleet-id>",
    "device_id": "<your-device-id>",
    "device_secret": "<your-device-secret>",
    "env": "<dev|test|prod>",
})

# Setup mail handler for incoming messages
def handle_mail(mail: Mail):
    print(f"Received: {mail.name} ({mail.id})")
    mail.ack()  # Acknowledge the message

fostrom.on_mail = handle_mail

# Start the Device Agent and event stream
fostrom.start()

# Send sensor data
fostrom.send_datapoint("sensors", {
    "temperature": 23.5,
    "humidity": 65,
})

# Send status messages
fostrom.send_msg("status", {"online": True})
```

## API Reference

### Fostrom Class

#### `__init__(config)`
Create a new Fostrom instance.

**Parameters:**
- `config` (dict): Configuration dictionary with:
  - `fleet_id` (str): Your fleet ID
  - `device_id` (str): Your device ID
  - `device_secret` (str): Your device secret
  - `log` (bool, default: True): Enable logging (default: True)
  - `env` (str, default: PYTHON_ENV || "dev"): Runtime Environment
  - `stop_agent_on_exit` (bool, default: False): If True, `shutdown()` will stop the Device Agent too.

#### `start() -> None`
Start the Device Agent in the background and connect to it.

#### `shutdown(stop_agent: bool = False) -> None`
Stop the background event processing. If `stop_agent=True`, also stops the Device Agent.

#### `send_datapoint(name: str, payload: dict) -> None`
Send a datapoint to Fostrom.

#### `send_msg(name: str, payload: dict | None) -> None`
Send a message to Fostrom. Pass `None` if the schema has no payload.

#### `mailbox_status() -> dict`
Get current mailbox status.

#### `next_mail() -> Mail | None`
Get the next mail from the mailbox.


### Mail Class

#### Properties
- `id` (str): Mail ID
- `name` (str): Mail name/type
- `payload` (dict): Mail payload data
- `mailbox_size` (int): Current mailbox size

#### Methods
- `ack()`: Acknowledge the mail
- `reject()`: Reject the mail
- `requeue()`: Requeue the mail

## Device Agent

The Fostrom Device SDK downloads and runs the Fostrom Device Agent in the background. The Device Agent is downloaded automatically when the package is installed. The Device Agent starts when `fostrom.start()` is called and handles all communication with the Fostrom platform. The Device Agent runs continuously in the background, even after your Python program has terminated, to ensure that when your process manager restarts your Python program, it connects to the Device Agent right away. In case you want to stop the Device Agent when your program terminates, you can pass `stop_agent_on_exit: True` to the config.

## Logging

The SDK logs via Python's `logging` module using the `fostrom` logger. By default, WARNING/ERROR/CRITICAL are visible; INFO is not.

To see INFO messages (e.g., "Connected"), enable logging:

```python
import logging

logging.basicConfig(level=logging.INFO)
```

## Links

- **Fostrom Platform**: [https://fostrom.io](https://fostrom.io)
- **Documentation**: [https://docs.fostrom.io/sdk/py](https://docs.fostrom.io/sdk/py)
- **Python SDK**: [https://pypi.org/project/fostrom/](https://pypi.org/project/fostrom/)
