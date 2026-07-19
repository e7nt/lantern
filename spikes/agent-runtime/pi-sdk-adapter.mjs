import { execFileSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

export const EXPECTED_PI_VERSION = '0.80.6';

export function mapPiEvent(event, turn = {}) {
	if (event.type === 'message_update' && event.assistantMessageEvent?.type === 'text_delta') {
		return { type: 'text_delta', text: event.assistantMessageEvent.delta };
	}
	if (event.type === 'tool_execution_start') {
		return { type: 'tool_started', id: event.toolCallId, name: event.toolName };
	}
	if (event.type === 'tool_execution_end') {
		return {
			type: 'tool_finished',
			id: event.toolCallId,
			name: event.toolName,
			failed: event.isError === true,
		};
	}
	if (event.type === 'agent_settled') {
		const outcome = turn.abortRequested
			? 'interrupted'
			: turn.failed
				? 'failed'
				: 'completed';
		return { type: 'turn_settled', outcome };
	}
	return undefined;
}

export class PiSdkAdapter {
	#session;
	#listeners = new Set();
	#unsubscribe;
	#active = false;
	#abortRequested = false;
	#failed = false;

	constructor(session) {
		this.#session = session;
		this.#unsubscribe = session.subscribe((event) => {
			if (event.type === 'agent_end') {
				const assistant = event.messages?.findLast?.((message) => message.role === 'assistant');
				this.#failed ||= assistant?.stopReason === 'error';
			}
			const mapped = mapPiEvent(event, {
				abortRequested: this.#abortRequested,
				failed: this.#failed,
			});
			if (!mapped) return;
			for (const listener of this.#listeners) listener(mapped);
		});
	}

	subscribe(listener) {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	async prompt(text) {
		if (this.#active) throw new Error('an agent turn is already active');
		this.#active = true;
		this.#abortRequested = false;
		this.#failed = false;
		try {
			await this.#session.prompt(text);
		} finally {
			this.#active = false;
		}
	}

	async interrupt() {
		if (!this.#active) return false;
		this.#abortRequested = true;
		await this.#session.abort();
		return true;
	}

	dispose() {
		this.#unsubscribe?.();
		this.#listeners.clear();
		this.#session.dispose();
	}
}

function piPackageRoot() {
	const binary = execFileSync('which', ['pi'], { encoding: 'utf8' }).trim();
	return dirname(dirname(realpathSync(binary)));
}

export async function createPiSdkAdapter({ cwd, tools = ['read'], systemPrompt }) {
	const version = execFileSync('pi', ['--version'], { encoding: 'utf8' }).trim();
	if (version !== EXPECTED_PI_VERSION) {
		throw new Error(`Pi ${EXPECTED_PI_VERSION} is required; found ${version}`);
	}
	const sdk = await import(pathToFileURL(join(piPackageRoot(), 'dist/index.js')).href);
	const resourceLoader = new sdk.DefaultResourceLoader({
		cwd,
		agentDir: sdk.getAgentDir(),
		agentsFilesOverride: () => ({ agentsFiles: [] }),
		skillsOverride: () => ({ skills: [], diagnostics: [] }),
		promptsOverride: () => ({ prompts: [], diagnostics: [] }),
		systemPromptOverride: () => systemPrompt,
	});
	await resourceLoader.reload();
	const { session } = await sdk.createAgentSession({
		cwd,
		tools,
		thinkingLevel: 'off',
		resourceLoader,
		sessionManager: sdk.SessionManager.inMemory(cwd),
		settingsManager: sdk.SettingsManager.inMemory({
			compaction: { enabled: false },
			retry: { enabled: false },
		}),
	});
	return { adapter: new PiSdkAdapter(session), version };
}
