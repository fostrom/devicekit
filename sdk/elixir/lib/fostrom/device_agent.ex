defmodule Fostrom.DeviceAgent do
  @moduledoc """
  Manages the starting and stopping of the Fostrom Device Agent
  """

  defp agent_path do
    path = Path.join([:code.priv_dir(:fostrom), ".agent", "fostrom-device-agent"])

    if not File.exists?(path) do
      raise "The Fostrom Device Agent is not downloaded. Run `mix fostrom.setup`."
    end

    path
  end

  @doc false
  def read_config do
    err = fn msg ->
      raise Fostrom.Exception,
            "[Fostrom] #{msg}. Refer to the docs to setup Fostrom correctly: https://hex.pm/fostrom"
    end

    case Application.fetch_env(:fostrom, :config) do
      {:ok, config} ->
        if is_nil(config[:fleet_id]), do: err.(":fleet_id is missing in config.")
        if is_nil(config[:device_id]), do: err.(":device_id is missing in config.")
        if is_nil(config[:device_secret]), do: err.(":device_secret is missing in config.")
        if is_nil(config[:env]), do: err.(":env is missing in config.")

        %{
          fleet_id: config[:fleet_id],
          device_id: config[:device_id],
          device_secret: config[:device_secret],
          env: config[:env],
          handler: config[:handler] || Fostrom.DefaultHandler
        }

      :error ->
        nil
    end
  end

  @doc false
  def start(config) do
    env =
      [
        {"FOSTROM_FLEET_ID", config.fleet_id},
        {"FOSTROM_DEVICE_ID", config.device_id},
        {"FOSTROM_DEVICE_SECRET", config.device_secret},
        {"FOSTROM_RUNTIME_ENV", to_string(config.env)}
      ]

    {output, status} = System.cmd(agent_path(), ["start"], env: env)
    output = String.trim(output)

    case status do
      0 ->
        case output do
          "started" <> _ -> :ok
          "already_started" <> _ -> :ok
          _ -> :ok
        end

      _ ->
        if output != "" do
          [reason, msg] = String.split(output, ":", parts: 2, trim: true)
          raise %Fostrom.Exception{reason: reason, message: msg}
        else
          raise %Fostrom.Exception{reason: "unknown", message: "Failed to start Device Agent"}
        end
    end
  end

  @doc """
  Stop the Fostrom Device Agent
  """
  def stop do
    System.cmd(agent_path(), ["stop"])
  end
end
