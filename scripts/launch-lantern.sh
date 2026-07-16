#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
FRONTEND_DIR="$ROOT/frontend/helix"
HELIX_BIN=${LANTERN_HELIX_BIN:-"$ROOT/.lantern/upstream/helix/target/release/hx"}
HELIX_RUNTIME=${LANTERN_HELIX_RUNTIME:-"$ROOT/.lantern/upstream/helix/runtime"}
LAZYGIT_BIN=${LANTERN_LAZYGIT_BIN:-"$ROOT/.lantern/toolchains/lazygit/lazygit"}
LAZYGIT_CONFIG="$FRONTEND_DIR/config/lazygit/config.yml"
RUNTIME_DIR="$ROOT/target/release"
DAEMON_BIN=${LANTERN_DAEMON_BIN:-"$RUNTIME_DIR/lantern-daemon"}
PANE_BIN=${LANTERN_PANE_BIN:-"$RUNTIME_DIR/lantern-terminal"}
PI_VERSION=0.80.6
detached=false
session=
runtime_dir=

cleanup_failed_launch() {
	if [[ -n $session ]] && tmux has-session -t "$session" 2>/dev/null; then
		tmux kill-session -t "$session"
	fi
	if [[ -n $runtime_dir && -d $runtime_dir ]]; then
		rm -f "$runtime_dir/selection.json" "$runtime_dir/selection.tmp" \
			"$runtime_dir/proposal.before" "$runtime_dir/proposal.after"
		rmdir "$runtime_dir" 2>/dev/null || true
	fi
}

trap cleanup_failed_launch ERR

if [[ ${1:-} == --detached ]]; then
	detached=true
	shift
fi

repo=${1:-$PWD}
repo=$(realpath "$repo")

if ! git -C "$repo" rev-parse --show-toplevel >/dev/null 2>&1; then
	echo "Lantern requires a Git repository: $repo" >&2
	exit 1
fi

for command in tmux git realpath awk; do
	if ! command -v "$command" >/dev/null; then
		echo "Required command is not installed: $command" >&2
		exit 1
	fi
done

PI_BIN=${LANTERN_PI_BIN:-$(command -v pi || true)}
if [[ -z $PI_BIN || ! -x $PI_BIN ]]; then
	echo "Pi $PI_VERSION is required for the explicit /agent driver." >&2
	echo "Install Pi, then authenticate privately with: pi  # use /login and choose OpenAI Codex" >&2
	exit 1
fi

installed_pi_version=$("$PI_BIN" -v 2>/dev/null || true)
if [[ $installed_pi_version != "$PI_VERSION" ]]; then
	echo "Lantern requires Pi $PI_VERSION; found '${installed_pi_version:-unknown}' at $PI_BIN" >&2
	exit 1
fi

if [[ ! -x $HELIX_BIN ]]; then
	echo "Pinned Helix build is missing: $HELIX_BIN" >&2
	echo "Build it with: cargo build --release --locked --manifest-path '$ROOT/.lantern/upstream/helix/Cargo.toml'" >&2
	exit 1
fi

if [[ ! -d $HELIX_RUNTIME ]]; then
	echo "Pinned Helix runtime is missing: $HELIX_RUNTIME" >&2
	exit 1
fi

if [[ ! -x $LAZYGIT_BIN ]]; then
	echo "Pinned Lazygit build is missing: $LAZYGIT_BIN" >&2
	exit 1
fi

if [[ ! -f $LAZYGIT_CONFIG ]]; then
	echo "Lantern Lazygit configuration is missing: $LAZYGIT_CONFIG" >&2
	exit 1
fi

if [[ ! -x $DAEMON_BIN || ! -x $PANE_BIN ]]; then
	echo "Lantern runtime is not built." >&2
	echo "Run: cargo build --release --locked --manifest-path '$ROOT/Cargo.toml'" >&2
	exit 1
fi

session="lantern-$$"
runtime_dir=$(mktemp -d "${TMPDIR:-/tmp}/lantern.XXXXXXXX")
selection_path="$runtime_dir/selection.json"
path="$FRONTEND_DIR/bin:$PATH"
editor_command=(env
	"PATH=$path"
	"XDG_CONFIG_HOME=$FRONTEND_DIR/config"
	"HELIX_RUNTIME=$HELIX_RUNTIME"
	"LANTERN_REPO=$repo"
	"LANTERN_SELECTION_PATH=$selection_path"
	"LANTERN_LAZYGIT_BIN=$LAZYGIT_BIN"
	"LANTERN_LAZYGIT_CONFIG=$LAZYGIT_CONFIG"
	"$HELIX_BIN"
	.)
printf -v editor_shell '%q ' "${editor_command[@]}"

# Detached validation has no client from which tmux can infer dimensions.
# Interactive attachment resizes this initial geometry to the real terminal.
editor_pane=$(tmux new-session -d -P -F '#{pane_id}' \
	-x "${COLUMNS:-160}" -y "${LINES:-48}" \
	-s "$session" -c "$repo" "$editor_shell")
tmux select-pane -t "$editor_pane" -T Helix

agent_command=(env
	"PATH=$path"
	"LANTERN_SESSION=$session"
	"LANTERN_EDITOR_PANE=$editor_pane"
	"LANTERN_REPO=$repo"
	"LANTERN_SELECTION_PATH=$selection_path"
	"LANTERN_LAZYGIT_BIN=$LAZYGIT_BIN"
	"LANTERN_LAZYGIT_CONFIG=$LAZYGIT_CONFIG"
	"LANTERN_DAEMON_BIN=$DAEMON_BIN"
	"LANTERN_PI_BIN=$PI_BIN"
	"LANTERN_PI_MODEL=${LANTERN_PI_MODEL:-gpt-5.4}"
	"LANTERN_MODEL_WORKDIR=$runtime_dir"
	"$PANE_BIN")
printf -v agent_shell '%q ' "${agent_command[@]}"

# tmux 3.4 accepts percentages through -l; older -p syntax is not portable.
# The agent spans the full terminal width below the editor.
agent_pane=$(tmux split-window -v -l '20%' -P -F '#{pane_id}' \
	-t "$editor_pane" -c "$repo" "$agent_shell")
tmux select-pane -t "$agent_pane" -T Lantern
tmux set-option -t "$session" pane-border-status off
tmux set-option -t "$session" pane-border-style 'fg=#5A3D6E,bg=#3A2A4D'
tmux set-option -t "$session" pane-active-border-style 'fg=#886C9C,bg=#3A2A4D'
tmux set-window-option -t "$session" window-style 'fg=#C7B8E0,bg=#3A2A4D'
tmux set-window-option -t "$session" window-active-style 'fg=#C7B8E0,bg=#3A2A4D'
tmux set-option -t "$session" mouse on
# tmux otherwise preserves the original absolute pane height after a client
# resize. Reassert the product's vertical 80/20 relationship on every resize.
tmux set-hook -t "$session" client-resized \
	"resize-pane -t '$agent_pane' -y '20%'"
tmux set-hook -t "$session" session-closed \
	"run-shell 'rm -f "$selection_path" "$selection_path.tmp" "$runtime_dir/proposal.before" "$runtime_dir/proposal.after"; rmdir "$runtime_dir" 2>/dev/null || true'"
trap - ERR

if $detached; then
	printf '%s\n' "$session"
else
	exec tmux attach-session -t "$session"
fi
