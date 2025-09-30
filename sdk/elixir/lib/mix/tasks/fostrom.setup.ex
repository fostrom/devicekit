defmodule Mix.Tasks.Fostrom.Setup do
  @moduledoc """
  Setup Fostrom by downloading the Device Agent.

  The Device Agent is automatically downloaded during initial compilation.
  However, if something goes wrong, you can call this task manually to download the agent.
  """

  @shortdoc "Download Fostrom Device Agent"

  use Mix.Task

  def run(_args) do
    agent_dir = Path.join(Mix.Project.app_path(), "priv/.agent")
    File.rm_rf!(agent_dir)
    File.mkdir_p!(agent_dir)
    System.cmd("sh", ["dl-agent.sh", agent_dir], cd: File.cwd!())
  end
end
