defmodule Fostrom.Exception do
  @moduledoc """
  A `Fostrom.Exception` contains a reason and a message. It also contains `reconnecting_in` (in milliseconds) when the reconnecting or unauthorized callbacks are called.
  """

  @type t() :: %__MODULE__{
          reason: binary(),
          message: binary(),
          reconnecting_in: nil | non_neg_integer()
        }

  defexception reason: nil, message: nil, reconnecting_in: nil
end

defmodule Fostrom.Mail do
  @moduledoc """
  `%Fostrom.Mail{}` is the struct delivered to your handler, and is returned from `Fostrom.Mailbox.next/0` if the mailbox is not empty.
  """

  @typedoc """
  * `id`: UUIDv7
  * `name`: name of the packet schema
  * `payload`: a map (with string keys)
  * `mailbox_size`: u16 integer
  """
  @type t() :: %__MODULE__{
          id: String.t(),
          name: String.t(),
          payload: map() | nil,
          mailbox_size: non_neg_integer()
        }

  defstruct [:id, :name, :payload, :mailbox_size]
end

defmodule Fostrom.Mailbox.Status do
  @moduledoc """
  Return value of `Fostrom.Mailbox.status/0`.

  This struct is only returned when there is mail in the mailbox. Otherwise `Fostrom.Mailbox.status/0` returns `{:ok, :mailbox_empty}`.
  """

  @typedoc """
  * `mailbox_size`: u16 integer
  * `next_mail_id`: UUIDv7
  * `next_mail_name`: name of the packet schema
  """
  @type t() :: %__MODULE__{
          mailbox_size: non_neg_integer(),
          next_mail_id: String.t(),
          next_mail_name: String.t()
        }

  defstruct [:mailbox_size, :next_mail_id, :next_mail_name]
end

defmodule Fostrom.Mailbox do
  @moduledoc """
  `Fostrom.Mailbox` provides manual access to the Device Mailbox.

  Usually you'll just implement the `Fostrom.Handler` behaviour in your application code,
  and handle incoming mail there. However, if you prefer to handle it manually, you can do so
  using this module.
  """

  @doc """
  Fetch the mailbox size and the next mail's ID and name.
  """
  @spec status() ::
          {:ok, Fostrom.Mailbox.Status.t()}
          | {:ok, :mailbox_empty}
          | {:error, Fostrom.Exception.t()}
  def status, do: Fostrom.Req.mailbox_status()

  @doc """
  Fetch the next available mail from the mailbox.
  """
  @spec next() ::
          {:ok, Fostrom.Mail.t()}
          | {:ok, :mailbox_empty}
          | {:error, Fostrom.Exception.t()}
  def next, do: Fostrom.Req.mailbox_next()

  @doc """
  Acknowledge the Mail.

  You would usually acknowledge when you want to signify that the mail was successfully processed by the device.
  """
  @spec ack(Fostrom.Mail.t()) :: :ok | {:error, Fostrom.Exception.t()}
  def ack(%Fostrom.Mail{id: id}), do: Fostrom.Req.mail_op(:ack, id)

  @doc """
  Reject the Mail.

  You would usually reject when you want to signify that the mail was not successfully processed by the device.
  """
  @spec reject(Fostrom.Mail.t()) :: :ok | {:error, Fostrom.Exception.t()}
  def reject(%Fostrom.Mail{id: id}), do: Fostrom.Req.mail_op(:reject, id)

  @doc """
  Requeue the Mail.

  Requeueing moves the mail to the end of the queue and allows you to process the next mail.
  """
  @spec requeue(Fostrom.Mail.t()) :: :ok | {:error, Fostrom.Exception.t()}
  def requeue(%Fostrom.Mail{id: id}), do: Fostrom.Req.mail_op(:requeue, id)
end

defmodule Fostrom do
  @external_resource "README.md"
  @moduledoc File.read!("README.md") |> String.split("\n") |> Enum.drop(4) |> Enum.join("\n")

  @doc """
  Send a Datapoint to Fostrom

  Takes the packet schema name and a payload. Payload must be a map, and is required to send datapoints.

  > #### Note {: .info}
  >
  > Ensure the packet schema exists and is for a **datapoint**.
  """
  @spec send_datapoint(String.t(), map() | nil) :: :ok | {:error, Fostrom.Exception.t()}
  def send_datapoint(name, payload)
      when (is_binary(name) or is_atom(name)) and is_map(payload) and map_size(payload) > 0 do
    Fostrom.Req.send_pulse(:datapoint, name, payload)
  end

  @doc """
  Send a Message to Fostrom

  Takes the packet schema name and a payload. Payload must either be a map or nil.

  > #### Note {: .info}
  >
  > Ensure the packet schema exists and is for a **message**.
  """
  @spec send_msg(String.t(), map() | nil) :: :ok | {:error, Fostrom.Exception.t()}
  def send_msg(name, payload)
      when (is_binary(name) or is_atom(name)) and (is_map(payload) or is_nil(payload)) do
    Fostrom.Req.send_pulse(:msg, name, payload)
  end
end
