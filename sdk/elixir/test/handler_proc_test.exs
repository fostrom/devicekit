defmodule Fostrom.HandlerProcTest do
  use ExUnit.Case
  @moduletag capture_log: true

  defmodule InvalidReturnHandler do
    use Fostrom.Handler

    @impl Fostrom.Handler
    def handle_mail(_mail), do: :invalid_return
  end

  test "invalid handle_mail return is treated as reject without crashing" do
    {:ok, _apps} = Application.ensure_all_started(:req)

    Application.put_env(:fostrom, :config,
      fleet_id: "FOSTROM0",
      device_id: "SANDBOX001",
      device_secret: "FOS-TESTFLIGHTCONNFOSTROM0SANDBOX001",
      env: :test
    )

    {:ok, pid} = Fostrom.HandlerProc.start_link(handler: InvalidReturnHandler)

    on_exit(fn ->
      if Process.alive?(pid), do: Process.exit(pid, :kill)
      Application.delete_env(:fostrom, :config)
    end)

    mail = %Fostrom.Mail{
      id: "018f4d1f-2a55-7f0e-b4ea-5f7c3e6fa001",
      name: "test",
      payload: %{"ok" => true},
      mailbox_size: 1
    }

    GenServer.cast(pid, {:handle_mail, mail})
    Process.sleep(50)

    assert Process.alive?(pid)
  end
end
