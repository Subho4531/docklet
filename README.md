# Docklet

A Windows-first system-tray utility for running local Docker services without a terminal.

Docklet manages one container per service, keeps data in named Docker volumes, and gets out of the way when you close it. Redis, PostgreSQL, MySQL, and MongoDB are built in; the Library adds curated services and Docker Hub search for everything else.

## Features

- **Tray quick panel** — a compact smoked-glass panel anchored to the Docklet tray icon. Left-click a service to start or stop it, right-click to copy its URL.
- **Adaptive layout** — the panel grows and shrinks with the services that have actually been set up.
- **Built-in services** — Redis, PostgreSQL, MySQL, and MongoDB with configurable local ports.
- **Service Library** — hand-picked templates for Nginx, Adminer, MariaDB, RabbitMQ, Memcached, Grafana, Jaeger, Uptime Kuma, Meilisearch, and n8n, plus live Docker Hub search.
- **Custom services** — one image, one `127.0.0.1` port binding, a URL template, and optional named volume, environment variables, and health command.
- **Safe lifecycle** — Docklet only removes containers and volumes carrying its own `com.docklet.managed=true` label. Unrelated Docker resources are never pruned.
- **Runs in the background** — closing the main window or quick panel keeps Docklet and its containers running. Quit only from the tray menu.
- **Autostart** — optional launch at login that stays hidden in the tray.
- **Hidden-when-unused** — services that have not been created yet are hidden from the quick panel.

## Requirements

- Windows 10 or 11
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) running with Linux containers
- Microsoft Edge WebView2 Runtime (installed on current Windows versions; the installer can bootstrap it)

Docker must be running before Docklet can start, stop, or inspect services.

## Install

Download the latest Windows installer from the [Releases](https://github.com/Subho4531/docklet/releases) page:

- `Docklet_0.1.0_x64_en-US.msi` — Windows Installer package
- `Docklet_0.1.0_x64-setup.exe` — NSIS installer

The current installers are not code-signed, so Windows SmartScreen may ask you to confirm the publisher.

## Using Docklet

1. Start Docker Desktop.
2. Launch Docklet from the Start menu. The taskbar window opens on **Services**.
3. Open **Library** to install a ready-to-run service, or search Docker Hub for a specific image.
4. Configure the local port and optional volume, environment, and health check, then install.
5. Use the tray icon to open the quick panel, or close the main window and let Docklet keep running in the background.

Built-in service ports can be changed in **Settings**. Resetting a service deletes its managed container and named volume, then recreates it with clean data. Removing a custom service deletes only that service's managed container and volume.

To quit Docklet completely, use **Quit** in the tray menu.

## Custom service limits

Docklet intentionally supports a narrow, safe subset of Docker:

- one image and one container per service
- one host port bound to `127.0.0.1`
- an optional named volume
- optional `KEY=value` environment variables
- an optional in-container health command

There is no Compose support, bind mounts, privileged mode, host networking, or multi-container orchestration.

## Development

Docklet uses Tauri 2, Angular 22, Rust, and Bun.

```powershell
bun install
bun run tauri dev
```

The Tauri dev command starts Angular on port `1420` and opens the desktop app. To run only the frontend:

```powershell
bun run start
```

Useful checks:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
bun run build
```

Build the production Windows application and installers:

```powershell
bun run tauri build
```

Build only one installer format:

```powershell
bun run tauri build --bundles msi
bun run tauri build --bundles nsis
```

Build output is written to `src-tauri/target/release/bundle/`.

## Project layout

- `src/` — Angular UI, tray quick panel, service store, and Library page
- `src-tauri/src/` — Rust commands, Docker CLI wrapper, lifecycle management, tray, and window shaping
- `src-tauri/capabilities/` — explicit Tauri permissions for the popup and main windows
- `src/assets/services/` — service icons
- `website/` — static launch and download site

## Status

Version `0.1.0` is the first Windows release. It targets one managed container per service and does not include a dashboard, multi-host support, or arbitrary Docker administration.
