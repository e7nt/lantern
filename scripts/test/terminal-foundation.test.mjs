import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const frontendBin = path.join(root, 'frontend/helix/bin');

async function fixture() {
	const directory = await mkdtemp(path.join(tmpdir(), 'lantern-helix-'));
	const repository = path.join(directory, 'repository');
	const fakeBin = path.join(directory, 'bin');
	const tmuxLog = path.join(directory, 'tmux.log');
	const submitLog = path.join(directory, 'submit.log');
	await mkdir(repository);
	await mkdir(fakeBin);
	await writeFile(path.join(repository, 'source file.rs'), 'fn main() {}\n');
	await writeFile(
		path.join(fakeBin, 'tmux'),
		'#!/bin/sh\n' +
			'if [ "$1" = display-message ]; then\n' +
			'  printf "%s\\n" "$TMUX_CLIENT_WIDTH"\n' +
			'elif [ "$1" = list-panes ]; then\n' +
			'  printf "%%7 Helix\\n%%8 Lantern\\n"\n' +
			'else\n' +
			'  printf "%s\\n" "$*" >> "$TMUX_LOG"\n' +
			'  if [ "$1" = display-popup ] && [ "$TMUX_EXPAND_ON_COMPACT" = 1 ]; then case "$*" in *LANTERN_GIT_LAYOUT=compact*) exit 21;; esac; fi\n' +
			'  if [ "$1" = display-popup ] && [ -n "$TMUX_POPUP_STATUS" ]; then exit "$TMUX_POPUP_STATUS"; fi\n' +
			'fi\n'
	);
	await chmod(path.join(fakeBin, 'tmux'), 0o755);
	await writeFile(
		path.join(fakeBin, 'lantern-submit'),
		'#!/bin/sh\nprintf "%s\\n" "$*" > "$LANTERN_SUBMIT_ARGS_LOG"\ncat > "$LANTERN_SUBMIT_LOG"\n'
	);
	await chmod(path.join(fakeBin, 'lantern-submit'), 0o755);
	return { directory, repository, fakeBin, tmuxLog, submitLog };
}

function environment({ directory, repository, fakeBin, tmuxLog, submitLog }) {
	return {
		...process.env,
		PATH: `${fakeBin}:${process.env.PATH}`,
		LANTERN_REPO: repository,
		LANTERN_REVIEW_PATH: path.join(directory, 'review.json'),
		LANTERN_GIT_RESUME_PATH: path.join(directory, 'git-resume.json'),
		LANTERN_GIT_FOCUS_PATH: path.join(directory, 'git-focus.json'),
		LANTERN_EDITOR_PANE: '%7',
		TMUX: '/tmp/fake',
		TMUX_CLIENT_WIDTH: '160',
		TMUX_LOG: tmuxLog,
		LANTERN_CONTROL_SOCKET: path.join(repository, 'control.sock'),
		LANTERN_SUBMIT_BIN: path.join(fakeBin, 'lantern-submit'),
		LANTERN_SUBMIT_LOG: submitLog,
		LANTERN_SUBMIT_ARGS_LOG: path.join(directory, 'submit-args.log')
	};
}

test('range navigation sends one validated Helix-native command', async () => {
	const context = await fixture();
	const result = spawnSync(
		path.join(frontendBin, 'lantern-open-range'),
		['source file.rs', '1', '4', '1', '8'],
		{ encoding: 'utf8', env: environment(context) }
	);

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /send-keys -t %7 Escape/);
	assert.match(calls, /send-keys -l -t %7 lantern-navigate 'source file\.rs' 1 4 1 8/);
	assert.doesNotMatch(calls, /select-pane/);
});

test('navigation rejects a repository escape before contacting tmux', async () => {
	const context = await fixture();
	const result = spawnSync(
		path.join(frontendBin, 'lantern-open-range'),
		['../outside.rs', '1', '1', '1', '2'],
		{ encoding: 'utf8', env: environment(context) }
	);

	assert.equal(result.status, 2);
	assert.match(result.stderr, /outside the repository/);
});

test('focused Git is constrained to a 10 percent rail above the agent', async () => {
	const context = await fixture();
	const gitRail = path.join(context.directory, 'lantern-git-rail');
	await writeFile(gitRail, '#!/bin/sh\nexit 0\n');
	await chmod(gitRail, 0o755);

	const result = spawnSync(path.join(frontendBin, 'lantern-git'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			LANTERN_GIT_BIN: gitRail
		}
	});

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /display-popup -E -w 10% -h 80% -x 0 -y 0/);
	assert.ok(calls.includes(context.repository));
	assert.ok(calls.includes(gitRail));
	assert.ok(calls.includes(environment(context).LANTERN_GIT_FOCUS_PATH));
	assert.ok(calls.includes(environment(context).LANTERN_CONTROL_SOCKET));
	assert.match(calls, /LANTERN_GIT_LAYOUT=compact/);
});

test('opening a diff expands Git across the upper editor region', async () => {
	const context = await fixture();
	const gitRail = path.join(context.directory, 'lantern-git-rail');
	await writeFile(gitRail, '#!/bin/sh\nexit 0\n');
	await chmod(gitRail, 0o755);

	const result = spawnSync(path.join(frontendBin, 'lantern-git'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			LANTERN_GIT_BIN: gitRail,
			TMUX_EXPAND_ON_COMPACT: '1'
		}
	});

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /display-popup -E -w 10% -h 80% -x 0 -y 0/);
	assert.match(calls, /display-popup -E -w 80% -h 80% -x 10% -y 0/);
	assert.match(calls, /LANTERN_GIT_LAYOUT=review/);
});

test('focused Git rejects a terminal too narrow for the 10 percent rail', async () => {
	const context = await fixture();
	const gitRail = path.join(context.directory, 'lantern-git-rail');
	await writeFile(gitRail, '#!/bin/sh\nexit 0\n');
	await chmod(gitRail, 0o755);

	const result = spawnSync(path.join(frontendBin, 'lantern-git'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			LANTERN_GIT_BIN: gitRail,
			TMUX_CLIENT_WIDTH: '119'
		}
	});

	assert.equal(result.status, 1);
	assert.match(result.stderr, /at least 120 columns wide/);
	const calls = await readFile(context.tmuxLog, 'utf8').catch(() => '');
	assert.doesNotMatch(calls, /display-popup/);
});

test('Ctrl-a composer opens a small contextual popup', async () => {
	const context = await fixture();
	const result = spawnSync(path.join(frontendBin, 'lantern-agent-composer'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			TMUX_PANE: '%7',
			LANTERN_SELECTION_PATH: path.join(context.directory, 'selection.json')
		}
	});

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /display-popup -t %7 -E -w 70% -h 8 -T  Ask Lantern /);
	assert.match(calls, /LANTERN_AGENT_PANE=%8/);
	assert.match(calls, /LANTERN_REVIEW_PATH=/);
});

test('Git review exit opens the one existing agent composer', async () => {
	const context = await fixture();
	const gitRail = path.join(context.directory, 'lantern-git-rail');
	const composerLog = path.join(context.directory, 'composer.log');
	await writeFile(gitRail, '#!/bin/sh\nexit 0\n');
	await chmod(gitRail, 0o755);
	await writeFile(
		path.join(context.fakeBin, 'lantern-agent-composer'),
		'#!/bin/sh\nprintf "opened\\n" > "$COMPOSER_LOG"\n'
	);
	await chmod(path.join(context.fakeBin, 'lantern-agent-composer'), 0o755);

	const result = spawnSync(path.join(frontendBin, 'lantern-git'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			LANTERN_GIT_BIN: gitRail,
			TMUX_POPUP_STATUS: '20',
			COMPOSER_LOG: composerLog
		}
	});

	assert.equal(result.status, 0, result.stderr);
	assert.equal(await readFile(composerLog, 'utf8'), 'opened\n');
});

test('composer submits the question literally and focuses Lantern', async () => {
	const context = await fixture();
	const question = 'explain $(touch /tmp/never) and ; literally';
	const result = spawnSync(
		path.join(frontendBin, 'lantern-agent-composer'),
		['--prompt'],
		{
			encoding: 'utf8',
			input: `${question}\n`,
			env: {
				...environment(context),
				TMUX_PANE: '%7',
				LANTERN_AGENT_PANE: '%8'
			}
		}
	);

	assert.equal(result.status, 0, result.stderr);
	assert.match(result.stdout, /Ask about this repository/);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.equal(await readFile(context.submitLog, 'utf8'), question);
	assert.doesNotMatch(calls, /send-keys/);
	assert.match(calls, /select-pane -t %8/);
});

test('composer can queue one plan comment without asking the model', async () => {
	const context = await fixture();
	const comment = 'Keep this task outside the first release.';
	const result = spawnSync(
		path.join(frontendBin, 'lantern-agent-composer'),
		['--prompt'],
		{
			encoding: 'utf8',
			input: `${comment} __LANTERN_PLAN_COMMENT__\n`,
			env: {
				...environment(context),
				LANTERN_AGENT_PANE: '%8'
			}
		}
	);

	assert.equal(result.status, 0, result.stderr);
	assert.equal(await readFile(context.submitLog, 'utf8'), comment);
	assert.equal(
		await readFile(path.join(context.directory, 'submit-args.log'), 'utf8'),
		'--plan-comment\n'
	);
});

test('dismissing the Git composer clears its one-shot review context', async () => {
	const context = await fixture();
	const reviewPath = path.join(context.directory, 'review.json');
	await writeFile(reviewPath, '{"bounded":"context"}\n');
	const result = spawnSync(
		path.join(frontendBin, 'lantern-agent-composer'),
		['--prompt'],
		{
			encoding: 'utf8',
			input: '\n',
			env: {
				...environment(context),
				LANTERN_AGENT_PANE: '%8',
				LANTERN_REVIEW_PATH: reviewPath
			}
		}
	);

	assert.equal(result.status, 0, result.stderr);
	await assert.rejects(readFile(reviewPath, 'utf8'), /ENOENT/);
});

test('agent zoom toggles the same Lantern pane without rebuilding the layout', async () => {
	const context = await fixture();
	const result = spawnSync(path.join(frontendBin, 'lantern-toggle-agent'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			TMUX_PANE: '%7'
		}
	});

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /resize-pane -Z -t %8/);
	assert.match(calls, /select-pane -t %8/);
});

test('terminal surfaces declare one mouse-enabled interaction contract', async () => {
	const helixConfig = await readFile(
		path.join(root, 'frontend/helix/config/helix/config.toml'),
		'utf8'
	);
	const gitRail = await readFile(
		path.join(root, 'apps/git-rail/src/main.rs'),
		'utf8'
	);
	const launcher = await readFile(path.join(root, 'scripts/launch-lantern.sh'), 'utf8');

	assert.match(helixConfig, /mouse = true/);
	assert.match(helixConfig, /theme = "lantern"/);
	assert.match(helixConfig, /C-a = \[":lantern-export-symbol-context", ":run-shell-command lantern-agent-composer"\]/);
	assert.match(helixConfig, /F2 = ":run-shell-command lantern-toggle-agent"/);
	assert.match(gitRail, /EnableMouseCapture/);
	assert.match(gitRail, /"conflict"/);
	assert.match(gitRail, /"modified"/);
	assert.match(launcher, /set-option -t "\$session" mouse on/);
	assert.match(launcher, /set-option -t "\$session" status off/);
	assert.match(launcher, /pane-border-status off/);
	assert.match(launcher, /bin\/lantern-cleanup-session/);
	assert.match(launcher, /window-style 'fg=#C7B8E0,bg=#3A2A4D'/);
});
