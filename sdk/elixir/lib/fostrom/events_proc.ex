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
    {:ok, %{events: open_stream(), sse_buffer: ""}}
  end

  def handle_info({{Finch.HTTP1.Pool, _}, _} = msg, %{events: events} = state) do
    with {:ok, chunks} <- Req.parse_message(events, msg) do
      {events_to_process, sse_buffer} =
        for chunk <- chunks, reduce: {[], state.sse_buffer} do
          {acc_events, buffer} ->
            {parsed_events, next_buffer} = process_chunk(chunk, buffer)
            {acc_events ++ parsed_events, next_buffer}
        end

      for event <- events_to_process do
        process_event(event)
      end

      {:noreply, %{state | sse_buffer: sse_buffer}}
    else
      _ ->
        # Attempt to re-open the event stream without stopping the agent.
        new_stream = open_stream()
        {:noreply, %{state | events: new_stream, sse_buffer: ""}}
    end
  end

  defp process_chunk(:done, buffer), do: {[], buffer}

  defp process_chunk({:trailers, _}, buffer), do: {[], buffer}

  defp process_chunk({:data, data}, buffer) do
    {events, sse_buffer} = (buffer <> data) |> split_sse_events()
    parsed_events = events |> Enum.map(&parse_sse_event/1) |> Enum.reject(&is_nil/1)
    {parsed_events, sse_buffer}
  end

  defp process_chunk(_, buffer), do: {[], buffer}

  defp split_sse_events(data) do
    parts = String.split(data, "\n\n", trim: false)

    if String.ends_with?(data, "\n\n") do
      {Enum.reject(parts, &(&1 == "")), ""}
    else
      buffer = List.last(parts) || ""
      complete_events = parts |> Enum.drop(-1) |> Enum.reject(&(&1 == ""))
      {complete_events, buffer}
    end
  end

  defp parse_sse_event(raw_event) do
    empty_str = fn s -> s == "" end

    raw_event
    |> String.trim()
    |> String.split("\n")
    |> Enum.reject(empty_str)
    |> Enum.map(fn s ->
      case String.split(s, ":", parts: 2, trim: true) do
        [k, v] ->
          k = k |> String.trim()
          v = v |> String.trim()

          # Only keep known keys and avoid creating atoms dynamically
          case k do
            "event" -> {"event", v}
            "id" -> {"id", v}
            "data" -> {"data", v}
            _ -> nil
          end

        _ ->
          nil
      end
    end)
    |> Enum.reject(fn x -> is_nil(x) end)
    |> Enum.into(%{})
    |> case do
      m when map_size(m) == 0 -> nil
      m -> m
    end
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
