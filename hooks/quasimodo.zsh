# quasimodo zsh integration
#
# Usage:
#   export QUASIMODO_BIN="$HOME/.local/bin/quasimodo"
#   export QUASIMODO_BANK="$HOME/.quasimodo/tldr_bank.db"
#   export QUASIMODO_SYSTEM="Return only shell commands"
#   export QUASIMODO_HISTORY="$HOME/.quasimodo/session.json"
#   export QUASIMODO_KEY="^G"
#   export QUASIMODO_ALT_KEY="^X^G"
#   source /path/to/quasimodo/hooks/quasimodo.zsh

: "${QUASIMODO_BIN:=$HOME/.local/bin/quasimodo}"
: "${QUASIMODO_BANK:=$HOME/.quasimodo/tldr_bank.db}"
: "${QUASIMODO_SYSTEM:=}"
: "${QUASIMODO_HISTORY:=}"
: "${QUASIMODO_KEY:=^G}"
: "${QUASIMODO_ALT_KEY:=^X^G}"

typeset -gi _QUASIMODO_IN_ZERR=0

_quasimodo_spin() {
  # Usage: _quasimodo_spin <label> <pid> <tty>
  emulate -L zsh
  unsetopt xtrace
  local label="$1" pid="$2" tty="$3"
  local -a frames=( '|' '/' '-' '\' )
  local i=1
  while kill -0 "$pid" 2>/dev/null; do
    printf '\rquasimodo: %s %s ' "$label" "${frames[i]}" >"$tty"
    i=$(( (i % 4) + 1 ))
    sleep 0.12
  done
  printf '\r\033[K' >"$tty"
}

_quasimodo_ctrl_g() {
  emulate -L zsh
  setopt localoptions nomonitor nonotify
  unsetopt xtrace
  local input="$BUFFER"
  [[ -z "$input" ]] && return

  local -a extra
  [[ -n "$QUASIMODO_SYSTEM" ]] && extra+=(--system "$QUASIMODO_SYSTEM")
  [[ -n "$QUASIMODO_HISTORY" ]] && extra+=(--history-file "$QUASIMODO_HISTORY")

  local tty_path="${TTY:-}"
  local tmp_output
  tmp_output="$(mktemp)" || { zle -M "quasimodo: mktemp failed"; return 0; }

  "$QUASIMODO_BIN" --prompt "$input" --bank "$QUASIMODO_BANK" "${extra[@]}" >"$tmp_output" 2>/dev/null &
  local cmd_pid=$!

  [[ -n "$tty_path" && -w "$tty_path" ]] && _quasimodo_spin "rewriting" "$cmd_pid" "$tty_path"
  wait "$cmd_pid"
  local cmd_status=$?

  local output
  output="$(<"$tmp_output")"
  rm -f "$tmp_output"

  (( cmd_status == 0 )) || {
    zle -M "quasimodo: check QUASIMODO_BIN/QUASIMODO_BANK and Ollama"
    return 0
  }
  [[ -z "$output" ]] && { zle -M "quasimodo: no suggestion returned"; return 0; }

  BUFFER="$output"
  CURSOR=${#BUFFER}
  zle -M "quasimodo: done"
  zle redisplay
}

# Ctrl+G to transform natural language into a command in-place (never auto-runs).
zle -N _quasimodo_ctrl_g
bindkey "$QUASIMODO_KEY" _quasimodo_ctrl_g
bindkey -M emacs "$QUASIMODO_KEY" _quasimodo_ctrl_g
bindkey -M viins "$QUASIMODO_KEY" _quasimodo_ctrl_g

if [[ -n "$QUASIMODO_ALT_KEY" ]]; then
  bindkey "$QUASIMODO_ALT_KEY" _quasimodo_ctrl_g
  bindkey -M emacs "$QUASIMODO_ALT_KEY" _quasimodo_ctrl_g
  bindkey -M viins "$QUASIMODO_ALT_KEY" _quasimodo_ctrl_g
fi

command_not_found_handler() {
  emulate -L zsh
  unsetopt xtrace
  local missing="$1"
  local suggestion
  suggestion="$($QUASIMODO_BIN --notfound "$missing" --bank "$QUASIMODO_BANK" 2>/dev/null)" || {
    _QUASIMODO_IN_ZERR=1
    return 127
  }
  [[ -n "$suggestion" ]] && print -P "%F{244}$suggestion%f"
  _QUASIMODO_IN_ZERR=1
  return 127
}

TRAPZERR() {
  emulate -L zsh
  setopt localoptions nomonitor nonotify
  unsetopt xtrace

  local code="$?"
  if (( _QUASIMODO_IN_ZERR )); then
    _QUASIMODO_IN_ZERR=0
    return 0
  fi

  local cmd="$history[$HISTCMD]"
  [[ -z "$cmd" ]] && return 0

  local tty_path="${TTY:-}"
  local tmp_output
  tmp_output="$(mktemp)" || return 0

  _QUASIMODO_IN_ZERR=1
  "$QUASIMODO_BIN" --explain "Command: $cmd -- Exit code: $code" >"$tmp_output" 2>/dev/null &
  local cmd_pid=$!

  [[ -n "$tty_path" && -w "$tty_path" ]] && _quasimodo_spin "explaining" "$cmd_pid" "$tty_path"
  wait "$cmd_pid"
  local cmd_status=$?
  _QUASIMODO_IN_ZERR=0

  local explain
  explain="$(<"$tmp_output")"
  rm -f "$tmp_output"

  (( cmd_status == 0 )) || return 0
  [[ -n "$explain" ]] && print -P "%F{244}$explain%f"

  return 0
}
