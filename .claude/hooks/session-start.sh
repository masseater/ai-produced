#!/bin/bash
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

if ! command -v vp >/dev/null 2>&1; then
  curl -fsSL https://vite.plus | VP_NODE_MANAGER=no bash >&2
  echo ". \"$HOME/.config/vite-plus/env\"" >>"$CLAUDE_ENV_FILE"
  . "$HOME/.config/vite-plus/env"
fi

echo "export ZIZMOR_NO_ONLINE_AUDITS=true" >>"$CLAUDE_ENV_FILE"

cd "$CLAUDE_PROJECT_DIR"
if ! command -v mise >/dev/null 2>&1; then
  curl -fsSL https://mise.run | sh >&2
  export PATH="$HOME/.local/bin:$PATH"
fi
mise trust --quiet mise.toml
mise install --locked >&2
echo "export PATH=\"$HOME/.local/bin:$HOME/.local/share/mise/shims:\$PATH\"" >>"$CLAUDE_ENV_FILE"
vp env on >&2
vp install --frozen-lockfile >&2
