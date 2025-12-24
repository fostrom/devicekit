# syntax=docker/dockerfile:1
FROM elixir:slim

WORKDIR /devicekit

ENV DEBIAN_FRONTEND=noninteractive
ENV HOME=/root
ENV BUN_INSTALL="${HOME}/.bun"
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH="${CARGO_HOME}/bin:${BUN_INSTALL}/bin:${HOME}/.local/bin:${PATH}"

RUN apt-get update
RUN apt-get install -y --no-install-recommends build-essential curl ca-certificates unzip just

RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash -
RUN apt-get install -y nodejs

RUN curl -LsSf https://astral.sh/uv/install.sh | sh
RUN curl -fsSL https://bun.com/install | bash
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

ARG ZIG_VERSION=0.15.2
RUN set -eux; \
    arch="$(dpkg --print-architecture)"; \
    case "$arch" in \
    amd64) zig_arch="x86_64-linux" ;; \
    arm64) zig_arch="aarch64-linux" ;; \
    *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; \
    esac; \
    url="https://ziglang.org/download/${ZIG_VERSION}/zig-${zig_arch}-${ZIG_VERSION}.tar.xz"; \
    curl -fsSL "$url" -o /tmp/zig.tar.xz; \
    tar -C /opt -xf /tmp/zig.tar.xz; \
    ln -sf "/opt/zig-${zig_arch}-${ZIG_VERSION}/zig" /usr/local/bin/zig; \
    rm -f /tmp/zig.tar.xz

COPY . .
RUN sed -i '/^set quiet$/d' justfile
RUN just -v setup
RUN just -v build
