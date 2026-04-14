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

_quasimodo_ctrl_g() {
  emulate -L zsh
  unsetopt xtrace
  local input="$BUFFER"
  [[ -z "$input" ]] && return

  local -a extra
  [[ -n "$QUASIMODO_SYSTEM" ]] && extra+=(--system "$QUASIMODO_SYSTEM")
  [[ -n "$QUASIMODO_HISTORY" ]] && extra+=(--history-file "$QUASIMODO_HISTORY")

  local output
  zle -M "quasimodo: rewriting..."
  output="$($QUASIMODO_BIN --prompt "$input" --bank "$QUASIMODO_BANK" "${extra[@]}" 2>/dev/null)" || {
    zle -M "quasimodo: check QUASIMODO_BIN/QUASIMODO_BANK and Ollama"
    return 0
  }
  [[ -z "$output" ]] && {
    zle -M "quasimodo: no suggestion returned"
    return 0
  }

  BUFFER="$output"
  CURSOR=${#BUFFER}
  zle -M "quasimodo: rewrite ready"
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
  suggestion="$($QUASIMODO_BIN --notfound "$missing" --bank "$QUASIMODO_BANK" 2>/dev/null)" || return 127
  [[ -n "$suggestion" ]] && print -P "%F{244}$suggestion%f"
  return 127
}

TRAPZERR() {
  emulate -L zsh
  unsetopt xtrace

  local code="$?"
  (( _QUASIMODO_IN_ZERR )) && return 0

  local cmd="$history[$HISTCMD]"
  [[ -z "$cmd" ]] && return 0

  local explain
  print -P "%F{244}quasimodo: explaining...%f"
  _QUASIMODO_IN_ZERR=1
  explain="$($QUASIMODO_BIN --explain "Command: $cmd -- Exit code: $code" 2>/dev/null)"
  _QUASIMODO_IN_ZERR=0
  [[ -n "$explain" ]] && print -P "%F{244}$explain%f"

  return 0
}
