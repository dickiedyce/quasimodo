# quasimodo zsh integration
#
# Usage:
#   export QUASIMODO_BIN="$HOME/.local/bin/quasimodo"
#   export QUASIMODO_BANK="$HOME/.local/share/quasimodo/tldr_bank.db"
#   export QUASIMODO_SYSTEM="Return only shell commands"
#   export QUASIMODO_HISTORY="$HOME/.local/share/quasimodo/session.json"
#   source /path/to/quasimodo/hooks/quasimodo.zsh

: "${QUASIMODO_BIN:=quasimodo}"
: "${QUASIMODO_BANK:=tldr_bank.db}"
: "${QUASIMODO_SYSTEM:=}"
: "${QUASIMODO_HISTORY:=}"

_quasimodo_ctrl_g() {
  emulate -L zsh
  local input="$BUFFER"
  [[ -z "$input" ]] && return

  local -a extra
  [[ -n "$QUASIMODO_SYSTEM" ]] && extra+=(--system "$QUASIMODO_SYSTEM")
  [[ -n "$QUASIMODO_HISTORY" ]] && extra+=(--history-file "$QUASIMODO_HISTORY")

  local output
  output="$($QUASIMODO_BIN --prompt "$input" --bank "$QUASIMODO_BANK" "${extra[@]}" 2>/dev/null)" || return
  [[ -z "$output" ]] && return

  BUFFER="$output"
  CURSOR=${#BUFFER}
  zle redisplay
}

# Ctrl+G to transform natural language into a command in-place (never auto-runs).
zle -N _quasimodo_ctrl_g
bindkey '^G' _quasimodo_ctrl_g

command_not_found_handler() {
  emulate -L zsh
  local missing="$1"
  local suggestion
  suggestion="$($QUASIMODO_BIN --notfound "$missing" --bank "$QUASIMODO_BANK" 2>/dev/null)" || return 127
  [[ -n "$suggestion" ]] && print -P "%F{244}$suggestion%f"
  return 127
}

TRAPZERR() {
  emulate -L zsh

  # Don't recurse from within the helper itself.
  [[ "$1" == "quasimodo" ]] && return $?

  local code="$?"
  local cmd="$history[$HISTCMD]"
  [[ -z "$cmd" ]] && return $code

  local explain
  explain="$($QUASIMODO_BIN --explain "Command: $cmd -- Exit code: $code" 2>/dev/null)" || return $code
  [[ -n "$explain" ]] && print -P "%F{244}$explain%f"

  return $code
}
