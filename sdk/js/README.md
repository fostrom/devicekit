# Fostrom Device SDK

[Fostrom](https://fostrom.io) is an IoT Cloud Platform built for developers. Monitor and control your fleet of devices, from microcontrollers to industrial IoT. Designed to be simple, secure, and fast. Experience first-class tooling with Device SDKs, type-safe schemas, programmable actions, and more.

The Fostrom Device SDK for JavaScript works in Node.js and Bun, on Linux and macOS, and helps you quickly integrate, start monitoring, and controlling your IoT devices in just a few lines of code.

## Example

```js
import Fostrom from 'fostrom';

const fostrom = new Fostrom({
  fleet_id: "<fleet-id>",
  device_id: "<device-id>",
  device_secret: "<device-secret>",
})

// Setup the on_mail handler, to process incoming mail.
fostrom.onMail = async (mail) => {
  const {id, name, payload, mailbox_size} = mail
  console.debug(`Received Mail (${mailbox_size}): ${name} ${id}`)

  // You need to call `mail.ack()` to acknowledge the mail.
  // Other options are: `mail.reject()` and `mail.requeue()`
  // Note that if your function throws an error, the SDK will auto-reject the mail.
  await mail.ack()
}

async function main() {
  await fostrom.start()

  // Send a message to Fostrom (payload can be null if schema has no payload)
  await fostrom.sendMsg("<packet-schema-name>", { /* ...payload */ })

  // Send a datapoint to Fostrom
  await fostrom.sendDatapoint("<packet-schema-name>", { /* ...payload */ })
}

main()
```

> Requires Node.js 22.12+.

You can load the SDK via either syntax:

```js
import Fostrom from 'fostrom'
```

```js
const { default: Fostrom } = require('fostrom')
```

## A Note on the Device Agent

The Fostrom Device SDK downloads and runs the Fostrom Device Agent in the background. The Agent is downloaded when the package is installed through `npm`. The Device Agent starts when `fostrom.start()` is called and handles communication with the Fostrom platform.

By default, the agent remains running in the background for fast reconnections. The SDK also installs a process exit handler; `fostrom.shutdown()` is called automatically on process exit, and if `stopAgentOnExit` is set to true in the config, the agent is stopped as well.

## Logging

By default, the SDK logs connection and handler messages to the console. Set
`log: false` in the constructor config to silence SDK logs. Levels used:

- `console.info` for successful connection events
- `console.warn` for reconnection attempts
- `console.error` for unauthorized/critical errors
