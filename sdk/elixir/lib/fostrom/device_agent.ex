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
          handler: config[:handler] || Fostrom.DefaultHandler,
          collect_telemetry: Keyword.get(config, :collect_telemetry, true)
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
        {"FOSTROM_SDK_MANIFEST", build_sdk_manifest(config)}
      ] ++ telemetry_env(config.collect_telemetry)

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

  @doc false
  def telemetry_env(false), do: [{"FOSTROM_COLLECT_TELEMETRY", "false"}]

  def telemetry_env(secs) when is_integer(secs) and secs >= 15,
    do: [{"FOSTROM_COLLECT_TELEMETRY", to_string(secs)}]

  def telemetry_env(_), do: []

  @doc false
  def build_sdk_manifest(config) do
    {app_name, app_version} = detect_host_app()

    sdk_manifest =
      %{
        sdk_version: sdk_version(),
        elixir_version: System.version(),
        otp_release: to_string(:erlang.system_info(:otp_release)),
        erts_version: to_string(:erlang.system_info(:version)),
        schedulers: :erlang.system_info(:schedulers),
        runtime_env: to_string(config.env),
        app_name: app_name,
        app_version: app_version
      }
      |> Map.reject(fn {_k, v} -> is_nil(v) end)

    JSON.encode!(%{sdk: "elixir", sdk_manifest: sdk_manifest})
  end

  defp sdk_version do
    case Application.spec(:fostrom, :vsn) do
      nil -> nil
      vsn -> to_string(vsn)
    end
  end

  defp detect_host_app do
    Enum.find_value(:application.loaded_applications(), {nil, nil}, fn {app, _desc, vsn} ->
      cond do
        app == :fostrom -> nil
        host_app?(app) -> {to_string(app), to_string(vsn)}
        true -> nil
      end
    end)
  end

  defp host_app?(app) do
    case :application.get_key(app, :applications) do
      {:ok, deps} -> :fostrom in deps
      _ -> false
    end
  end
end
