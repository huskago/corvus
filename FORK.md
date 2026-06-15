# Forking Corvus

This guide explains how to fork Corvus and deploy your own launcher.

## What to configure

### 1. `src-tauri/launcher.toml`

Copy the example and fill in your values:

```sh
cp src-tauri/launcher.toml.example src-tauri/launcher.toml
```

```toml
[branding]
name = "MyLauncher"

[server]
instances_url = "https://example.com/instances.json"
news_url      = "https://example.com/news.json"

[auth]
microsoft_client_id = "your-azure-app-client-id"
allow_offline = true
```

This file is gitignored, never commit it.

### 2. `src-tauri/tauri.conf.json`

Update:
- `productName` - your launcher name
- `identifier` - a unique reverse-domain ID (e.g. `com.yourname.yourlauncher`)
- `plugins > updater > endpoints` - where your launcher checks for updates
- `plugins > updater > pubkey` - your signing public key

Generate a signing keypair:

```sh
cargo tauri signer generate -w ~/.tauri/my-launcher.key
```

Put the public key in `tauri.conf.json` and keep the private key for GitHub Actions.

### 3. `src-tauri/icons/`

Replace the default icons with your own. Tauri can generate all sizes from a single 1024×1024 PNG:

```sh
cargo tauri icon path/to/your-logo.png
```

## Releases (GitHub Actions)

The included workflow (`.github/workflows/release.yml`) builds installers for Windows, Linux, and macOS. To enable automatic releases on tag push, update the trigger in the workflow file:

```yaml
on:
  push:
    tags:
      - 'v*'
```

Then add these secrets to your GitHub repository (**Settings → Secrets and variables → Actions**):

| Secret | Description |
|--------|-------------|
| `LAUNCHER_TOML` | Full content of your `launcher.toml` |
| `TAURI_SIGNING_PRIVATE_KEY` | Content of your signing private key file |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for the private key (can be empty) |

To publish a release:

```sh
git tag v1.0.0
git push origin v1.0.0
```

The workflow builds all platforms and creates a draft GitHub Release with the installers attached.

## Backend

The launcher expects `instances.json` and `news.json` at the URLs set in `launcher.toml`. See [corvus-server](https://github.com/huskago/corvus-server) for a ready-made backend, or host the JSON files yourself on any static file host.
