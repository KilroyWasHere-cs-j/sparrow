#!/usr/bin/env bash
# Full build + bundle: installs frontend deps, builds the frontend and the
# Rust/Tauri binary, then packages OS installers.
# Skips AppImage — it requires FUSE2, which isn't installed on this system
# (see scripts/build.sh for the same constraint).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

(cd frontend && npm install)

# Invoked directly (not via `npm run tauri`) and from the repo root: the Tauri
# CLI only searches subfolders of its cwd for tauri.conf.json, and frontend/
# and src-tauri/ are siblings, so running it from inside frontend/ can't find it.
./frontend/node_modules/.bin/tauri build --bundles deb,rpm
