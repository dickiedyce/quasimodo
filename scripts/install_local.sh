#!/usr/bin/env sh
set -eu

# Installs quasimodo binary and zsh hook into user-local paths.
REPO_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

BIN_SRC="$REPO_DIR/target/debug/quasimodo"
HOOK_SRC="$REPO_DIR/hooks/quasimodo.zsh"

BIN_DIR="${QUASIMODO_BIN_DIR:-$HOME/.local/bin}"
HOOK_DIR="${QUASIMODO_HOOK_DIR:-$HOME/.quasimodo/hooks}"

BIN_DEST="$BIN_DIR/quasimodo"
HOOK_DEST="$HOOK_DIR/quasimodo.zsh"

if [ ! -f "$BIN_SRC" ]; then
  echo "Missing binary: $BIN_SRC"
  echo "Run 'cargo build' first."
  exit 1
fi

if [ ! -f "$HOOK_SRC" ]; then
  echo "Missing hook: $HOOK_SRC"
  exit 1
fi

mkdir -p "$BIN_DIR"
mkdir -p "$HOOK_DIR"

cp "$BIN_SRC" "$BIN_DEST"
cp "$HOOK_SRC" "$HOOK_DEST"

echo "Installed binary: $BIN_DEST"
echo "Installed hook:   $HOOK_DEST"
