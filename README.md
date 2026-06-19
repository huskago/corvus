# Corvus

A custom Minecraft launcher built with [Tauri v2](https://tauri.app) and [Leptos](https://leptos.dev). Connects to a [corvus-server](https://github.com/huskago/corvus-server) backend to fetch instances, news, and mod manifests.

## Features

- Microsoft (Xbox Live) and offline account support
- Automatic game file synchronization from a remote manifest
- Fabric, Forge, NeoForge, Quilt, and Vanilla support
- Automatic Java download and management
- World backups
- Mod toggles, JVM arguments, and per-instance settings
- Built-in console and launch progress tracking
- Crash detection with direct access to crash reports
- Auto-updater

Want to use Corvus as a base for your own launcher? See [FORK.md](FORK.md).

## Setup

### Requirements

- [Rust](https://rustup.rs) 1.85+
- [trunk](https://trunkrs.dev) (`cargo install trunk`)
- [Tauri CLI v2](https://tauri.app/start/prerequisites/) (`cargo install tauri-cli --version '^2'`)
- `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)

### Development

```sh
git clone https://github.com/huskago/corvus
cd corvus
cp src-tauri/launcher.toml.example src-tauri/launcher.toml
# edit src-tauri/launcher.toml
cargo tauri dev
```

### Build

For a signed release (required for the auto-updater), use the provided build scripts:

```sh
# Windows
.\build.ps1

# Linux / macOS
./build.sh
```

These scripts read your private key from `~/.tauri/corvus.key` and prompt for the password. The installer and update artifacts are output to `src-tauri/target/release/bundle/`.

## Backend

The launcher fetches instances, news, and manifests from the URLs set in `launcher.toml`. Any static file host (nginx, GitHub Pages, S3, etc.) works as long as the JSON files match the expected format.

[corvus-server](https://github.com/huskago/corvus-server) is the recommended backend, it provides an admin panel, file upload, and manifest management out of the box, but it is not required.

## License

MIT, see [LICENSE](LICENSE).
