defmodule Fostrom.Application do
  @moduledoc false
  require Logger
  use Application

  @impl true
  def start(_type, _args) do
    opts = [strategy: :one_for_one, name: Fostrom.Supervisor]

    case Fostrom.DeviceAgent.read_config() do
      nil ->
        Logger.warning("Fostrom will not be started: missing configuration")
        Supervisor.start_link([], opts)

      config ->
        Fostrom.DeviceAgent.start(config)

        children = [
          {Fostrom.HandlerProc, [handler: config.handler]},
          Fostrom.EventsProc
        ]

        Supervisor.start_link(children, opts)
    end
  end
end
