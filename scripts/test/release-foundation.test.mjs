import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const exec = promisify(execFile);
const root = path.resolve(import.meta.dirname, '../..');

test('release publication is tag-only, least privilege, and commit pinned', async () => {
	const workflow = await readFile(path.join(root, '.github/workflows/release.yml'), 'utf8');
	assert.match(workflow, /tags: \["v\*"\]/);
	assert.match(workflow, /workflow_dispatch:/);
	assert.match(workflow, /permissions:\n  contents: read/);
	assert.doesNotMatch(workflow, /pull_request_target|workflow_run/);
	for (const line of workflow.split('\n').filter((candidate) => candidate.includes('uses:'))) {
		assert.match(line, /uses: [^@]+@[0-9a-f]{40} # v\d/);
	}
	assert.match(workflow, /visibility --jq \.visibility\) == PUBLIC/);
	assert.match(workflow, /HOMEBREW_TAP_TOKEN/);
	assert.match(workflow, /publish:[\s\S]+if: github\.event_name == 'push'/);
});

test('Homebrew acceptance installs and launches both supported architectures', async () => {
	const workflow = await readFile(
		path.join(root, '.github/workflows/homebrew-install.yml'),
		'utf8',
	);
	assert.match(workflow, /runner: macos-14\n\s+arch: arm64/);
	assert.match(workflow, /runner: macos-15-intel\n\s+arch: x86_64/);
	assert.match(workflow, /brew install e7nt\/tap\/lantern/);
	assert.match(workflow, /brew info --json=v2 e7nt\/tap\/lantern/);
	assert.match(workflow, /lantern \$installed_version/);
	assert.match(workflow, /\[\[ -L \$candidate \]\]/);
	assert.match(workflow, /realpath "\$candidate"/);
	assert.match(workflow, /\/Library\/Frameworks\/Python\.framework\/Versions\/3\.12/);
	assert.match(workflow, /lantern --detached/);
	assert.match(workflow, /grep -Fxq 'Explorer\|0'/);
	assert.match(workflow, /grep -Fxq 'Helix\|0'/);
	assert.match(workflow, /grep -Fxq 'Lantern\|0'/);
});

test('formula renderer emits architecture-pinned AGPL metadata', async () => {
	const sha = 'a'.repeat(64);
	const version = '1.2.3';
	const base = `https://github.com/e7nt/lantern/releases/download/v${version}`;
	const { stdout } = await exec(path.join(root, 'scripts/render-homebrew-formula.sh'), [
		version,
		`${base}/lantern-${version}-darwin-arm64.tar.gz`,
		sha,
		`${base}/lantern-${version}-darwin-x86_64.tar.gz`,
		sha,
	]);
	assert.match(stdout, /license "AGPL-3\.0-only"/);
	assert.match(stdout, /on_arm do/);
	assert.match(stdout, /on_intel do/);
	assert.match(stdout, /semantic_runtime\/"vendor\.tar"/);
	assert.match(stdout, /pi_runtime\/"node_modules\.tar"/);
	assert.match(stdout, /\(bin\/"lantern"\)\.write_env_script/);
	assert.match(stdout, /Formula\["git"\]\.opt_bin/);
	assert.match(stdout, /def post_install/);
	assert.equal((stdout.match(/sha256 "a{64}"/g) ?? []).length, 2);
	assert.match(stdout, /assert_match "lantern #\{version\}"/);
});

test('packaging and formula scripts are executable and reject invalid versions', async () => {
	for (const script of ['package-macos.sh', 'render-homebrew-formula.sh']) {
		const metadata = await stat(path.join(root, 'scripts', script));
		assert.notEqual(metadata.mode & 0o111, 0, `${script} should be executable`);
	}
	await assert.rejects(
		exec(path.join(root, 'scripts/render-homebrew-formula.sh'), ['latest']),
		(error) => error.code === 2,
	);
});

test('installed launch path has explicit version and semantic overrides', async () => {
	const launcher = await readFile(path.join(root, 'scripts/launch-lantern.sh'), 'utf8');
	const semantic = await readFile(path.join(root, 'scripts/run-semantic-worker.sh'), 'utf8');
	assert.match(launcher, /LANTERN_VERSION_FILE/);
	assert.match(launcher, /--version/);
	for (const variable of [
		'LANTERN_SEMANTIC_PYTHON',
		'LANTERN_SEMANTIC_SERVICE',
		'LANTERN_SEMANTIC_VENDOR',
		'LANTERN_SEMANTIC_MODEL_CACHE',
		'LANTERN_SEMANTIC_STORAGE',
	]) {
		assert.match(semantic, new RegExp(variable));
	}
});

test('help explains Lantern before requiring a repository or runtime', async () => {
	const { stdout } = await exec(path.join(root, 'scripts/launch-lantern.sh'), ['help']);
	assert.match(stdout, /developers who love to understand and write code/);
	assert.match(stdout, /lantern \[repository\]/);
	assert.match(stdout, /Ctrl-a/);
	assert.match(stdout, /Space-g/);
	assert.match(stdout, /Space-e/);
	assert.match(stdout, /Ctrl-d/);
	assert.match(stdout, /private Pi login/);
});

test('release package locks Pi and replaces only its known vulnerable nested copies', async () => {
	const manifest = JSON.parse(
		await readFile(path.join(root, 'packaging/pi/package.json'), 'utf8'),
	);
	assert.equal(manifest.dependencies['@earendil-works/pi-coding-agent'], '0.80.6');
	assert.equal(manifest.dependencies['brace-expansion'], '5.0.7');
	assert.equal(manifest.dependencies.protobufjs, '7.6.5');
	const packager = await readFile(path.join(root, 'scripts/package-macos.sh'), 'utf8');
	assert.match(packager, /node_modules\/brace-expansion/);
	assert.match(packager, /node_modules\/protobufjs/);
	assert.match(packager, /== 5\.0\.7/);
	assert.match(packager, /== 7\.6\.5/);
	assert.match(packager, /onnxruntime==1\.23\.2/);
	assert.match(packager, /shasum -a 256 \"\$\(basename \"\$ARCHIVE\"\)\"/);
	assert.match(packager, /packaging\/helix-runtime-manifest\.txt/);
	assert.match(packager, /packaging\/helix-grammars\.txt/);
	assert.match(packager, /lantern-explorer/);
	assert.doesNotMatch(packager, /cp -R \"\$HELIX_ROOT\/runtime\"/);
});

test('packaged Helix runtime is an explicit supported-language allowlist', async () => {
	const preparer = await readFile(
		path.join(root, 'frontend/helix/prepare.sh'),
		'utf8',
	);
	const grammars = await readFile(
		path.join(root, 'packaging/helix-grammars.txt'),
		'utf8',
	);
	const manifest = await readFile(
		path.join(root, 'packaging/helix-runtime-manifest.txt'),
		'utf8',
	);
	assert.match(grammars, /^python$/m);
	assert.match(grammars, /^javascript$/m);
	assert.match(grammars, /^typescript$/m);
	assert.match(grammars, /^tsx$/m);
	assert.match(manifest, /queries\/_javascript/);
	assert.doesNotMatch(`${grammars}\n${manifest}`, /grammars\/sources|\.so|\.dylib/);
	assert.match(preparer, /helix-grammars\.txt/);
	assert.match(preparer, /--grammar fetch/);
	assert.match(preparer, /--grammar build/);
});
