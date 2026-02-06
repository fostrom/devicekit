defmodule Fostrom.HandlerProc do
  @moduledoc false
  use GenServer

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  def init(opts) do
    handler = Access.fetch!(opts, :handler)
    Application.ensure_loaded(Fostrom.Handler)
    Application.ensure_loaded(handler)

    is_handler? =
      handler.__info__(:attributes)
      |> Keyword.get(:behaviour, [])
      |> Enum.member?(Fostrom.Handler)

    if is_handler? do
      {:ok, handler}
    else
      raise "[Fostrom] The handler module `#{handler}` is not valid. `use Fostrom.Handler` on top of the module and implement the required callbacks."
    end
  end

  def handle_cast(:connected, handler) do
    handler.connected()
    {:noreply, handler}
  end

  def handle_cast({:connect_failed, exception}, handler) do
    handler.reconnecting(exception)
    {:noreply, handler}
  end

  def handle_cast({:unauthorized, exception}, handler) do
    handler.unauthorized(exception)
    {:noreply, handler}
  end

  def handle_cast({:handle_mail, mail}, handler) do
    call_handle_mail(handler, mail) |> process_handler_resp(mail)
    {:noreply, handler}
  end

  defp call_handle_mail(handler, mail) do
    handler.handle_mail(mail)
  rescue
    _ -> :reject
  catch
    _ -> :reject
  end

  defp process_handler_resp(resp, mail) do
    case resp do
      :ack -> Fostrom.Mailbox.ack(mail)
      :reject -> Fostrom.Mailbox.reject(mail)
      :requeue -> Fostrom.Mailbox.requeue(mail)
      :noop -> nil
      _ -> Fostrom.Mailbox.reject(mail)
    end
  end
end
