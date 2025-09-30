defmodule Mix.Tasks.Compile.DownloadAgent do
  @moduledoc false
  @shortdoc "Downloads Fostrom Device Agent at Compile-time"

  use Mix.Task.Compiler
  @recursive true

  def clean() do
    File.rm_rf!(Path.join(Mix.Project.app_path(), "priv/.agent"))
  end

  def run(_args) do
    agent_dir = Path.join(Mix.Project.app_path(), "priv/.agent")

    if File.exists?(agent_dir <> "/fostrom-device-agent") do
      {:ok, []}
    else
      Mix.shell().info("Downloading agent...")
      File.mkdir_p!(agent_dir)
      download_agent(agent_dir)
      {:ok, []}
    end
  end

  defp download_agent(target_dir) do
    System.cmd("sh", ["dl-agent.sh", target_dir], cd: File.cwd!())
  end
end
