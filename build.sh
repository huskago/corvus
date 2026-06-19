#!/usr/bin/env bash
set -e

KEY_PATH="$HOME/.tauri/corvus.key"

if [ ! -f "$KEY_PATH" ]; then
    echo "Private key not found: $KEY_PATH" >&2
    exit 1
fi

export TAURI_SIGNING_PRIVATE_KEY=$(cat "$KEY_PATH")
read -rsp "Key password: " TAURI_SIGNING_PRIVATE_KEY_PASSWORD
echo
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

cargo tauri build
