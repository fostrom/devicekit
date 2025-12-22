# syntax=docker/dockerfile:1
FROM alpine:latest

WORKDIR /devicekit
ENV BUN_INSTALL="$HOME/.bun"
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH="${CARGO_HOME}/bin:${BUN_INSTALL}/bin:/root/.local/bin:${PATH}"

RUN apk add --no-cache build-base bash curl ca-certificates just uv zig nodejs elixir rustup
RUN curl -fsSL https://bun.com/install | bash
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

COPY . .
RUN sed -i '/^set quiet$/d' justfile
RUN just -v setup
RUN just -v build
