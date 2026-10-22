#!/bin/sh
# Install the tx command for macOS (Apple Silicon) or Linux (x86_64):
#   curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh
set -eu

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) asset=tx-cli-aarch64-apple-darwin ;;
  Linux-x86_64) asset=tx-cli-x86_64-unknown-linux-gnu ;;
  *) echo "tx: no prebuilt binary for $(uname -s) $(uname -m); build from source: https://github.com/jrydel/tx" >&2; exit 1 ;;
esac

writable() { mkdir -p "$1" 2>/dev/null && [ -w "$1" ]; }

if [ -n "${TX_INSTALL_DIR:-}" ]; then
  dir="$TX_INSTALL_DIR"
elif writable "$HOME/.local/bin"; then
  dir="$HOME/.local/bin"
else
  # ~/.local sometimes belongs to root (left by a tool run with sudo); use a folder of our own instead.
  dir="$HOME/.tx/bin"
fi
if ! writable "$dir"; then
  echo "tx: can't write to $dir. Pick a folder you own, for example:" >&2
  echo "  curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | TX_INSTALL_DIR=\"\$HOME/bin\" sh" >&2
  exit 1
fi
echo "Downloading ${asset}..."
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
    line="export PATH=\"$dir:\$PATH\""
    if grep -qsF "$line" "$rc"; then
      echo "$rc already adds $dir to your PATH. Open a new terminal to use tx."
    elif { printf '\n# tx\n%s\n' "$line" >> "$rc"; } 2>/dev/null; then
      echo "Added $dir to your PATH in $rc. Open a new terminal to use tx."
    else
      echo "Add $dir to your PATH, then open a new terminal:"
      echo "  echo '$line' >> $rc"
    fi
    ;;
esac

cat <<EOF

Next, connect tx to your AI (run the one you use, then restart that app):
  tx setup codex            # Codex: ChatGPT desktop app or CLI
  tx setup claude-code      # Claude Code
  tx setup claude-desktop   # Claude Desktop
EOF
