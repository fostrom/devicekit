defmodule Fostrom.Req do
  @moduledoc false

  defp get_header(%Req.Response{} = resp, header) when is_binary(header) or is_atom(header) do
    Req.Response.get_header(resp, to_string(header)) |> List.first()
  end

  defp parse_bool("true"), do: true
  defp parse_bool("1"), do: true
  defp parse_bool("yes"), do: true
  defp parse_bool(_), do: false

  defp req(url \\ "/", method \\ :get, body \\ nil) do
    %{fleet_id: fleet_id, device_id: device_id} = Fostrom.DeviceAgent.read_config()

    Req.new(
      headers: [
        {"Content-Type", "application/json"},
        {"X-Fleet-ID", fleet_id},
        {"X-Device-ID", device_id}
      ],
      base_url: "http://localhost/",
      unix_socket: "/tmp/fostrom/agent.sock",
      method: method,
      url: url,
      body: body
    )
    |> Req.run()
    |> elem(1)
    |> case do
      %Req.Response{status: 200} = resp ->
        {:ok, resp}

      %Req.Response{status: status, body: body}
      when status in [400, 401, 403, 404, 408, 500, 505] ->
        {reason, message} =
          case body do
            %{"error" => e, "msg" => msg} -> {e, msg}
            %{"error" => e} -> {e, "Request failed"}
            %{"msg" => msg} -> {"request_failed", msg}
            _ -> {"request_failed", "Communicating with the Device Agent failed"}
          end

        {:error, %Fostrom.Exception{reason: reason, message: message}}

      _ ->
        {:error,
         %Fostrom.Exception{
           reason: "request_failed",
           message: "Communicating with the Device Agent failed"
         }}
    end
  end

  def agent_status do
    with {:ok, resp} <- req() do
      resp
    end
  end

  def mailbox_status do
    with {:ok, resp} <- req("/mailbox/next", :head) do
      empty? = get_header(resp, "x-mailbox-empty") |> parse_bool()

      if empty? do
        {:ok, :mailbox_empty}
      else
        {:ok,
         %Fostrom.Mailbox.Status{
           mailbox_size: get_header(resp, "x-mailbox-size") |> String.to_integer(),
           next_mail_id: get_header(resp, "x-mail-id") |> String.to_integer(),
           next_mail_name: get_header(resp, "x-mail-name")
         }}
      end
    end
  end

  def mailbox_next do
    with {:ok, resp} <- req("/mailbox/next") do
      empty? = get_header(resp, "x-mailbox-empty") |> parse_bool()

      if empty? do
        {:ok, :mailbox_empty}
      else
        payload =
          if get_header(resp, "x-mail-has-payload") |> parse_bool() do
            resp.body
          else
            nil
          end

        {:ok,
         %Fostrom.Mail{
           mailbox_size: get_header(resp, "x-mailbox-size") |> String.to_integer(),
           id: get_header(resp, "x-mail-id") |> String.to_integer(),
           name: get_header(resp, "x-mail-name"),
           payload: payload
         }}
      end
    end
  end

  def mail_op(op, mail_id) when op in [:ack, :reject, :requeue] do
    with {:ok, resp} <- req("/mailbox/#{to_string(op)}/#{mail_id}", :put) do
      # If more mail is available, fetch the next mail and process it
      if get_header(resp, "x-mail-available") |> parse_bool() do
        case Fostrom.Req.mailbox_next() do
          {:ok, :mailbox_empty} -> nil
          {:ok, mail} -> GenServer.cast(Fostrom.HandlerProc, {:handle_mail, mail})
          _ -> nil
        end
      end

      :ok
    end
  end

  def send_pulse(type, name, payload) when type in [:msg, :datapoint] do
    with {:ok, _resp} <- req("/pulse/#{type}/#{name}", :post, payload) do
      :ok
    end
  end
end
