#!/usr/bin/env node

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { performance } from 'node:perf_hooks';

import { createPiSdkAdapter } from '../spikes/agent-runtime/pi-sdk-adapter.mjs';

const TURN_TIMEOUT_MS = 45_000;
const ABORT_TARGET_MS = 1_000;

function deferred() {
	let resolve;
	const promise = new Promise((done) => { resolve = done; });
	return { promise, resolve };
}

async function within(promise, milliseconds, operation) {
	let timer;
	try {
		return await Promise.race([
			promise,
			new Promise((_, reject) => {
				timer = setTimeout(() => reject(new Error(`${operation} exceeded ${milliseconds} ms`)), milliseconds);
			}),
		]);
	} finally {
		clearTimeout(timer);
	}
}

async function main() {
	const repository = mkdtempSync(join(tmpdir(), 'lantern-pi-sdk-streaming-'));
	writeFileSync(join(repository, 'sample.py'), 'def lantern_marker():\n    return "grounded"\n', 'utf8');
	const initializedAt = performance.now();
	const { adapter, version } = await createPiSdkAdapter({
		cwd: repository,
		tools: ['read'],
		systemPrompt: 'Use the read tool before answering code questions. Be accurate and concise.',
	});
	const initMs = Math.round(performance.now() - initializedAt);
	const events = [];
	let startedAt = 0;
	let firstActivityMs;
	let firstTextMs;
	const firstText = deferred();
	const unsubscribe = adapter.subscribe((event) => {
		events.push(event);
		const elapsed = Math.round(performance.now() - startedAt);
		firstActivityMs ??= elapsed;
		if (event.type === 'text_delta') {
			firstTextMs ??= elapsed;
			firstText.resolve();
		}
	});

	try {
		startedAt = performance.now();
		await within(
			adapter.prompt('Read sample.py and state what lantern_marker returns.'),
			TURN_TIMEOUT_MS,
			'grounded streaming turn',
		);
		for (const required of ['tool_started', 'tool_finished', 'text_delta', 'turn_settled']) {
			if (!events.some((event) => event.type === required)) throw new Error(`missing ${required} event`);
		}
		if (events.at(-1)?.outcome !== 'completed') throw new Error('streaming turn did not complete cleanly');
		const groundedFirstTextMs = firstTextMs;

		events.length = 0;
		firstTextMs = undefined;
		const interruptionText = deferred();
		const stopWatching = adapter.subscribe((event) => {
			if (event.type === 'text_delta') interruptionText.resolve();
		});
		startedAt = performance.now();
		const activeTurn = adapter.prompt(
			'Read sample.py, then write a long, detailed explanation with at least thirty numbered observations.',
		);
		await within(interruptionText.promise, TURN_TIMEOUT_MS, 'first interruptible text');
		const abortStarted = performance.now();
		if (!await adapter.interrupt()) throw new Error('adapter did not interrupt an active turn');
		const abortMs = Math.round(performance.now() - abortStarted);
		await within(activeTurn, TURN_TIMEOUT_MS, 'interrupted turn settlement');
		stopWatching();
		if (!events.some((event) => event.type === 'turn_settled' && event.outcome === 'interrupted')) {
			throw new Error('interrupted settlement was not emitted');
		}
		if (abortMs > ABORT_TARGET_MS) throw new Error(`abort took ${abortMs} ms; target is ${ABORT_TARGET_MS} ms`);

		process.stdout.write(
			`PASS Pi SDK ${version}: init ${initMs} ms, first activity ${firstActivityMs} ms, first text ${groundedFirstTextMs} ms, abort ${abortMs} ms\n`,
		);
	} finally {
		unsubscribe();
		adapter.dispose();
		rmSync(repository, { recursive: true, force: true });
	}
}

main().catch((error) => {
	process.stderr.write(`${error instanceof Error ? (error.stack ?? error.message) : String(error)}\n`);
	process.exitCode = 1;
});
