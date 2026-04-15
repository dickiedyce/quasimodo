#!/usr/bin/env sh
set -eu

# Adds quasimodo shell functions to ~/.zshrc.
# Safe to run multiple times -- skips any block already present.

ZSHRC="${ZSHRC_FILE:-$HOME/.zshrc}"

# ── helpers ───────────────────────────────────────────────────────────────────

already_present() {
  grep -qF "$1" "$ZSHRC" 2>/dev/null
}

append_block() {
  printf '\n%s\n' "$1" >> "$ZSHRC"
  echo "Added: $2"
}

maybe_add() {
  marker="$1"
  block="$2"
  label="$3"
  if already_present "$marker"; then
    echo "Skipped (already present): $label"
  else
    append_block "$block" "$label"
  fi
}

# ── function definitions ──────────────────────────────────────────────────────

# qq  -- query: natural language -> shell command
QQ_BLOCK='# quasimodo: query
qq() { quasimodo --prompt "$*" --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"; }'

# qd  -- describe: explain what a shell command does
QD_BLOCK='# quasimodo: describe
qd() { quasimodo --describe "$*" --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"; }'

# qe  -- explain: describe what a failed command did wrong
QE_BLOCK='# quasimodo: explain
qe() { quasimodo --explain "$*" --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"; }'

# qh  -- help: show CLI usage
QH_BLOCK='# quasimodo: help
qh() { quasimodo --help; }'

# qt  -- teach: store a corrected example
#   usage: qt "description" "correct command"
QT_BLOCK='# quasimodo: teach
qt() {
  if [ $# -lt 2 ]; then
    echo "usage: qt <description> <command>" >&2
    return 1
  fi
  quasimodo --teach "$1" --command "$2" --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"
}'

# qlt -- list-taught: show all taught examples
QLT_BLOCK='# quasimodo: list taught
qlt() { quasimodo --list-taught --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"; }'

# qrm -- remove taught example by description substring
#   usage: qrm "description substring"
QRM_BLOCK='# quasimodo: delete taught
qrm() {
  if [ $# -lt 1 ]; then
    echo "usage: qrm <description substring>" >&2
    return 1
  fi
  quasimodo --delete-taught "$1" --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"
}'

# qnf -- not-found: manually query the not-found resolver
QNF_BLOCK='# quasimodo: not-found lookup
qnf() { quasimodo --notfound "$1" --bank "${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"; }'

# qb  -- build-bank: rebuild the retrieval database from tldr pages
QB_BLOCK='# quasimodo: build bank
qb() {
  _bank="${QUASIMODO_BANK:-$HOME/.quasimodo/tldr_bank.db}"
  mkdir -p "$(dirname "$_bank")"
  build-bank "$_bank" 2>/dev/null \
    || cargo run --manifest-path "${QUASIMODO_REPO:-$HOME/Documents/GitHub/quasimodo}/Cargo.toml" \
         --bin build-bank -- "$_bank"
}'

# ── apply ─────────────────────────────────────────────────────────────────────

maybe_add "# quasimodo: query"     "$QQ_BLOCK"  "qq  (query)"
maybe_add "# quasimodo: describe"  "$QD_BLOCK"  "qd  (describe)"
maybe_add "# quasimodo: explain"   "$QE_BLOCK"  "qe  (explain)"
maybe_add "# quasimodo: help"      "$QH_BLOCK"  "qh  (help)"
maybe_add "# quasimodo: teach"     "$QT_BLOCK"  "qt  (teach)"
maybe_add "# quasimodo: list taught" "$QLT_BLOCK" "qlt (list taught)"
maybe_add "# quasimodo: delete taught" "$QRM_BLOCK" "qrm (delete taught)"
maybe_add "# quasimodo: not-found" "$QNF_BLOCK" "qnf (not-found lookup)"
maybe_add "# quasimodo: build"     "$QB_BLOCK"  "qb  (build bank)"

echo ""
echo "Done. Reload with:  source ~/.zshrc"
