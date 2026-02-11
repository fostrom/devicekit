defmodule Mix.Tasks.Compile.DownloadAgent do
  @moduledoc false
  @shortdoc "Downloads Fostrom Device Agent at Compile-time"

  use Mix.Task.Compiler
  @recursive true

  def clean do
    Mix.Project.app_path() |> Path.join("priv/.agent") |> File.rm_rf!()
  end

  def run(_args) do
    script_path = Mix.Project.project_file() |> Path.dirname() |> Path.join("dl-agent.sh")
    agent_dir = Mix.Project.app_path() |> Path.join("priv/.agent")
    File.mkdir_p!(agent_dir)
    args = [script_path, agent_dir]
    {_, status} = System.cmd("sh", args, stderr_to_stdout: true, into: IO.stream(:stdio, :line))
    if status != 0, do: Mix.raise("Failed to download Device Agent")
    {:ok, []}
  end
end
