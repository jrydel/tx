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

# How to run tx in this terminal: by name if it's already on PATH, else by full path.
tx="tx"
case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    tx="$dir/tx"
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

# Set up right away when a person is at the terminal (`curl … | sh` keeps the terminal on /dev/tty).
# A bare `tx` walks through setup until it's done, then just shows where you stand, so upgrades are quiet.
if [ -z "${TX_NO_SETUP:-}" ] && [ -t 1 ] && { : </dev/tty; } 2>/dev/null; then
  echo
  echo "Setting tx up now. Skip with Ctrl+C; run $tx any time to continue."
  "$dir/tx" </dev/tty || true
else
  cat <<EOF

Next, run this to set tx up (your AI apps, what you want to learn, what to follow):
  $tx
EOF
fi
