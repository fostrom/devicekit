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
end
