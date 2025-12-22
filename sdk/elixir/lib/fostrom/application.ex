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
        System.at_exit(fn _status -> stop(nil) end)
        Fostrom.DeviceAgent.start(config)

        children = [
          {Fostrom.HandlerProc, [handler: config.handler]},
          Fostrom.EventsProc
        ]

        Supervisor.start_link(children, opts)
    end
  end

  @impl true
  def stop(_state) do
    if conf = Application.get_env(:fostrom, :config) do
      if conf[:stop_agent_on_terminate] do
        Fostrom.DeviceAgent.stop()
      end
    end

    :ok
  end
end
