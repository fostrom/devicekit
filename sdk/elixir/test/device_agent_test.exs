defmodule Fostrom.DeviceAgentTest do
  use ExUnit.Case, async: false
  @moduletag capture_log: true

  @base_config [
    fleet_id: "FOSTROM0",
    device_id: "SANDBOX001",
    device_secret: "FOS-TESTFLIGHTCONNFOSTROM0SANDBOX001",
    env: :test
  ]

  setup do
    on_exit(fn -> Application.delete_env(:fostrom, :config) end)
    :ok
  end

  test "read_config/0 defaults collect_telemetry to true when omitted" do
    Application.put_env(:fostrom, :config, @base_config)
    assert %{collect_telemetry: true} = Fostrom.DeviceAgent.read_config()
  end

  test "read_config/0 surfaces collect_telemetry: false" do
    Application.put_env(:fostrom, :config, @base_config ++ [collect_telemetry: false])
    assert %{collect_telemetry: false} = Fostrom.DeviceAgent.read_config()
  end

  test "read_config/0 surfaces a numeric collect_telemetry interval" do
    Application.put_env(:fostrom, :config, @base_config ++ [collect_telemetry: 120])
    assert %{collect_telemetry: 120} = Fostrom.DeviceAgent.read_config()
  end

  describe "telemetry_env/1" do
    test "false sets FOSTROM_COLLECT_TELEMETRY=false" do
      assert Fostrom.DeviceAgent.telemetry_env(false) == [{"FOSTROM_COLLECT_TELEMETRY", "false"}]
    end

    test "an integer >= 15 sets the interval" do
      assert Fostrom.DeviceAgent.telemetry_env(15) == [{"FOSTROM_COLLECT_TELEMETRY", "15"}]
      assert Fostrom.DeviceAgent.telemetry_env(120) == [{"FOSTROM_COLLECT_TELEMETRY", "120"}]
    end

    test "an integer < 15 omits the env var (agent will use its default)" do
      assert Fostrom.DeviceAgent.telemetry_env(14) == []
      assert Fostrom.DeviceAgent.telemetry_env(0) == []
      assert Fostrom.DeviceAgent.telemetry_env(-1) == []
    end

    test "true and other values omit the env var" do
      assert Fostrom.DeviceAgent.telemetry_env(true) == []
      assert Fostrom.DeviceAgent.telemetry_env(nil) == []
      assert Fostrom.DeviceAgent.telemetry_env("60") == []
    end
  end

  describe "build_sdk_manifest/1" do
    test "returns valid JSON with sdk == \"elixir\"" do
      json = Fostrom.DeviceAgent.build_sdk_manifest(%{env: :prod})
      decoded = JSON.decode!(json)

      assert %{"sdk" => "elixir", "sdk_manifest" => %{} = manifest} = decoded
      assert is_map(manifest)
    end

    test "includes the expected language/runtime keys" do
      json = Fostrom.DeviceAgent.build_sdk_manifest(%{env: :prod})
      %{"sdk_manifest" => manifest} = JSON.decode!(json)

      for key <- ~w(sdk_version elixir_version otp_release erts_version schedulers runtime_env) do
        assert Map.has_key?(manifest, key), "expected sdk_manifest to contain #{key}"
      end

      assert manifest["elixir_version"] == System.version()
      assert manifest["otp_release"] == to_string(:erlang.system_info(:otp_release))
      assert manifest["erts_version"] == to_string(:erlang.system_info(:version))
      assert is_integer(manifest["schedulers"])
    end

    test "runtime_env reflects config.env" do
      for env <- [:prod, :dev, :staging, :test] do
        json = Fostrom.DeviceAgent.build_sdk_manifest(%{env: env})
        %{"sdk_manifest" => manifest} = JSON.decode!(json)
        assert manifest["runtime_env"] == to_string(env)
      end
    end

    test "omits nil app_name and app_version keys" do
      # In the test runtime no host app declares :fostrom as a dep, so app_name/app_version
      # come back as nil and Map.reject should drop them entirely.
      json = Fostrom.DeviceAgent.build_sdk_manifest(%{env: :test})
      %{"sdk_manifest" => manifest} = JSON.decode!(json)

      refute Map.has_key?(manifest, "app_name")
      refute Map.has_key?(manifest, "app_version")
    end

    test "sdk_version comes from the fostrom application spec" do
      json = Fostrom.DeviceAgent.build_sdk_manifest(%{env: :prod})
      %{"sdk_manifest" => manifest} = JSON.decode!(json)
      expected = Application.spec(:fostrom, :vsn) |> to_string()
      assert manifest["sdk_version"] == expected
    end
  end
end
