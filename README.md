# Docklet

A Windows-first system-tray utility for running local Docker services and private llama.cpp models without a terminal.

Docklet manages one container per service, keeps data in named Docker volumes, and gets out of the way when you close it. Redis, PostgreSQL, MySQL, and MongoDB are built in; the Library adds curated services and Docker Hub search for everything else. It can also install a CPU llama.cpp runtime and serve one GGUF model at a time through an OpenAI-compatible endpoint on your machine.

## Features

- **Tray quick panel** — a compact smoked-glass panel anchored to the Docklet tray icon. Left-click a service or model to start or stop it, right-click to copy its URL or endpoint.
- **Adaptive layout** — the panel grows and shrinks with the services and models that have actually been set up.
- **Built-in services** — Redis, PostgreSQL, MySQL, and MongoDB with configurable local ports.
- **Service Library** — hand-picked templates for Nginx, Adminer, MariaDB, RabbitMQ, Memcached, Grafana, Jaeger, Uptime Kuma, Meilisearch, and n8n, plus live Docker Hub search.
- **Custom services** — one image, one `127.0.0.1` port binding, a URL template, and optional named volume, environment variables, and health command.
- **Local AI** — install a pinned CPU llama.cpp engine and serve one GGUF model at a time at `http://127.0.0.1:11435/v1` with a generated bearer key. Use the curated catalog or add a public GGUF from Hugging Face.
- **Models page** — engine setup, download progress, custom model entry, copyable endpoint/API key/model name, and safe model removal.
- **Docker Desktop handoff** — launch Docker Desktop from the offline state and let Docklet reconnect when the engine is ready.
- **Safe lifecycle** — Docklet only removes containers and volumes carrying its own `com.docklet.managed=true` label. Unrelated Docker resources are never pruned.
- **Runs in the background** — closing the main window or quick panel keeps Docklet and its containers running. Quit only from the tray menu.
- **Autostart** — optional launch at login that stays hidden in the tray.
- **Hidden-when-unused** — services and models that have not been created yet are hidden from the quick panel.

## Requirements

- Windows 10 or 11 (x64)
- Microsoft Edge WebView2 Runtime (installed on current Windows versions; the installer can bootstrap it)
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) running with Linux containers for Docker services
- An internet connection for first-time llama.cpp engine and model downloads

Docker services require Docker Desktop. Local models can run without it. The bundled llama.cpp engine is a CPU-only Windows x64 build, so no GPU is required.

## Install

Download the latest Windows installer from the [Releases](https://github.com/Subho4531/docklet/releases) page:

- `Docklet_0.2.0_x64_en-US.msi` — Windows Installer package
- `Docklet_0.2.0_x64-setup.exe` — NSIS installer

The current installers are not code-signed, so Windows SmartScreen may ask you to confirm the publisher.

## Using Docklet

1. Launch Docklet from the Start menu. The taskbar window opens on **Services**.
2. If Docker Desktop is offline, choose **Start Docker Desktop** and wait for the engine to become ready.
3. Open **Library** to install a ready-to-run service, or search Docker Hub for a specific image.
4. Configure the local port and optional volume, environment, and health check, then install.
5. Open **Models** to install llama.cpp, add a GGUF model, and press **Start** to serve it. Copy the endpoint, API key, and model name into any OpenAI-compatible client.
6. Use the tray icon to open the quick panel, or close the main window and let Docklet keep running in the background.

Built-in service ports can be changed in **Settings**. Resetting a service deletes its managed container and named volume, then recreates it with clean data. Removing a custom service deletes only that service's managed container and volume.

To quit Docklet completely, use **Quit** in the tray menu.

## Local AI

The Models page installs a pinned llama.cpp `b11838` CPU build on first use. The engine archive is checksum-verified before it is extracted. Model files are downloaded from the curated catalog or a public Hugging Face `.gguf` file and stay in Docklet's local app-data folder.

- Default endpoint: `http://127.0.0.1:11435/v1`
- Authentication: generated bearer key, shown on the Models page and in the quick panel
- One model serves at a time; loading a model on CPU can take a few minutes
- Removing a model stops it first and deletes only that model's files

The endpoint is bound to `127.0.0.1` and is not exposed to the network.

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

- `src/` — Angular UI, tray quick panel, service store, Library, and Models pages
- `src-tauri/src/` — Rust commands, Docker CLI wrapper, llama.cpp runtime, lifecycle management, tray, and window shaping
- `src-tauri/capabilities/` — explicit Tauri permissions for the popup and main windows
- `src/assets/services/` — service icons
- `website/` — static launch and download site

## Status

Version `0.2.0` adds a local llama.cpp runtime and Models page, quick-panel model controls, Docker Desktop launch, and a rebuilt smoked-glass interface. It still targets one managed container per service and does not include a dashboard, multi-host support, or arbitrary Docker administration.
