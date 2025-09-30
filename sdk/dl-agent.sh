#!/bin/sh
#
# dl-agent.sh - Download Fostrom Device Agent binary
#
# Usage: ./dl-agent.sh <directory>

VERSION="v0.0.13"

# CDN URLs in order of preference
CDN_PRIMARY="https://cdn.fostrom.dev/fostrom-device-agent/$VERSION"
CDN_SECONDARY="https://b.cdn.fostrom.dev/fostrom-device-agent/$VERSION"

set -e

# Print error message and exit
die() {
    printf "Failed to download Fostrom Device Agent: %s\n" "$1" >&2
    exit 1
}

# Check arguments
[ $# -eq 0 ] && die "Usage: $0 <directory>"
LOCATION="$1"

download_file() {
    URL="$1"
    OUTPUT="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --connect-timeout 10 --max-time 300 "$URL" -o "$OUTPUT" 2>/dev/null
    elif command -v wget >/dev/null 2>&1; then
        wget -q --timeout=10 --tries=1 -O "$OUTPUT" "$URL" 2>/dev/null
    else
        die "No download tool found (curl or wget required)"
    fi
}

try_download() {
    FILE_PATH="$1"
    OUTPUT="$2"

    for CDN_URL in "$CDN_PRIMARY" "$CDN_SECONDARY"; do
        FULL_URL="${CDN_URL}/${FILE_PATH}"
        if download_file "$FULL_URL" "$OUTPUT"; then
            return 0
        fi
    done
    return 1
}

create_temp_dir() {
    if command -v mktemp >/dev/null 2>&1; then
        mktemp -d 2>/dev/null || mktemp -d -t 'fostrom'
    else
        TEMP_DIR="/tmp/fostrom.$$"
        mkdir -p "$TEMP_DIR" || die "Cannot create temporary directory"
        printf "%s\n" "$TEMP_DIR"
    fi
}

verify_checksum() {
    CHECKSUM_FILE="$1"

    # Try sha256sum (Linux) first, then shasum (macOS)
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c --ignore-missing "$CHECKSUM_FILE" >/dev/null 2>&1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 -c --ignore-missing "$CHECKSUM_FILE" >/dev/null 2>&1
    else
        printf "Warning: No checksum verification tool found, skipping verification\n" >&2
        return 0
    fi
}

download_and_verify() {
    FILENAME="$1"
    INSTALL_DIR="$2"

    TEMP_DIR="$(create_temp_dir)"
    trap 'rm -rf "$TEMP_DIR"' EXIT INT TERM

    TEMP_BINARY="$TEMP_DIR/$FILENAME"
    TEMP_CHECKSUM="$TEMP_DIR/fostrom-device-agent.sha256"

    # Download checksum file
    if ! try_download "fostrom-device-agent.sha256" "$TEMP_CHECKSUM"; then
        die "Failed to download checksum file"
    fi

    # Download binary
    if ! try_download "$FILENAME" "$TEMP_BINARY"; then
        die "Failed to download binary"
    fi

    # Verify checksum
    cd "$TEMP_DIR"
    if ! verify_checksum "fostrom-device-agent.sha256"; then
        die "Checksum verification failed"
    fi
    cd - >/dev/null

    install_binary "$TEMP_BINARY" "$INSTALL_DIR" "$FILENAME"
}

install_binary() {
    TEMP_BINARY="$1"
    INSTALL_DIR="$2"
    FILENAME="$3"

    # Create target directory if needed
    [ ! -d "$INSTALL_DIR" ] && mkdir -p "$INSTALL_DIR"

    # Install binary
    FINAL_BINARY_PATH="$INSTALL_DIR/$FILENAME"
    cp "$TEMP_BINARY" "$FINAL_BINARY_PATH" || die "Cannot copy binary to $FINAL_BINARY_PATH"
    chmod +x "$FINAL_BINARY_PATH" || die "Cannot make binary executable"

    # Create symlink
    ln -sf "$FILENAME" "$INSTALL_DIR/fostrom-device-agent"
}

main() {
    # Detect OS and architecture
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    # Normalize OS name
    case "$OS" in
        Linux*)
            OS="linux"
            case "$ARCH" in
                x86_64|amd64)  ARCH="amd64" ;;
                aarch64|arm64) ARCH="arm64" ;;
                armv6l)        ARCH="armv6" ;;
                riscv64)       ARCH="riscv64" ;;
                *)             die "Unsupported architecture: $ARCH" ;;
            esac
            ;;
        Darwin*)
            OS="apple"
            case "$ARCH" in
                x86_64|amd64)  ARCH="amd64" ;;
                aarch64|arm64) ARCH="arm64" ;;
                *)             die "Unsupported architecture: $ARCH" ;;
            esac
            ;;
        *) die "Unsupported OS: $OS" ;;
    esac

    FILENAME="fostrom-device-agent-${OS}-${ARCH}"

    # Check if binary already exists
    if [ -f "$LOCATION/$FILENAME" ]; then
        exit 0
    fi

    printf "Downloading Fostrom Device Agent...\n"

    download_and_verify "$FILENAME" "$LOCATION"

    # Remove quarantine on macOS
    if [ "$OS" = "apple" ]; then
        xattr -r -d com.apple.quarantine "$LOCATION/$FILENAME" 2>/dev/null || true
    fi

    printf "Fostrom Device Agent downloaded successfully\n"
}

main
