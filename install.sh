#!/bin/sh
# Install the tx command for macOS (Apple Silicon) or Linux (x86_64):
#   curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh
set -eu

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) asset=tx-cli-aarch64-apple-darwin ;;
  Linux-x86_64) asset=tx-cli-x86_64-unknown-linux-gnu ;;
  *) echo "tx: no prebuilt binary for $(uname -s) $(uname -m); build from source: https://github.com/jrydel/tx" >&2; exit 1 ;;
esac

dir="${TX_INSTALL_DIR:-$HOME/.local/bin}"
# A folder created earlier by a tool run with sudo belongs to root, and the download can't be written there.
if ! mkdir -p "$dir" 2>/dev/null || [ ! -w "$dir" ]; then
  # The folder that blocks it: the default's parent ~/.local is the usual culprit.
  fix="$dir"
  case "$dir" in "$HOME/.local/"*) fix="$HOME/.local" ;; esac
  echo "tx: can't write to $dir. $fix probably belongs to root (tools run with sudo leave it that way)." >&2
  echo "Make it yours, then run the installer again:" >&2
  echo "  sudo chown -R \"\$(id -un)\" \"$fix\"" >&2
  echo "Or install somewhere else, for example ~/bin:" >&2
  echo "  curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | TX_INSTALL_DIR=\"\$HOME/bin\" sh" >&2
  exit 1
fi
echo "Downloading $asset…"
curl -fsSL "https://github.com/jrydel/tx/releases/latest/download/$asset" -o "$dir/tx.tmp"
chmod +x "$dir/tx.tmp"
mv "$dir/tx.tmp" "$dir/tx"
# Downloads via curl carry no quarantine flag on macOS, so Gatekeeper doesn't block the binary.
echo "Installed $("$dir/tx" --version) to $dir/tx"

case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    case "${SHELL:-}" in
      */zsh) rc="$HOME/.zshrc" ;;
      */bash) rc="$HOME/.bashrc" ;;
      *) rc="$HOME/.profile" ;;
    esac
    echo "Add $dir to your PATH, then open a new terminal:"
    echo "  echo 'export PATH=\"$dir:\$PATH\"' >> $rc"
    ;;
esac

cat <<EOF

Next, connect tx to your AI (run the one you use, then restart that app):
  tx setup codex            # Codex: ChatGPT desktop app or CLI
  tx setup claude-code      # Claude Code
  tx setup claude-desktop   # Claude Desktop
EOF
