#!/usr/bin/env bash
# One-command build + run for local testing: installs frontend deps (if needed),
# compiles the app in release mode, then runs the binary directly.
# --no-bundle skips OS-package bundling (deb/rpm/AppImage) since that's not
# needed for local testing and AppImage bundling requires FUSE2, which isn't
# installed on this system.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

(cd frontend && npm install)

# Invoked directly (not via `npm run tauri`) and from the repo root: the Tauri
# CLI only searches subfolders of its cwd for tauri.conf.json, and frontend/
# and src-tauri/ are siblings, so running it from inside frontend/ can't find it.
./frontend/node_modules/.bin/tauri build --no-bundle

# WEBKIT_DISABLE_DMABUF_RENDERER works around a WebKitGTK/Wayland crash
# ("Error 71 (Protocol error) dispatching to Wayland display") seen on this
# machine's compositor.
# Workspace-level target dir (not src-tauri/target) since this is a Cargo workspace.
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./target/release/sparrow
