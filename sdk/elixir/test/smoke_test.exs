defmodule Fostrom.SmokeTests do
  use ExUnit.Case
  @moduletag capture_log: true

  test "ensure version is same in readme and mix.exs" do
    mix_vsn = Fostrom.MixProject.project() |> Keyword.fetch!(:version)

    readme_dep =
      File.read!("README.md")
      |> String.split("def deps do", parts: 2)
      |> List.last()
      |> String.split("end", parts: 2)
      |> List.first()
      |> String.split("[", parts: 2)
      |> List.last()
      |> String.split("]", parts: 2)
      |> List.first()
      |> String.replace("\n", "")
      |> String.trim()

    assert readme_dep == "{:fostrom, \"~> #{mix_vsn}\"}"
  end

  test "ensure fostrom supervisor has not started" do
    assert GenServer.whereis(Fostrom.Supervisor) == nil
  end

  test "start application and ensure device agent starts" do
    fleet_id = "FOSTROM0"
    device_id = "SANDBOX001"
    device_secret = "FOS-TESTFLIGHTCONNFOSTROM0SANDBOX001"

    agent_path = to_string(:code.priv_dir(:fostrom)) <> "/.agent/fostrom-device-agent"
    {agent_version, 0} = System.cmd(agent_path, ["version"])
    agent_version = String.trim(agent_version)

    Application.put_all_env(
      fostrom: [
        config: [
          fleet_id: fleet_id,
          device_id: device_id,
          device_secret: device_secret,
          env: :dev,
          stop_agent_on_terminate: true
        ]
      ]
    )

    Application.ensure_all_started(:fostrom)
    assert is_pid(GenServer.whereis(Fostrom.Supervisor))

    %Req.Response{} = resp = Req.get!("http://localhost/", unix_socket: "/tmp/fostrom/agent.sock")
    assert resp.status == 200
    assert resp.headers["x-powered-by"] == ["Fostrom"]
    assert resp.headers["x-protocol"] == ["Moonlight"]
    assert resp.headers["x-protocol-version"] == ["1"]
    assert resp.headers["x-api-version"] == ["1"]
    assert resp.headers["x-agent-version"] == [String.replace(agent_version, "v", "")]
    assert resp.headers["server"] == ["Fostrom-Device-Agent/#{agent_version}"]
    assert resp.headers["x-fleet-id"] == [fleet_id]
    assert resp.headers["x-device-id"] == [device_id]

    assert is_map(resp.body)

    :ok = Application.stop(:fostrom)
    false = File.exists?("/tmp/fostrom/agent.sock")
  end
end
