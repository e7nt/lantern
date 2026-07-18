#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';

const EXPECTED_PI_VERSION = '0.80.6';
const MUTATING_TOOLS = new Set(['edit', 'write', 'bash']);

function piPackageRoot() {
	const binary = execFileSync('which', ['pi'], { encoding: 'utf8' }).trim();
	const cli = realpathSync(binary);
	return dirname(dirname(cli));
}

async function main() {
	const version = execFileSync('pi', ['--version'], { encoding: 'utf8' }).trim();
	if (version !== EXPECTED_PI_VERSION) {
		throw new Error(`Pi ${EXPECTED_PI_VERSION} is required; found ${version}`);
	}
	const sdk = await import(pathToFileURL(join(piPackageRoot(), 'dist/index.js')).href);
	const repository = mkdtempSync(join(tmpdir(), 'lantern-pi-sdk-control-'));
	const source = join(repository, 'sample.txt');
	writeFileSync(source, 'old\n', 'utf8');
	execFileSync('git', ['init', '-q'], { cwd: repository });
	execFileSync('git', ['add', 'sample.txt'], { cwd: repository });
	execFileSync('git', ['-c', 'user.name=Lantern Spike', '-c', 'user.email=spike@example.com', 'commit', '-qm', 'fixture'], { cwd: repository });

	const gate = { approved: false, blocked: [] };
	const resourceLoader = new sdk.DefaultResourceLoader({
		cwd: repository,
		agentDir: sdk.getAgentDir(),
		extensionFactories: [
		(pi) => {
			pi.on('tool_call', (event) => {
				if (!gate.approved && MUTATING_TOOLS.has(event.toolName)) {
					gate.blocked.push(event.toolName);
					return { block: true, reason: 'Lantern plan confirmation is required before mutation.' };
				}
				return undefined;
			});
		},
		],
		agentsFilesOverride: () => ({ agentsFiles: [] }),
		skillsOverride: () => ({ skills: [], diagnostics: [] }),
		promptsOverride: () => ({ prompts: [], diagnostics: [] }),
		systemPromptOverride: () => 'You are testing a tool gate. Follow the developer request exactly and be concise.',
	});
	await resourceLoader.reload();
	const sessionStarted = performance.now();
	const { session } = await sdk.createAgentSession({
		cwd: repository,
		tools: ['read', 'edit'],
		thinkingLevel: 'off',
		resourceLoader,
		sessionManager: sdk.SessionManager.inMemory(repository),
		settingsManager: sdk.SettingsManager.inMemory({
			compaction: { enabled: false },
			retry: { enabled: false },
		}),
	});
	const sessionInitMs = Math.round(performance.now() - sessionStarted);

	try {
		await session.prompt('Use the edit tool to replace old with new in sample.txt. Do not merely describe the change.');
		if (gate.blocked.length === 0) {
			throw new Error('the model did not attempt a mutating tool call');
		}
		if (readFileSync(source, 'utf8') !== 'old\n') {
			throw new Error('a blocked tool changed repository source');
		}
		gate.approved = true;
		await session.prompt('The plan is now approved. Use the edit tool to make the requested change.');
		if (readFileSync(source, 'utf8') !== 'new\n') {
			throw new Error('the approved edit did not change the fixture as requested');
		}
		const status = execFileSync('git', ['status', '--porcelain'], {
			cwd: repository,
			encoding: 'utf8',
		}).trim();
		if (status !== 'M sample.txt') {
			throw new Error(`unexpected review state: ${JSON.stringify(status)}`);
		}
		process.stdout.write(
			`PASS Pi SDK ${version}: initialized in ${sessionInitMs} ms, blocked ${gate.blocked.join(', ')}, then applied one approved edit\n`,
		);
	} finally {
		session.dispose();
		rmSync(repository, { recursive: true, force: true });
	}
}

main().catch((error) => {
	process.stderr.write(`${error instanceof Error ? (error.stack ?? error.message) : String(error)}\n`);
	process.exitCode = 1;
});
