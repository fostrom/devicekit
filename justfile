# Fostrom DeviceKit Justfile

set ignore-comments
set quiet

# Rust Build Targets
ARM_LINUX := "aarch64-unknown-linux-musl"
ARMV6HF_LINUX := "arm-unknown-linux-musleabihf"
RISCV_LINUX := "riscv64gc-unknown-linux-musl"
AMD_LINUX := "x86_64-unknown-linux-musl"
ARM_MAC := "aarch64-apple-darwin"
AMD_MAC := "x86_64-apple-darwin"
# ARM_FREEBSD  := "aarch64-unknown-freebsd"
AMD_FREEBSD  := "x86_64-unknown-freebsd"

PYTHON_VERSION := "3.10"
export UV_PYTHON := PYTHON_VERSION

QUIET := if env("GITHUB_ACTIONS", "false") == "true" { "" } else { "-q" }

OS := if os() == "linux" {
    "linux"
} else if os() == "macos" {
    "macos"
} else if os() == "freebsd" {
    "freebsd"
} else {
    error("unsupported build architecture")
}

ARCH := if arch() == "aarch64" {
    "arm64"
} else if arch() == "x86_64" {
    "amd64"
} else if arch() == "riscv64" {
    "riscv64"
} else if arch() == "arm" {
    "armv6hf"
} else {
    error("unsupported build os")
}

BIN := "fostrom-device-agent"
BIN_OS_ARCH := BIN + "-" + OS + "-" + ARCH


# just build
default: build



# -----------------------------
# --- BUILD + VERIFY + TEST ---
# -----------------------------

# BUILD + VERIFY + TEST
[group("build")]
build:
    just verify-version-coherence
    just build-device-agent
    just copy-device-agent-to-sdk-js
    just copy-device-agent-to-sdk-python
    just copy-device-agent-to-sdk-elixir
    just build-sdk-elixir
    just build-sdk-python
    just build-sdk-js


# BUILD + VERIFY + TEST ON FREEBSD
[group("build")]
build-on-freebsd:
    just verify-version-coherence
    just build-device-agent
    just copy-device-agent-to-sdk-js
    just copy-device-agent-to-sdk-python
    just copy-device-agent-to-sdk-elixir
    just build-sdk-js-freebsd
    just build-sdk-python-freebsd


# BUILD + VERIFY + TEST + CROSS COMPILE
[group("build")]
release:
    just build
    just test-extensive
    just cross-compile-device-agent


# BUILD DEVICE AGENT FOR CURRENT OS/ARCH
[private]
[group("build")]
[working-directory("device-agent/")]
build-device-agent:
    cargo build --release {{QUIET}}
    cargo test --release {{QUIET}}
    cargo run {{QUIET}} --release -- stop > /dev/null

    rm -rf .release
    mkdir -p .release
    install -m 0755 "target/release/{{BIN}}" ".release/{{BIN_OS_ARCH}}"


# CROSS COMPILE DEVICE AGENT FOR ALL TARGETS
[private]
[group("build")]
[working-directory("device-agent/")]
cross-compile-device-agent:
    rm -rf .release
    mkdir -p .release

    echo -n "compiling {{ARM_LINUX}}      "
    cargo zigbuild --release --target {{ARM_LINUX}}
    echo -n "compiling {{ARMV6HF_LINUX}}    "
    cargo zigbuild --release --target {{ARMV6HF_LINUX}}
    echo -n "compiling {{AMD_LINUX}}       "
    cargo zigbuild --release --target {{AMD_LINUX}}
    echo -n "compiling {{RISCV_LINUX}}    "
    cargo zigbuild --release --target {{RISCV_LINUX}}
    echo -n "compiling {{ARM_MAC}}            "
    cargo zigbuild --release --target {{ARM_MAC}}
    echo -n "compiling {{AMD_MAC}}             "
    cargo zigbuild --release --target {{AMD_MAC}}
    echo -n "compiling {{AMD_FREEBSD}}         "
    cargo zigbuild --release --target {{AMD_FREEBSD}}

    install -m 0755 "target/{{ARM_LINUX}}/release/{{BIN}}" ".release/{{BIN}}-linux-arm64"
    install -m 0755 "target/{{ARMV6HF_LINUX}}/release/{{BIN}}" ".release/{{BIN}}-linux-armv6hf"
    install -m 0755 "target/{{RISCV_LINUX}}/release/{{BIN}}" ".release/{{BIN}}-linux-riscv64"
    install -m 0755 "target/{{AMD_LINUX}}/release/{{BIN}}" ".release/{{BIN}}-linux-amd64"
    install -m 0755 "target/{{ARM_MAC}}/release/{{BIN}}" ".release/{{BIN}}-macos-arm64"
    install -m 0755 "target/{{AMD_MAC}}/release/{{BIN}}" ".release/{{BIN}}-macos-amd64"
    install -m 0755 "target/{{AMD_FREEBSD}}/release/{{BIN}}" ".release/{{BIN}}-freebsd-amd64"

    just codesign-mac-binaries

    ".release/{{BIN_OS_ARCH}}" version > ".release/{{BIN}}.vsn"
    cd .release && sha256sum {{BIN}}* > "{{BIN}}.sha256"
    cd .release && sha256sum -c --quiet "{{BIN}}.sha256"


# CODESIGN MACOS BINARIES
[private]
[group("build")]
[working-directory("device-agent/")]
codesign-mac-binaries:
    #!/bin/bash
    CODESIGN_WARNED=""
    codesign_macos_binary() {
      local file="$1"
      if [[ ! -f "$file" ]]; then return 0; fi
      if [[ "$(uname)" == "Darwin" ]]; then
        codesign -s - "$file"
      elif command -v mac >/dev/null 2>&1; then
        mac codesign -s - "$file"
      else
        if [[ -z "$CODESIGN_WARNED" ]]; then
          echo "Warning: codesigning is not available on Linux; binaries will remain unsigned." >&2
          CODESIGN_WARNED=1
        fi
      fi
    }

    codesign_macos_binary ".release/{{BIN}}-macos-arm64"
    codesign_macos_binary ".release/{{BIN}}-macos-amd64"


# COPY DEVICE AGENT TO JS SDK
[private]
[group("build")]
[working-directory("sdk/js/")]
copy-device-agent-to-sdk-js:
    @rm -rf .agent/
    @mkdir -p .agent/
    @install -m 0755 ../../device-agent/.release/{{BIN_OS_ARCH}} .agent/{{BIN_OS_ARCH}}
    @cd .agent && ln -s {{BIN_OS_ARCH}} {{BIN}}


# COPY DEVICE AGENT TO PYTHON SDK
[private]
[group("build")]
[working-directory("sdk/python/src/fostrom/")]
copy-device-agent-to-sdk-python:
    @rm -rf .agent/
    @mkdir -p .agent/
    @install -m 0755 ../../../../device-agent/.release/{{BIN_OS_ARCH}} .agent/{{BIN_OS_ARCH}}
    @cd .agent && ln -s {{BIN_OS_ARCH}} {{BIN}}


# COPY DEVICE AGENT TO ELIXIR SDK
[private]
[group("build")]
[working-directory("sdk/elixir/")]
copy-device-agent-to-sdk-elixir:
    rm -rf _build/dev/lib/fostrom/priv/.agent/
    rm -rf _build/test/lib/fostrom/priv/.agent/
    rm -rf _build/prod/lib/fostrom/priv/.agent/

    rm -rf priv/.agent/
    mkdir -p priv/.agent/
    install -m 0755 ../../device-agent/.release/{{BIN_OS_ARCH}} priv/.agent/{{BIN_OS_ARCH}}
    cd priv/.agent && ln -s {{BIN_OS_ARCH}} {{BIN}}

    # Copy to all build environments if they exist and priv/ is not a symlink
    [ -d _build/dev/lib/fostrom/priv ] && [ ! -L _build/dev/lib/fostrom/priv ] && cp -r priv/.agent _build/dev/lib/fostrom/priv/.agent || true
    [ -d _build/test/lib/fostrom/priv ] && [ ! -L _build/test/lib/fostrom/priv ] && cp -r priv/.agent _build/test/lib/fostrom/priv/.agent || true
    [ -d _build/prod/lib/fostrom/priv ] && [ ! -L _build/prod/lib/fostrom/priv ] && cp -r priv/.agent _build/prod/lib/fostrom/priv/.agent || true


# BUILD ELIXIR SDK
[private]
[group("build")]
[working-directory("sdk/elixir/")]
build-sdk-elixir:
    [ ! -d "deps" ] && mix deps.get || true
    mix compile
    echo "{{BLUE}}Running Elixir Tests...{{NORMAL}}"
    mix test


# BUILD PYTHON SDK
[private]
[group("build")]
[working-directory("sdk/python/")]
build-sdk-python:
    uvx ruff check -s .
    uvx ty check {{QUIET}} .
    rm -rf dist/
    uv build {{QUIET}}
    just verify-sdk-python-build
    echo
    echo "{{BLUE}}Running Python Tests...{{NORMAL}}"
    uvx --with 'fostrom @ .' pytest {{QUIET}}


# BUILD PYTHON SDK ON FREEBSD
[private]
[group("build")]
[working-directory("sdk/python/")]
build-sdk-python-freebsd:
    rm -rf dist/
    uv build {{QUIET}}
    just verify-sdk-python-build
    uvx --with 'fostrom @ .' pytest {{QUIET}}


# BUILD JS SDK
[private]
[group("build")]
[working-directory("sdk/js/")]
build-sdk-js:
    echo
    echo "{{BLUE}}Running JS Tests...{{NORMAL}}"
    if [ "{{QUIET}}"  = -q ]; then node --test --test-reporter=dot; else node --test; fi
    if [ "{{QUIET}}"  = -q ]; then bun test >/dev/null 2>&1; else bun test; fi


# BUILD JS SDK ON FREEBSD
[private]
[group("build")]
[working-directory("sdk/js/")]
build-sdk-js-freebsd:
    echo
    echo "{{BLUE}}Running JS Tests (Node-only)...{{NORMAL}}"
    if [ "{{QUIET}}"  = -q ]; then node --test --test-reporter=dot; else node --test; fi


# VERIFY PYTHON SDK PACKAGE CONTENTS
[private]
[group("build")]
[working-directory("sdk/python/")]
verify-sdk-python-build:
    #!/bin/bash
    set -euo pipefail
    wheel_list="$(unzip -l dist/*.whl)"
    src_list="$(tar -tf dist/*.tar.gz)"

    if grep -q '\.agent' <<<"$wheel_list"; then
      printf "Error: .agent found in wheel!\n" >&2
      exit 1
    fi
    if ! grep -q 'dl-agent\.sh' <<<"$wheel_list"; then
      printf "Error: dl-agent.sh not found in wheel!\n" >&2
      exit 1
    fi

    if grep -q '\.agent' <<<"$src_list"; then
      printf "Error: .agent found in source!\n" >&2
      exit 1
    fi
    if ! grep -q 'dl-agent\.sh' <<<"$src_list"; then
      printf "Error: dl-agent.sh not found in source!\n" >&2
      exit 1
    fi


# -------------------------
# --- EXTENSIVE TESTING ---
# -------------------------

[private]
[group("test-extensive")]
test-extensive:
    just test-extensive-sdk-python


# TEST PYTHON SDK WITH ALL SUPPORTED PYTHON VERSIONS
[private]
[group("test-extensive")]
[working-directory("sdk/python/")]
test-extensive-sdk-python:
    # Based on: https://simonwillison.net/2025/Oct/9/uv-test
    uvx --python 3.10 --isolated --with 'fostrom @ .' pytest
    uvx --python 3.11 --isolated --with 'fostrom @ .' pytest
    uvx --python 3.12 --isolated --with 'fostrom @ .' pytest
    uvx --python 3.13 --isolated --with 'fostrom @ .' pytest
    uvx --python 3.14 --isolated --with 'fostrom @ .' pytest
    uvx --python 3.15 --isolated --with 'fostrom @ .' pytest


# -----------
# --- DEV ---
# -----------

# GENERATE AND OPEN DEVICE AGENT CODE COVERAGE
[group("dev")]
[working-directory("device-agent/")]
cover-device-agent:
    cargo llvm-cov --json | llvm-cov-pretty --open



# -------------
# --- SETUP ---
# -------------


# SETUP RUST AND PYTHON ENVIRONMENTS
[group("setup")]
setup:
    just setup-rust
    just setup-python
    just setup-elixir

# SETUP RUST ENVIRONMENT
[private]
[group("setup")]
[working-directory("device-agent/")]
setup-rust:
    rustup default stable
    cargo install --locked cargo-bump {{QUIET}}
    cargo install --locked cargo-zigbuild {{QUIET}}
    rustup target add {{ARM_LINUX}} {{ARMV6HF_LINUX}} \
      {{AMD_LINUX}} {{RISCV_LINUX}} \
      {{ARM_MAC}} {{AMD_MAC}} \
      {{AMD_FREEBSD}}


# SETUP PYTHON ENVIRONMENT
[private]
[group("setup")]
[working-directory("sdk/python/")]
setup-python:
    uv python install {{PYTHON_VERSION}}


# SETUP ELIXIR ENVIRONMENT
[private]
[group("setup")]
[working-directory("sdk/elixir/")]
setup-elixir:
    mix deps.get
    mix deps.compile



# --------------------
# --- VERSION BUMP ---
# --------------------


# VERSION BUMP ALL
[group("version-bump")]
version-bump-all LEVEL="patch":
    just version-bump-device-agent "{{LEVEL}}"
    just version-bump-dl-agent-script
    just version-bump-sdk-python "{{LEVEL}}"
    just version-bump-sdk-js "{{LEVEL}}"
    just version-bump-sdk-elixir "{{LEVEL}}"
    just verify-version-coherence


# VERSION BUMP FOR DEVICE AGENT
[group("version-bump")]
[working-directory("device-agent/")]
version-bump-device-agent LEVEL="patch":
    cargo bump "{{LEVEL}}"
    just build-device-agent


[group("version-bump")]
[working-directory("sdk/")]
version-bump-dl-agent-script:
    #!/bin/bash
    set -euo pipefail

    SCRIPT="dl-agent.sh"
    AGENT="../device-agent/.release/{{BIN_OS_ARCH}}"

    if [[ ! -x "$AGENT" ]]; then
        echo "Built device agent not found or not executable: $AGENT" >&2
        echo "Run 'just build-device-agent' or 'just release' first." >&2
        exit 1
    fi

    VSN="$("$AGENT" version | tr -d '[:space:]')"
    if [[ "$VSN" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        VSN="v$VSN"
    fi
    if [[ ! "$VSN" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Unexpected version output from '$AGENT version': '$VSN'" >&2
        exit 1
    fi

    VSN="$VSN" perl -0pi -e 'our $n=0; $n += s/^(?:qq\{)?VERSION="v\d+\.\d+\.\d+"(?:\})?$/VERSION="$ENV{VSN}"/mg; END { die "VERSION line not updated\n" if $n != 1 }' "$SCRIPT"
    echo "Updated $SCRIPT VERSION to $VSN"


# VERSION BUMP FOR PYTHON SDK
[group("version-bump")]
[working-directory("sdk/python/")]
version-bump-sdk-python LEVEL="patch":
    uv version --bump "{{LEVEL}}"


# VERSION BUMP FOR JS SDK
[group("version-bump")]
[working-directory("sdk/js/")]
version-bump-sdk-js LEVEL="patch":
    npm version "{{LEVEL}}"


# VERSION BUMP FOR ELIXIR SDK
[group("version-bump")]
[working-directory("sdk/elixir/")]
version-bump-sdk-elixir level="patch":
    #!/bin/bash
    set -euo pipefail
    LEVEL="{{level}}"
    case "$LEVEL" in patch|minor|major) ;; *)
      echo "Invalid level '$LEVEL'. Use patch|minor|major." >&2
      exit 1
      ;;
    esac

    VSN=$(awk -F\" '/^[[:space:]]*version:/ {print $2; exit}' mix.exs)
    IFS='.' read -r MAJOR MINOR PATCH <<< "$VSN"
    case "$LEVEL" in
      patch) PATCH=$((PATCH + 1)) ;;
      minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
      major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    esac
    VSN="$MAJOR.$MINOR.$PATCH"

    VSN="$VSN" perl -0pi -e 's/(version:\s*")\d+\.\d+\.\d+(")/${1}$ENV{VSN}${2}/' mix.exs
    perl -0pi -e 's/{:fostrom, "~> \K\d+\.\d+\.\d+/'"$VSN"'/' README.md
    if ! grep -q "~> $VSN" README.md; then
        echo "Failed to update README.md with version $VSN" >&2
        exit 1
    fi
    echo "New version: $VSN"


# VERIFY THAT THE DEVICE AGENT AND DOWNLOADER EXPECTED VERSION MATCH
[private]
[group("build")]
verify-version-coherence:
    #!/bin/bash
    set -euo pipefail

    AGENT_VSN="$(awk -F'\"' '/^[[:space:]]*version[[:space:]]*=/ { print $2; exit }' device-agent/Cargo.toml)"
    DL_VSN="$(awk -F'\"' '/^VERSION="/ { print $2; exit }' sdk/dl-agent.sh)"
    DL_VSN="${DL_VSN#v}"

    if [[ -z "$AGENT_VSN" || -z "$DL_VSN" ]]; then
      echo "Failed to read versions from device-agent/Cargo.toml and sdk/dl-agent.sh" >&2
      exit 1
    fi

    if [[ "$AGENT_VSN" != "$DL_VSN" ]]; then
      echo "Version mismatch:" >&2
      echo "  device-agent/Cargo.toml: $AGENT_VSN" >&2
      echo "  sdk/dl-agent.sh:         v$DL_VSN" >&2
      exit 1
    fi



# ---------------
# --- PUBLISH ---
# ---------------


# PUBLISH PYTHON SDK
[confirm("Publish Python SDK?")]
[group("publish")]
[working-directory("sdk/python/")]
publish-sdk-python:
    just build
    uv publish --username __token__


# PUBLISH JS SDK
[confirm("Publish JS SDK?")]
[group("publish")]
[working-directory("sdk/js/")]
publish-sdk-js:
    just build

    rm -rf dl-agent.sh
    cp ../dl-agent.sh .
    chmod +x dl-agent.sh
    npm publish --access public
    rm -rf dl-agent.sh
    ln -s ../dl-agent.sh dl-agent.sh


# PUBLISH ELIXIR SDK
[confirm("Publish Elixir SDK?")]
[group("publish")]
[working-directory("sdk/elixir/")]
publish-sdk-elixir:
    just build

    rm -rf dl-agent.sh
    cp ../dl-agent.sh .
    chmod +x dl-agent.sh
    mix hex.publish
    rm -rf dl-agent.sh
    ln -s ../dl-agent.sh dl-agent.sh


# PUBLISH DEVICE AGENT RELEASE
[confirm("Publish Device Agent Release to CDNs?")]
[group("publish")]
[working-directory("device-agent/")]
publish-device-agent:
    #!/bin/bash
    set -euo pipefail

    # make release
    just release

    VSN=$(.release/{{BIN_OS_ARCH}} version | tr -d '[:space:]')
    echo "Releasing version $VSN"

    # upload release to CDNs
    just upload-device-agent-to-cdn tigris
    just upload-device-agent-to-cdn bunny
    just verify-device-agent-cdn


[private]
[group("publish")]
[working-directory("device-agent/")]
upload-device-agent-to-cdn cdn:
    #!/bin/bash
    set -euo pipefail

    VSN=$(.release/{{BIN_OS_ARCH}} version | tr -d '[:space:]')
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}-linux-arm64"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}-linux-armv6hf"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}-linux-amd64"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}-linux-riscv64"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}-macos-arm64"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}-macos-amd64"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}.vsn"
    just "upload-to-{{cdn}}-cdn" $VSN "{{BIN}}.sha256"
    echo "Release $VSN uploaded to {{cdn}} CDN successfully!"


[private]
[group("publish")]
[working-directory("device-agent/")]
upload-to-dryrun-cdn vsn file:
    echo "will upload: {{vsn}} {{file}}"
    [ -f ".release/{{file}}" ] || (echo "Error: {{file}} does not exist" && exit 1)


[private]
[group("publish")]
[working-directory("device-agent/")]
upload-to-tigris-cdn vsn file:
    #!/bin/bash
    set -euo pipefail

    echo "Uploading to Tigris: {{file}}"
    [ -f ".release/{{file}}" ] || (echo "Error: {{file}} does not exist" && exit 1)
    [ -z "${TIGRIS_ID}" ] && echo "Error: \$TIGRIS_ID is not set" && exit 1 || true
    [ -z "${TIGRIS_SECRET}" ] && echo "Error: \$TIGRIS_SECRET is not set" && exit 1 || true
    FILE_HASH=$(sha256sum ".release/{{file}}" | cut -d' ' -f1)

    curl -X PUT \
      --progress-bar \
      --aws-sigv4 "aws:amz:auto:s3" \
      --user "${TIGRIS_ID}:${TIGRIS_SECRET}" \
      --header "x-amz-content-sha256: ${FILE_HASH}" \
      --upload-file ".release/{{file}}" \
      "https://fostrom.t3.storage.dev/fostrom-device-agent/{{vsn}}/{{file}}"

      curl -X PUT \
        --progress-bar \
        --aws-sigv4 "aws:amz:auto:s3" \
        --user "${TIGRIS_ID}:${TIGRIS_SECRET}" \
        --header "x-amz-content-sha256: ${FILE_HASH}" \
        --upload-file ".release/{{file}}" \
        "https://fostrom.t3.storage.dev/fostrom-device-agent/latest/{{file}}"


[private]
[group("publish")]
[working-directory("device-agent/")]
upload-to-bunny-cdn vsn file:
    echo "Uploading to Bunny: {{file}}"
    [ -f ".release/{{file}}" ] || (echo "Error: {{file}} does not exist" && exit 1)
    [ -z "${BUNNY_SECRET}" ] && echo "Error: \$BUNNY_SECRET is not set" && exit 1 || true

    curl -X PUT -s \
      --progress-bar \
      --header "AccessKey: ${BUNNY_SECRET}" \
      --header "Content-Type: application/octet-stream" \
      --header "accept: application/json"  \
      --upload-file ".release/{{file}}" \
      "https://uk.storage.bunnycdn.com/fostrom/fostrom-device-agent/{{vsn}}/{{file}}" > /dev/null

    curl -X PUT -s \
        --progress-bar \
        --header "AccessKey: ${BUNNY_SECRET}" \
        --header "Content-Type: application/octet-stream" \
        --header "accept: application/json"  \
        --upload-file ".release/{{file}}" \
        "https://uk.storage.bunnycdn.com/fostrom/fostrom-device-agent/latest/{{file}}" > /dev/null


[private]
[group("publish")]
[working-directory("device-agent/")]
verify-device-agent-cdn:
    #!/bin/bash
    set -euo pipefail

    VSN=$(.release/{{BIN_OS_ARCH}} version | tr -d '[:space:]')
    FILES=(
      "{{BIN}}-linux-arm64"
      "{{BIN}}-linux-armv6hf"
      "{{BIN}}-linux-amd64"
      "{{BIN}}-linux-riscv64"
      "{{BIN}}-macos-arm64"
      "{{BIN}}-macos-amd64"
      "{{BIN}}.vsn"
      "{{BIN}}.sha256"
    )
    BASES=(
      "https://cdn.fostrom.dev/fostrom-device-agent/$VSN"
      "https://b.cdn.fostrom.dev/fostrom-device-agent/$VSN"
    )

    for BASE in "${BASES[@]}"; do
      for FILE in "${FILES[@]}"; do
        URL="$BASE/$FILE"
        echo "Verifying: $URL"
        curl -fsI "$URL" >/dev/null || (echo "Missing CDN artifact: $URL" >&2 && exit 1)
      done
    done
    echo "Verified CDN artifacts for Device Agent $VSN"


# ---------------
# --- CLEANUP ---
# ---------------


# CLEAN ALL
[group("clean")]
clean-all:
    just clean-device-agent
    just clean-sdks


# CLEAN DEVICE AGENT
[group("clean")]
clean-device-agent:
    rm -rf device-agent/target/
    rm -rf device-agent/.release/


# CLEAN SDKS
[group("clean")]
clean-sdks:
    rm -rf sdk/elixir/_build/
    rm -rf sdk/elixir/deps/
    rm -rf sdk/elixir/.elixir_ls/
    rm -rf sdk/elixir/.expert/
    rm -rf sdk/elixir/priv/.agent/

    rm -rf sdk/python/uv.lock
    rm -rf sdk/python/.venv/
    rm -rf sdk/python/dist/
    rm -rf sdk/python/build/
    rm -rf sdk/python/.pytest_cache/
    rm -rf sdk/python/.ruff_cache/
    rm -rf sdk/python/src/fostrom/.agent/

    rm -rf sdk/js/node_modules/
    rm -rf sdk/js/.agent/
