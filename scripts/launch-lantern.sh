#!/usr/bin/env bash

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION_FILE=${LANTERN_VERSION_FILE:-"$ROOT/VERSION"}
FRONTEND_DIR="$ROOT/frontend/helix"
HELIX_BIN=${LANTERN_HELIX_BIN:-"$ROOT/.lantern/upstream/helix/target/release/hx"}
HELIX_RUNTIME=${LANTERN_HELIX_RUNTIME:-"$ROOT/.lantern/upstream/helix/runtime"}
RUNTIME_DIR="$ROOT/target/release"
GIT_BIN=${LANTERN_GIT_BIN:-"$RUNTIME_DIR/lantern-git-rail"}
DAEMON_BIN=${LANTERN_DAEMON_BIN:-"$RUNTIME_DIR/lantern-daemon"}
PANE_BIN=${LANTERN_PANE_BIN:-"$RUNTIME_DIR/lantern-terminal"}
SUBMIT_BIN=${LANTERN_SUBMIT_BIN:-"$RUNTIME_DIR/lantern-submit"}
SEMANTIC_WORKER=${LANTERN_SEMANTIC_WORKER:-"$ROOT/scripts/run-semantic-worker.sh"}
PI_VERSION=0.80.6
detached=false
session=
runtime_dir=

print_help() {
	cat <<'EOF'
Lantern is an AI coding environment for developers who love to understand and write code.
It keeps you in the editor while you ask questions, shape changes, and review the result.

Usage:
  lantern [repository]   Open a Git repository (defaults to the current directory)
  lantern help           Show this guide
  lantern --version      Show the installed version

Inside Lantern:
  Ctrl-a    Ask about the repository or selected code
  F2        Expand or restore the agent conversation
  Space-g   Open changes for review and staging
  Esc       Interrupt the active agent turn
  Ctrl-d    Quit from an empty, idle agent prompt

Use natural language. Ask what code does, request a change, or comment directly
on a diff; Lantern keeps the code and your review at the center of the work.
EOF
}

if [[ ${1:-} == help || ${1:-} == -h || ${1:-} == --help ]]; then
	print_help
	exit 0
fi

if [[ ${1:-} == --version ]]; then
	if [[ ! -f $VERSION_FILE ]]; then
		echo "Lantern version metadata is missing: $VERSION_FILE" >&2
		exit 1
	fi
	printf 'lantern %s\n' "$(<"$VERSION_FILE")"
	exit 0
fi

cleanup_failed_launch() {
	if [[ -n $session ]] && tmux has-session -t "$session" 2>/dev/null; then
		tmux kill-session -t "$session"
	fi
	if [[ -n $runtime_dir && -d $runtime_dir ]]; then
		rm -f "$runtime_dir/selection.json" "$runtime_dir/selection.tmp" \
			"$runtime_dir/review.json" "$runtime_dir/review.tmp" \
			"$runtime_dir/git-resume.json" "$runtime_dir/git-resume.tmp" "$runtime_dir/control.sock" \
			"$runtime_dir/git-focus.json" "$runtime_dir/git-focus.tmp" \
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
	echo "Pi $PI_VERSION is required for Lantern's agent." >&2
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

if [[ ! -x $DAEMON_BIN || ! -x $PANE_BIN || ! -x $SUBMIT_BIN || ! -x $GIT_BIN ]]; then
	echo "Lantern runtime is not built." >&2
	echo "Run: cargo build --release --locked --manifest-path '$ROOT/Cargo.toml'" >&2
	exit 1
fi

if [[ ! -x $SEMANTIC_WORKER ]]; then
	echo "Lantern semantic worker is missing: $SEMANTIC_WORKER" >&2
	echo "Run: $ROOT/frontend/helix/prepare.sh" >&2
	exit 1
fi

session="lantern-$$"
runtime_dir=$(mktemp -d "${TMPDIR:-/tmp}/lantern.XXXXXXXX")
selection_path="$runtime_dir/selection.json"
review_path="$runtime_dir/review.json"
git_resume_path="$runtime_dir/git-resume.json"
git_focus_path="$runtime_dir/git-focus.json"
control_socket="$runtime_dir/control.sock"
path="$FRONTEND_DIR/bin:$PATH"
editor_command=(env
	"PATH=$path"
	"XDG_CONFIG_HOME=$FRONTEND_DIR/config"
	"HELIX_RUNTIME=$HELIX_RUNTIME"
	"LANTERN_REPO=$repo"
	"LANTERN_SELECTION_PATH=$selection_path"
	"LANTERN_REVIEW_PATH=$review_path"
	"LANTERN_GIT_RESUME_PATH=$git_resume_path"
	"LANTERN_GIT_FOCUS_PATH=$git_focus_path"
	"LANTERN_CONTROL_SOCKET=$control_socket"
	"LANTERN_SUBMIT_BIN=$SUBMIT_BIN"
	"LANTERN_GIT_BIN=$GIT_BIN"
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
	"LANTERN_REVIEW_PATH=$review_path"
	"LANTERN_GIT_RESUME_PATH=$git_resume_path"
	"LANTERN_GIT_FOCUS_PATH=$git_focus_path"
	"LANTERN_CONTROL_SOCKET=$control_socket"
	"LANTERN_GIT_BIN=$GIT_BIN"
	"LANTERN_DAEMON_BIN=$DAEMON_BIN"
	"LANTERN_PI_BIN=$PI_BIN"
	"LANTERN_PI_MODEL=${LANTERN_PI_MODEL:-gpt-5.4}"
	"LANTERN_SEMANTIC_WORKER=$SEMANTIC_WORKER"
	"$PANE_BIN")
printf -v agent_shell '%q ' "${agent_command[@]}"

# tmux 3.4 accepts percentages through -l; older -p syntax is not portable.
# The agent spans the full terminal width below the editor.
agent_pane=$(tmux split-window -v -l '20%' -P -F '#{pane_id}' \
	-t "$editor_pane" -c "$repo" "$agent_shell")
tmux select-pane -t "$agent_pane" -T Lantern
tmux set-option -t "$session" status off
tmux set-option -t "$session" pane-border-status off
tmux set-option -t "$session" pane-border-style 'fg=#3A2A4D,bg=#3A2A4D'
tmux set-option -t "$session" pane-active-border-style 'fg=#5A3D6E,bg=#3A2A4D'
tmux set-window-option -t "$session" window-style 'fg=#C7B8E0,bg=#3A2A4D'
tmux set-window-option -t "$session" window-active-style 'fg=#C7B8E0,bg=#3A2A4D'
tmux set-option -t "$session" mouse on
# tmux otherwise preserves the original absolute pane height after a client
# resize. Reassert the product's vertical 80/20 relationship on every resize.
tmux set-hook -t "$session" client-resized \
	"resize-pane -t '$agent_pane' -y '20%'"
"$FRONTEND_DIR/bin/lantern-cleanup-session" "$session" "$runtime_dir" \
	</dev/null >/dev/null 2>&1 &
trap - ERR

if $detached; then
	printf '%s\n' "$session"
else
	exec tmux attach-session -t "$session"
fi
