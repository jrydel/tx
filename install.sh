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
mkdir -p "$dir"
echo "Downloading $asset…"
curl -fsSL "https://github.com/jrydel/tx/releases/latest/download/$asset" -o "$dir/tx.tmp"
chmod +x "$dir/tx.tmp"
mv "$dir/tx.tmp" "$dir/tx"
# Downloads via curl carry no quarantine flag on macOS, so Gatekeeper doesn't block the binary.
echo "Installed $("$dir/tx" --version) to $dir/tx"

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "Add $dir to your PATH, e.g.: echo 'export PATH=\"$dir:\$PATH\"' >> ~/.profile" ;;
esac

cat <<EOF

Next, connect tx to your AI (run the one you use, then restart that app):
  tx setup codex            # Codex: ChatGPT desktop app or CLI
  tx setup claude-code      # Claude Code
  tx setup claude-desktop   # Claude Desktop
EOF
