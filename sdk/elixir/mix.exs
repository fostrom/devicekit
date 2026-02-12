defmodule Fostrom.MixProject do
  use Mix.Project

  def project do
    [
      app: :fostrom,
      version: "0.1.0",
      elixir: "~> 1.18",
      start_permanent: Mix.env() == :prod,
      compilers: Mix.compilers() ++ [:download_agent],
      deps: deps(),
      name: "Fostrom",
      homepage_url: "https://fostrom.io",
      description: description(),
      docs: docs(),
      package: package(),
      aliases: aliases()
    ]
  end

  def description do
    "Fostrom is an IoT Cloud Platform built for developers, to easily connect, monitor and control your fleet of devices, from microcontrollers to industrial IoT. The Elixir SDK enables you to get started with Fostrom in just a few lines of code."
  end

  def package do
    [
      name: "fostrom",
      files: ~w(lib .formatter.exs mix.exs dl-agent.sh README.md LICENSE),
      licenses: ["Apache-2.0"],
      links: %{
        "Fostrom" => "https://fostrom.io",
        "SDK Docs" => "https://fostrom.io/docs/sdk/elixir"
      }
    ]
  end

  def docs do
    [
      main: "Fostrom",
      logo: "logo.png",
      favicon: "logo.png",
      formatters: ["html"],
      groups_for_modules: [
        Main: [Fostrom, Fostrom.Handler, Fostrom.Mail],
        "Manual Mailbox Operations": [Fostrom.Mailbox, Fostrom.Mailbox.Status],
        "Device Agent": [Fostrom.DeviceAgent]
      ]
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Fostrom.Application, []}
    ]
  end

  defp aliases do
    [
      test: ["test --no-start"]
    ]
  end

  defp deps do
    [
      {:req, "~> 0.5.15"},

      # --- DEV/TEST DEPS ---
      {:credo, ">= 0.0.0", only: [:dev, :test], runtime: false},
      {:ex_doc, ">= 0.0.0", only: [:dev, :test], runtime: false}
    ]
  end
end
