defmodule Fostrom.EventsProc do
  @moduledoc false
  use GenServer

  defp open_stream do
    %{fleet_id: fleet_id, device_id: device_id} = Fostrom.DeviceAgent.read_config()

    Req.get!(
      url: "http://localhost/events",
      unix_socket: "/tmp/fostrom/agent.sock",
      headers: [
        {"Accept", "text/event-stream"},
        {"X-Fleet-ID", fleet_id},
        {"X-Device-ID", device_id}
      ],
      into: :self,
      receive_timeout: :infinity
    )
  end

  def start_link(_) do
    GenServer.start_link(__MODULE__, [], name: __MODULE__)
  end

  def init(_args) do
    {:ok, %{events: open_stream()}}
  end

  def handle_info({{Finch.HTTP1.Pool, _}, _} = msg, %{events: events} = state) do
    with {:ok, chunks} <- Req.parse_message(events, msg) do
      events =
        for chunk <- chunks do
          process_chunk(chunk)
        end
        |> List.flatten()
        |> Enum.reject(&is_nil/1)

      for event <- events do
        process_event(event)
      end

      {:noreply, state}
    else
      _ ->
        # Attempt to re-open the event stream without stopping the agent.
        new_stream = open_stream()
        {:noreply, %{state | events: new_stream}}
    end
  end

  def terminate(_reason, _state) do
    if conf = Application.get_env(:fostrom, :config) do
      if conf[:stop_agent_on_terminate] do
        Fostrom.DeviceAgent.stop()
      end
    end

    :ok
  end

  defp process_chunk(:done), do: nil

  defp process_chunk({:trailers, _}), do: nil

  defp process_chunk({:data, data}) do
    empty_str = fn s -> s == "" end

    data
    |> String.split("\n\n")
    |> Enum.reject(empty_str)
    |> Enum.map(
      &(&1
        |> String.trim()
        |> String.split("\n")
        |> Enum.reject(empty_str)
        |> Enum.map(fn s ->
          [k, v] = String.split(s, ":", parts: 2, trim: true)
          k = k |> String.trim()
          v = v |> String.trim()

          # Only keep known keys and avoid creating atoms dynamically
          case k do
            "event" -> {"event", v}
            "id" -> {"id", v}
            "data" -> {"data", v}
            _ -> nil
          end
        end)
        |> Enum.reject(fn x -> is_nil(x) end)
        |> Enum.into(%{}))
    )
  end

  defp process_event(%{"event" => "connected"}) do
    GenServer.cast(Fostrom.HandlerProc, :connected)
  end

  defp process_event(%{"event" => "disconnected", "data" => data}) do
    %{"error" => error, "reconnecting_in_ms" => reconnecting_in} = JSON.decode!(data)
    [reason | rest] = String.split(error, ":", parts: 2, trim: true)
    reason = String.trim(reason)
    msg = rest |> List.first() |> to_string() |> String.trim()

    e = %Fostrom.Exception{
      reason: reason,
      message: msg,
      reconnecting_in: reconnecting_in
    }

    case reason do
      "unauthorized" -> GenServer.cast(Fostrom.HandlerProc, {:unauthorized, e})
      _ -> GenServer.cast(Fostrom.HandlerProc, {:connect_failed, e})
    end
  end

  defp process_event(%{"event" => "new_mail"}) do
    case Fostrom.Req.mailbox_next() do
      {:ok, :mailbox_empty} -> nil
      {:ok, mail} -> GenServer.cast(Fostrom.HandlerProc, {:handle_mail, mail})
      _ -> nil
    end
  end

  defp process_event(_), do: nil
end
