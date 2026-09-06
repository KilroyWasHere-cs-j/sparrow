#!/usr/bin/env bash
# Build, bundle, and launch the app.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

./bundle.sh

# WEBKIT_DISABLE_DMABUF_RENDERER works around a WebKitGTK/Wayland crash
# ("Error 71 (Protocol error) dispatching to Wayland display") seen on this
# machine's compositor.
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./target/release/sparrow
