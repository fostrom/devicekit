# Fostrom

Docs: <https://hexdocs.pm/fostrom>

[Fostrom](https://fostrom.io) is an IoT Cloud Platform built for developers. Monitor and control your fleet of devices, from microcontrollers to industrial IoT. Designed to be simple, secure, and fast. Experience first-class tooling with Device SDKs, type-safe schemas, programmable actions, and more.

The Fostrom Device SDK for Elixir works on Linux and macOS, and helps you quickly integrate, start monitoring, and controlling your IoT devices in just a few lines of code.

## Installation

The package can be installed by adding `fostrom` to your list of dependencies in `mix.exs`:

```elixir
def deps do
  [
    {:fostrom, "~> 0.0.13"}
  ]
end
```


## Configuration

```elixir
# Add in config/config.exs or config/runtime.exs:
config :fostrom, :config,
  fleet_id: "<fleet-id>",
  device_id: "<device-id>",
  device_secret: "<device-secret>",
  env: Config.config_env(),
  handler: MyApp.FostromHandler
```

## Defining a Handler

```elixir
defmodule MyApp.FostromHandler do
  use Fostrom.Handler

  # handle_mail needs to return either :ack, :reject, or :requeue
  def handle_mail(%Fostrom.Mail{} = mail) do
    # process the mail here
    :ack
  end
end
```

## Sending Datapoints and Messages

```elixir
# To send a datapoint:
Fostrom.send_datapoint("<packet-schema-name>", %{ ...payload })

# To send a message:
Fostrom.send_msg("<packet-schema-name>", %{ ...payload })
```


## A Note on the Fostrom Device Agent

The _Fostrom Device SDK_ downloads and runs the **Fostrom Device Agent** in the background. The Agent is downloaded when the library is first compiled. The Device Agent is started when your Elixir Application starts, and it remains running in the background forever.

We recommend you allow the Device Agent to run continously, even if your program has exited or crashed, so that when your program is automatically restarted by a process manager, the reconnection to Fostrom is nearly instant. However, if you wish to stop the Device Agent manually, you can call `Fostrom.DeviceAgent.stop()` in some `terminate()` callback in your application.
