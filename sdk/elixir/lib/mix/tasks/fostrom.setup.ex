defmodule Mix.Tasks.Fostrom.Setup do
  @moduledoc """
  Setup Fostrom by downloading the Device Agent.

  The Device Agent is automatically downloaded during initial compilation.
  However, if something goes wrong, you can call this task manually to download the agent.
  """

  @shortdoc "Download Fostrom Device Agent"

  use Mix.Task

  def run(_args) do
    Mix.Project.app_path() |> Path.join("priv/.agent") |> File.rm_rf!()
    Mix.Task.reenable("compile.download_agent")
    Mix.Task.run("compile.download_agent")
  end
end
