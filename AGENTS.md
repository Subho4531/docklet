# AGENTS.md

## Repository state

- Fresh **Tauri 2 + Angular 22** desktop scaffold. The only implemented feature is the default greet demo; do not assume the tray/Redis work described below exists yet.
- The agreed v1 product is a **Windows-first system-tray utility for one managed Redis container**, not a dashboard or multi-service manager.
- `README.md` is still stock scaffolding. The product constraints in this file are the source of truth.

## Commands

- **Bun is the package manager.** Use `bun install`; do not introduce npm/yarn/pnpm artifacts.
- `bun run tauri dev` — full desktop app. Tauri starts Angular on port `1420` via `beforeDevCommand`.
- `bun run start` — Angular dev server only.
- `bun run build` — production frontend build to `dist/docklet/browser`.
- `bun run tauri build` — desktop bundle.
- From `src-tauri`: `cargo check` for Rust, `cargo test` for tests, `cargo test <name>` for one test.
- There are no lint, test, or CI scripts configured. Do not claim a check ran when no command exists.

## Wiring and quirks

- Rust starts at `src-tauri/src/main.rs`, which calls `docklet_lib::run()` in `src-tauri/src/lib.rs`. Register new Tauri commands in that builder's `invoke_handler`.
- Angular starts at `src/main.ts` and renders `src/app/app.component.ts`. It currently calls `invoke("greet")`; replace the demo rather than layering new UI beside it.
- Tauri 2 permissions are explicit: every plugin must be added as a Rust dependency, registered in `lib.rs`, and allowed in `src-tauri/capabilities/default.json`. Only the `main` window has a capability today.
- `tauri.conf.json` currently creates a visible, decorated 800×600 window. Tray behavior and translucency require config and Rust setup changes.
- Angular is standalone and uses signals; there is no `zone.js` dependency or Angular test target. Do not run `ng test`.
- Shared design tokens belong in `src/styles.css`; component CSS warns above 4 KB and fails above 8 KB in production builds.
- `/dist`, `node_modules`, `src-tauri/target`, and `src-tauri/gen` are ignored. Dependencies are not installed until `bun install` runs.

## Product constraints

- **Redis only in v1.** Dedicated Docker container `docklet-redis`, image `redis:8-alpine`, volume `docklet-redis-data`, bound to `127.0.0.1:6379:6379`.
- Start auto-creates the container/volume if missing and waits for Redis to be reachable. Copy the **actual Docker-mapped port** as `redis://127.0.0.1:<port>`.
- Stop is graceful and preserves the container and volume. Reset is destructive: confirm first, then remove only the Docklet-managed container and volume, recreate, and start.
- Never delete or prune arbitrary containers or volumes.
- Use the Docker CLI; do not introduce Compose or a Docker daemon library without a clear reason.
- Docker Desktop must be running for real integration checks. Verify with `docker info`; if unavailable, test error/offline states and state that Docker integration was not verified.
- UI: compact popup anchored to the tray. One fixed **smoked-glass** appearance with native acrylic/translucency. Do not add a light/dark theme switch or opaque dashboard surfaces.
- Closing/hiding the popup keeps the app and Redis running. Quit only from the tray menu. Autostart launch stays hidden; manual launch may show the popup.

## Git workflow

- Commit each self-contained, verified change. Do not batch unrelated work.
- Use `.opencode/agents/git-committer.md` for commits after implementation changes.
- Conventional Commits, imperative and scoped where useful: `feat(tray): ...`, `fix(redis): ...`, `docs: ...`.
- Stage only intended files. Never commit build output, dependencies, secrets, or unrelated edits.
- Do not amend, rebase, or force-push unless explicitly asked.
- Origin is `Subho4531/docklet` on branch `master`. The repository was initially empty; the first push establishes `origin/master`.
- SSH to GitHub currently fails with `Permission denied (publickey)`. `gh` is authenticated. Use HTTPS plus the GitHub CLI credential helper for pushes:
  `git -c credential.helper="!gh auth git-credential" push https://github.com/Subho4531/docklet.git HEAD:master`
- If using the configured remote, ensure `gh auth status` is healthy before blaming the repository.
