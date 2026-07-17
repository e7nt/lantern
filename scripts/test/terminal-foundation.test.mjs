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
			'fi\n'
	);
	await chmod(path.join(fakeBin, 'tmux'), 0o755);
	return { directory, repository, fakeBin, tmuxLog };
}

function environment({ repository, fakeBin, tmuxLog }) {
	return {
		...process.env,
		PATH: `${fakeBin}:${process.env.PATH}`,
		LANTERN_REPO: repository,
		LANTERN_EDITOR_PANE: '%7',
		TMUX: '/tmp/fake',
		TMUX_CLIENT_WIDTH: '160',
		TMUX_LOG: tmuxLog
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

test('Lazygit is constrained to a 10 percent rail above the agent', async () => {
	const context = await fixture();
	const lazygit = path.join(context.directory, 'lazygit');
	const lazygitConfig = path.join(context.directory, 'lazygit.yml');
	await writeFile(lazygit, '#!/bin/sh\nexit 0\n');
	await writeFile(lazygitConfig, 'gui:\n  mouseEvents: true\n');
	await chmod(lazygit, 0o755);

	const result = spawnSync(path.join(frontendBin, 'lantern-lazygit'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			LANTERN_LAZYGIT_BIN: lazygit,
			LANTERN_LAZYGIT_CONFIG: lazygitConfig
		}
	});

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /display-popup -E -w 10% -h 80% -x 0 -y 0/);
	assert.ok(calls.includes(context.repository));
	assert.ok(calls.includes(`--use-config-file ${lazygitConfig}`));
});

test('Lazygit rejects a terminal too narrow for the 10 percent rail', async () => {
	const context = await fixture();
	const lazygit = path.join(context.directory, 'lazygit');
	const lazygitConfig = path.join(context.directory, 'lazygit.yml');
	await writeFile(lazygit, '#!/bin/sh\nexit 0\n');
	await writeFile(lazygitConfig, 'gui:\n  mouseEvents: true\n');
	await chmod(lazygit, 0o755);

	const result = spawnSync(path.join(frontendBin, 'lantern-lazygit'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			LANTERN_LAZYGIT_BIN: lazygit,
			LANTERN_LAZYGIT_CONFIG: lazygitConfig,
			TMUX_CLIENT_WIDTH: '119'
		}
	});

	assert.equal(result.status, 1);
	assert.match(result.stderr, /at least 120 columns wide/);
	const calls = await readFile(context.tmuxLog, 'utf8').catch(() => '');
	assert.doesNotMatch(calls, /display-popup/);
});

test('Ctrl-a focus bridge selects the Lantern pane', async () => {
	const context = await fixture();
	const result = spawnSync(path.join(frontendBin, 'lantern-focus-agent'), [], {
		encoding: 'utf8',
		env: {
			...environment(context),
			TMUX_PANE: '%7'
		}
	});

	assert.equal(result.status, 0, result.stderr);
	const calls = await readFile(context.tmuxLog, 'utf8');
	assert.match(calls, /select-pane -t %8/);
});

test('terminal surfaces declare one mouse-enabled interaction contract', async () => {
	const helixConfig = await readFile(
		path.join(root, 'frontend/helix/config/helix/config.toml'),
		'utf8'
	);
	const lazygitConfig = await readFile(
		path.join(root, 'frontend/helix/config/lazygit/config.yml'),
		'utf8'
	);
	const launcher = await readFile(path.join(root, 'scripts/launch-lantern.sh'), 'utf8');

	assert.match(helixConfig, /mouse = true/);
	assert.match(helixConfig, /theme = "lantern"/);
	assert.match(helixConfig, /C-a = \[":lantern-export-symbol-context"/);
	assert.match(lazygitConfig, /mouseEvents: true/);
	assert.match(lazygitConfig, /activeBorderColor:\s+[^#]*"#C7B8E0"/s);
	assert.match(lazygitConfig, /selectedLineBgColor:\s+[^#]*"#47345E"/s);
	assert.match(launcher, /set-option -t "\$session" mouse on/);
	assert.match(launcher, /set-option -t "\$session" status off/);
	assert.match(launcher, /pane-border-status off/);
	assert.match(launcher, /window-style 'fg=#C7B8E0,bg=#3A2A4D'/);
});
