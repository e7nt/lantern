#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import {
	copyFileSync,
	mkdtempSync,
	mkdirSync,
	readFileSync,
	rmSync,
	statSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { performance } from 'node:perf_hooks';

const ROOT = resolve(import.meta.dirname, '..');
const RAIL = resolve(process.env.LANTERN_GIT_RAIL_BIN ?? join(ROOT, 'spikes/git-rail/target/release/lantern-git-rail-spike'));
const LAZYGIT = resolve(process.env.LANTERN_LAZYGIT_BIN ?? join(ROOT, '.lantern/toolchains/lazygit/lazygit'));
const LAZYGIT_CONFIG = resolve(process.env.LANTERN_LAZYGIT_CONFIG ?? join(ROOT, 'frontend/helix/config/lazygit/config.yml'));
const TMUX_SOCKET = `lantern-git-benchmark-${process.pid}`;
const WIDTH = 120;
const HEIGHT = 40;
const STARTUP_SAMPLES = 6;
const TIMEOUT_MS = 15_000;

function run(file, args, options = {}) {
	const result = spawnSync(file, args, { encoding: 'utf8', ...options });
	if (result.status !== 0) {
		throw new Error(`${basename(file)} failed (${result.status}): ${(result.stderr || result.stdout).trim()}`);
	}
	return result.stdout;
}

function sleep(milliseconds) {
	Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function percentile(values, fraction) {
	const ordered = [...values].sort((left, right) => left - right);
	return ordered[Math.min(ordered.length - 1, Math.floor((ordered.length - 1) * fraction))];
}

function summarize(values) {
	return {
		median: Math.round(percentile(values, 0.5) * 10) / 10,
		p95: Math.round(percentile(values, 0.95) * 10) / 10,
		min: Math.round(Math.min(...values) * 10) / 10,
		max: Math.round(Math.max(...values) * 10) / 10,
	};
}

function git(repository, args) {
	run('git', args, { cwd: repository, stdio: ['ignore', 'pipe', 'pipe'] });
}

function createFixture(root) {
	const repository = join(root, 'repository');
	mkdirSync(repository);
	git(repository, ['init', '-q', '-b', 'main']);
	git(repository, ['config', 'user.name', 'Lantern Benchmark']);
	git(repository, ['config', 'user.email', 'benchmark@example.com']);
	for (let index = 0; index < 1_000; index += 1) {
		const directory = join(repository, 'src', `module-${Math.floor(index / 100)}`);
		mkdirSync(directory, { recursive: true });
		writeFileSync(join(directory, `file-${String(index).padStart(4, '0')}.ts`), `export const value${index} = ${index};\n`);
	}
	writeFileSync(join(repository, 'conflict.txt'), 'base\n');
	git(repository, ['add', '.']);
	git(repository, ['commit', '-qm', 'benchmark base']);
	for (let index = 0; index < 6; index += 1) {
		const path = join(repository, 'src', `module-${Math.floor(index / 100)}`, `file-${String(index).padStart(4, '0')}.ts`);
		writeFileSync(path, `export const value${index} = ${index + 1};\n`);
	}
	git(repository, ['add', ...Array.from({ length: 3 }, (_, index) => `src/module-0/file-${String(index).padStart(4, '0')}.ts`)]);
	for (let index = 0; index < 2; index += 1) {
		writeFileSync(join(repository, `untracked-${index}.txt`), `untracked ${index}\n`);
	}
	return repository;
}

function tmux(args, options = {}) {
	return run('tmux', ['-L', TMUX_SOCKET, ...args], options);
}

function capture(session) {
	return tmux(['capture-pane', '-p', '-t', `${session}:0.0`]);
}

function waitFor(label, operation, predicate) {
	const started = performance.now();
	let lastValue;
	for (;;) {
		const value = operation();
		lastValue = value;
		if (predicate(value)) return { elapsedMs: performance.now() - started, value };
		if (performance.now() - started >= TIMEOUT_MS) {
			const frame = typeof lastValue === 'string' ? `\n${lastValue.slice(0, 2_000)}` : '';
			throw new Error(`${label} exceeded ${TIMEOUT_MS} ms${frame}`);
		}
		sleep(5);
	}
}

function processTree(rootPid) {
	const pending = [rootPid];
	const seen = new Set();
	while (pending.length > 0) {
		const pid = pending.pop();
		if (seen.has(pid)) continue;
		seen.add(pid);
		try {
			const children = readFileSync(`/proc/${pid}/task/${pid}/children`, 'utf8').trim();
			if (children) pending.push(...children.split(/\s+/).map(Number));
		} catch {}
	}
	return [...seen];
}

function memoryKilobytes(rootPid) {
	return processTree(rootPid).reduce((total, pid) => {
		try {
			const match = readFileSync(`/proc/${pid}/status`, 'utf8').match(/^VmRSS:\s+(\d+)\s+kB$/m);
			return total + (match ? Number(match[1]) : 0);
		} catch {
			return total;
		}
	}, 0);
}

function cpuTicks(rootPid) {
	return processTree(rootPid).reduce((total, pid) => {
		try {
			const fields = readFileSync(`/proc/${pid}/stat`, 'utf8').trim().split(' ');
			return total + Number(fields[13]) + Number(fields[14]);
		} catch {
			return total;
		}
	}, 0);
}

function command(kind, repository, home, config) {
	if (kind === 'rail') return `exec env HOME=${JSON.stringify(home)} LANTERN_REPO=${JSON.stringify(repository)} ${JSON.stringify(RAIL)}`;
	return `exec env HOME=${JSON.stringify(home)} ${JSON.stringify(LAZYGIT)} --use-config-file ${JSON.stringify(config)}`;
}

function ready(kind, frame) {
	return kind === 'rail'
		? frame.includes('Git  main') && frame.includes('modified')
		: frame.includes('Files - Worktrees - Submodules') && frame.includes('Local branches');
}

let sequence = 0;
function measureLaunch(kind, repository, home, config, probeInteractions) {
	const session = `${kind}-${sequence++}`;
	const started = performance.now();
	tmux(['new-session', '-d', '-s', session, '-x', String(WIDTH), '-y', String(HEIGHT), '-c', repository, command(kind, repository, home, config)]);
	const startup = waitFor(`${kind} startup`, () => capture(session), (frame) => ready(kind, frame));
	const startupMs = performance.now() - started;
	const pid = Number(tmux(['display-message', '-p', '-t', `${session}:0.0`, '#{pane_pid}']).trim());
	sleep(500);
	const rssKb = memoryKilobytes(pid);
	const ticksBefore = cpuTicks(pid);
	sleep(1_000);
	const idleCpuTicks = Math.max(0, cpuTicks(pid) - ticksBefore);
	let inputMs;
	let refreshMs;
	if (probeInteractions) {
		const before = startup.value;
		const inputStarted = performance.now();
		tmux(['send-keys', '-t', `${session}:0.0`, '?']);
		waitFor(`${kind} input`, () => capture(session), (frame) => frame !== before);
		inputMs = performance.now() - inputStarted;
		tmux(['send-keys', '-t', `${session}:0.0`, 'Escape']);
		sleep(100);
		const externalName = `000-external-refresh-${kind}.txt`;
		const refreshStarted = performance.now();
		writeFileSync(join(repository, externalName), 'external\n');
		waitFor(`${kind} refresh`, () => capture(session), (frame) => frame.includes(externalName));
		refreshMs = performance.now() - refreshStarted;
		rmSync(join(repository, externalName));
	}
	tmux(['send-keys', '-t', `${session}:0.0`, 'q']);
	sleep(50);
	spawnSync('tmux', ['-L', TMUX_SOCKET, 'kill-session', '-t', session]);
	return { startupMs, markerMs: startup.elapsedMs, rssKb, idleCpuTicks, inputMs, refreshMs };
}

function main() {
	for (const path of [RAIL, LAZYGIT, LAZYGIT_CONFIG]) statSync(path);
	const directory = mkdtempSync(join(tmpdir(), 'lantern-git-surface-benchmark-'));
	try {
		const repository = createFixture(directory);
		const home = join(directory, 'home');
		mkdirSync(home);
		const config = join(directory, 'lazygit.yml');
		copyFileSync(LAZYGIT_CONFIG, config);
		writeFileSync(config, `${readFileSync(config, 'utf8')}\ndisableStartupPopups: true\n`);
		spawnSync('tmux', ['-L', TMUX_SOCKET, 'kill-server']);
		const samples = { rail: [], lazygit: [] };
		for (let index = 0; index < STARTUP_SAMPLES; index += 1) {
			for (const kind of index % 2 === 0 ? ['rail', 'lazygit'] : ['lazygit', 'rail']) {
				samples[kind].push(measureLaunch(kind, repository, home, config, index === STARTUP_SAMPLES - 1));
			}
		}
		const result = {
			formatVersion: 1,
			timestamp: new Date().toISOString(),
			environment: {
				platform: process.platform,
				arch: process.arch,
				node: process.version,
				tmux: run('tmux', ['-V']).trim(),
				git: run('git', ['--version']).trim(),
				width: WIDTH,
				height: HEIGHT,
				trackedFiles: 1_001,
			},
			binaries: {
				rail: { path: RAIL, bytes: statSync(RAIL).size },
				lazygit: { path: LAZYGIT, bytes: statSync(LAZYGIT).size, revision: '080da5cacfcff63a89ea23493bb91b11b0612876' },
			},
			raw: samples,
			summary: Object.fromEntries(Object.entries(samples).map(([kind, values]) => [kind, {
				startupMs: summarize(values.map((sample) => sample.startupMs)),
				rssKb: summarize(values.map((sample) => sample.rssKb)),
				idleCpuTicks: summarize(values.map((sample) => sample.idleCpuTicks)),
				inputMs: values.at(-1).inputMs,
				refreshMs: values.at(-1).refreshMs,
			}]))
		};
		result.passed = result.summary.rail.startupMs.median < result.summary.lazygit.startupMs.median
			&& result.summary.rail.rssKb.median < result.summary.lazygit.rssKb.median
			&& result.summary.rail.refreshMs <= 1_500;
		process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
	} finally {
		spawnSync('tmux', ['-L', TMUX_SOCKET, 'kill-server']);
		rmSync(directory, { recursive: true, force: true });
	}
}

try {
	main();
} catch (error) {
	spawnSync('tmux', ['-L', TMUX_SOCKET, 'kill-server']);
	process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
	process.exitCode = 1;
}
