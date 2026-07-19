#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { performance } from 'node:perf_hooks';

import { createPiSdkAdapter } from '../spikes/agent-runtime/pi-sdk-adapter.mjs';

const TIMEOUT_MS = 45_000;
const MODEL_PATTERN = process.env.LANTERN_PI_MODEL ?? 'gpt-5.4';
const SYSTEM_PROMPT = 'Use the read tool before answering code questions. Be accurate and concise.';
const PROMPT = 'Read sample.py and state what transport_marker returns.';

class RpcProbe {
	#child;
	#buffer = Buffer.alloc(0);
	#waiters = new Set();
	#stderr = '';

	constructor(cwd) {
		this.#child = spawn('pi', [
			'--mode', 'rpc', '--provider', 'openai-codex', '--model', MODEL_PATTERN,
			'--no-session', '--tools', 'read', '--no-extensions', '--no-skills',
			'--no-prompt-templates', '--no-context-files', '--no-approve',
			'--system-prompt', SYSTEM_PROMPT,
		], { cwd, stdio: ['pipe', 'pipe', 'pipe'] });
		this.#child.stdout.on('data', (chunk) => this.#consume(chunk));
		this.#child.stderr.on('data', (chunk) => {
			if (this.#stderr.length < 4096) this.#stderr += chunk.toString('utf8');
		});
		this.#child.once('error', (error) => this.#fail(error));
		this.#child.once('exit', (code) => {
			if (code !== null && code !== 0) this.#fail(new Error(`Pi RPC exited with status ${code}`));
		});
	}

	#consume(chunk) {
		this.#buffer = Buffer.concat([this.#buffer, chunk]);
		for (;;) {
			const newline = this.#buffer.indexOf(0x0a);
			if (newline < 0) return;
			let record = this.#buffer.subarray(0, newline);
			this.#buffer = this.#buffer.subarray(newline + 1);
			if (record.at(-1) === 0x0d) record = record.subarray(0, -1);
			if (record.length === 0) continue;
			let event;
			try {
				event = JSON.parse(record.toString('utf8'));
			} catch {
				this.#fail(new Error('Pi RPC emitted an invalid JSON record'));
				return;
			}
			for (const waiter of [...this.#waiters]) {
				if (waiter.predicate(event)) {
					this.#waiters.delete(waiter);
					clearTimeout(waiter.timer);
					waiter.resolve(event);
				}
			}
		}
	}

	#fail(error) {
		for (const waiter of this.#waiters) {
			clearTimeout(waiter.timer);
			waiter.reject(error);
		}
		this.#waiters.clear();
	}

	waitFor(predicate, operation) {
		return new Promise((resolve, reject) => {
			const waiter = { predicate, resolve, reject, timer: undefined };
			waiter.timer = setTimeout(() => {
				this.#waiters.delete(waiter);
				reject(new Error(`${operation} exceeded ${TIMEOUT_MS} ms`));
			}, TIMEOUT_MS);
			this.#waiters.add(waiter);
		});
	}

	send(command) {
		this.#child.stdin.write(`${JSON.stringify(command)}\n`);
	}

	dispose() {
		this.#child.stdin.end();
		this.#child.kill();
	}
}

async function measureRpc(cwd) {
	const started = performance.now();
	const rpc = new RpcProbe(cwd);
	try {
		const stateResponse = rpc.waitFor(
			(event) => event.type === 'response' && event.id === 'state',
			'RPC initialization',
		);
		rpc.send({ id: 'state', type: 'get_state' });
		const state = await stateResponse;
		if (!state.success || !state.data?.model) throw new Error('Pi RPC did not resolve a model');
		const initMs = Math.round(performance.now() - started);
		const turnStarted = performance.now();
		const firstActivity = rpc.waitFor(
			(event) => event.type === 'tool_execution_start' ||
				(event.type === 'message_update' && event.assistantMessageEvent?.type === 'text_delta'),
			'RPC first activity',
		);
		const firstText = rpc.waitFor(
			(event) => event.type === 'message_update' && event.assistantMessageEvent?.type === 'text_delta',
			'RPC first text',
		);
		const settled = rpc.waitFor((event) => event.type === 'agent_settled', 'RPC settlement');
		rpc.send({ id: 'prompt', type: 'prompt', message: PROMPT });
		await firstActivity;
		const firstActivityMs = Math.round(performance.now() - turnStarted);
		await firstText;
		const firstTextMs = Math.round(performance.now() - turnStarted);
		await settled;
		return {
			model: { provider: state.data.model.provider, id: state.data.model.id },
			initMs, firstActivityMs, firstTextMs,
		};
	} finally {
		rpc.dispose();
	}
}

async function measureSdk(cwd, model) {
	const started = performance.now();
	const { adapter, model: resolvedModel } = await createPiSdkAdapter({
		cwd, tools: ['read'], systemPrompt: SYSTEM_PROMPT, model,
	});
	if (resolvedModel?.provider !== model.provider || resolvedModel?.id !== model.id) {
		adapter.dispose();
		throw new Error('SDK and RPC resolved different models');
	}
	const initMs = Math.round(performance.now() - started);
	const firstActivity = { value: undefined };
	const firstText = { value: undefined };
	const turnStarted = performance.now();
	const unsubscribe = adapter.subscribe((event) => {
		const elapsed = Math.round(performance.now() - turnStarted);
		firstActivity.value ??= elapsed;
		if (event.type === 'text_delta') firstText.value ??= elapsed;
	});
	try {
		await adapter.prompt(PROMPT);
		if (firstActivity.value === undefined || firstText.value === undefined) {
			throw new Error('Pi SDK omitted a required streaming event');
		}
		return { initMs, firstActivityMs: firstActivity.value, firstTextMs: firstText.value };
	} finally {
		unsubscribe();
		adapter.dispose();
	}
}

async function main() {
	const repository = mkdtempSync(join(tmpdir(), 'lantern-pi-transport-benchmark-'));
	writeFileSync(join(repository, 'sample.py'), 'def transport_marker():\n    return "same-fixture"\n', 'utf8');
	try {
		const rpc = await measureRpc(repository);
		const sdk = await measureSdk(repository, rpc.model);
		process.stdout.write(`Pi transport benchmark (${rpc.model.provider}/${rpc.model.id})\n`);
		process.stdout.write('transport  init_ms  first_activity_ms  first_text_ms\n');
		process.stdout.write(`rpc        ${rpc.initMs}  ${rpc.firstActivityMs}  ${rpc.firstTextMs}\n`);
		process.stdout.write(`sdk        ${sdk.initMs}  ${sdk.firstActivityMs}  ${sdk.firstTextMs}\n`);
	} finally {
		rmSync(repository, { recursive: true, force: true });
	}
}

main().catch((error) => {
	process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
	process.exitCode = 1;
});
