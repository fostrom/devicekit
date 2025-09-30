defmodule Fostrom.Handler do
  @moduledoc """
  Define a Handler module to handle incoming mail, and other events such as `connected`, `reconnecting`, and `unauthorized`.

  The easiest way to define a handler is to `use Fostrom.Handler`:

  ```elixir
  defmodule MyApp.FostromHandler do
    use Fostrom.Handler

    # handle_mail needs to return either :ack, :reject, or :requeue
    def handle_mail(mail) do
      # process the mail here
      :ack
    end
  end
  ```

  Note that you need to pass the handler module in the config:

  ```elixir
  # Add in config/config.exs or config/runtime.exs:
  config :fostrom, :config,
    fleet_id: "<fleet-id>",
    device_id: "<device-id>",
    device_secret: "<device-secret>",
    env: Config.config_env(),
    handler: MyApp.FostromHandler
  ```

  Here is a complete implementation:

  ```elixir
  defmodule MyApp.FostromHandler do
    @moduledoc false
    use Fostrom.Handler

    def reconnecting(error) do
      %{reason: reason, message: message, reconnecting_in: reconnecting_in} = error
    end

    def unauthorized(error) do
      %{reason: reason, message: message, reconnecting_in: reconnecting_in} = error
    end

    def connected do
      :ok
    end

    # Return :ack | :reject | :requeue from this function
    def handle_mail(%Fostrom.Mail{} = mail) do
      %{id: id, name: name, payload: payload, mailbox_size: mailbox_size} = mail
      :ack
    end
  end
  ```

  """

  @doc """
  Called when connection is established or re-established to Fostrom.
  """
  @callback connected() :: any()

  @doc """
  Called when connection to Fostrom is lost, and the Device Agent is going to retry connecting after sometime.
  """
  @callback reconnecting(Fostrom.Exception.t()) :: any()

  @doc """
  Called when the device is not authorized to connect to Fostrom. This could be if the device is disabled, the secret is incorrect, or the device does not exist.
  """
  @callback unauthorized(Fostrom.Exception.t()) :: any()

  @doc """
  Handle a Mail sent by Fostrom to this device.

  Check out the `Fostrom.Mail` struct for the structure of the mail. You need to return either `:ack`, `:reject`, or `:requeue` from the handler. If your handler function returns anything else, or raises, or throws, `:reject` will be assumed.

  You can return `:noop` from the handler, and then call the `Fostrom.Mailbox` functions to manually acknowledge the mail. Please note that the Device Mailbox is sequential, so returning `:noop` means you'll still get the same mail again if you call `Fostrom.Mailbox.next()`.
  """
  @callback handle_mail(Fostrom.Mail.t()) :: :ack | :reject | :requeue | :noop

  defmacro __using__(_) do
    quote do
      require Logger
      @behaviour Fostrom.Handler

      @impl Fostrom.Handler
      def reconnecting(error) do
        Logger.error(
          "[Fostrom] Failed to connect (#{error.reason}): #{error.message}. Reconnecting in #{error.reconnecting_in / 1000} seconds..."
        )
      end

      @impl Fostrom.Handler
      def unauthorized(error) do
        Logger.critical(
          "[Fostrom] Unauthorized (#{error.reason}): #{error.message}. Reconnecting in #{error.reconnecting_in / 1000} seconds..."
        )
      end

      @impl Fostrom.Handler
      def connected do
        Logger.info("[Fostrom] Connected")
      end

      @impl Fostrom.Handler
      def handle_mail(%Fostrom.Mail{} = mail) do
        Logger.warning("""
        [Fostrom] Received Mail [#{mail.name} -> ID #{mail.id}] (Mailbox Size: #{mail.mailbox_size})
        Warning: Auto-Acknowledging Mail. Refer docs to define a handler module.
        """)

        :ack
      end
    end
  end
end

defmodule Fostrom.DefaultHandler do
  @moduledoc false
  use Fostrom.Handler
end
