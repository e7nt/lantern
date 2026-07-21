import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('canonical verification rejects unknown suites visibly', () => {
	const result = spawnSync(path.join(root, 'scripts/check.sh'), ['unknown'], {
		encoding: 'utf8'
	});
	assert.equal(result.status, 2);
	assert.match(result.stderr, /Unknown verification suite: unknown/);
});

test('pinned Helix preparation agrees with the upstream inventory', async () => {
	const inventory = JSON.parse(
		await readFile(path.join(root, 'frontend/helix/upstream.json'), 'utf8')
	);
	const prepare = await readFile(path.join(root, 'frontend/helix/prepare.sh'), 'utf8');
	assert.match(prepare, new RegExp(`HELIX_REVISION=${inventory.helix.revision}`));
	assert.ok(prepare.includes(`HELIX_REPOSITORY=${inventory.helix.repository}`));
	assert.match(prepare, /fetch --depth 1 origin "\$HELIX_REVISION"/);
});

test('CI is least privilege and pins every external action by commit', async () => {
	const workflow = await readFile(path.join(root, '.github/workflows/ci.yml'), 'utf8');
	assert.match(workflow, /permissions:\n  contents: read/);
	assert.doesNotMatch(workflow, /pull_request_target|@v\d/);
	for (const line of workflow.split('\n').filter((line) => line.includes('uses:'))) {
		assert.match(line, /@[0-9a-f]{40}(?:\s+#\s+v\d[^\s]*)?$/);
	}
});

test('public contributor entry points exist and the check script is executable', async () => {
	for (const file of ['CONTRIBUTING.md', 'SECURITY.md', 'rust-toolchain.toml']) {
		assert.ok((await stat(path.join(root, file))).isFile(), `${file} should exist`);
	}
	assert.notEqual((await stat(path.join(root, 'scripts/check.sh'))).mode & 0o111, 0);
});
