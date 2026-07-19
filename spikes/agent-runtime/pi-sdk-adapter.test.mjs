import assert from 'node:assert/strict';
import test from 'node:test';

import { mapPiEvent, PiSdkAdapter } from './pi-sdk-adapter.mjs';

test('maps only the bounded Lantern event contract', () => {
	assert.deepEqual(
		mapPiEvent({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'hi' } }),
		{ type: 'text_delta', text: 'hi' },
	);
	assert.deepEqual(mapPiEvent({ type: 'tool_execution_start', toolCallId: '1', toolName: 'read', args: {} }), {
		type: 'tool_started', id: '1', name: 'read',
	});
	assert.deepEqual(mapPiEvent({ type: 'tool_execution_end', toolCallId: '1', toolName: 'read', isError: false }), {
		type: 'tool_finished', id: '1', name: 'read', failed: false,
	});
	assert.deepEqual(mapPiEvent({ type: 'agent_settled' }, { abortRequested: true }), {
		type: 'turn_settled', outcome: 'interrupted',
	});
	assert.equal(mapPiEvent({ type: 'agent_end', messages: [] }), undefined);
	assert.equal(mapPiEvent({ type: 'message_start' }), undefined);
});

test('interrupts an active turn and reports an interrupted settlement', async () => {
	let listener;
	let release;
	const session = {
		subscribe(value) { listener = value; return () => {}; },
		prompt() { return new Promise((resolve) => { release = resolve; }); },
		async abort() {
			listener({ type: 'agent_end', messages: [{ role: 'assistant', stopReason: 'aborted' }] });
			listener({ type: 'agent_settled' });
			release();
		},
		dispose() {},
	};
	const adapter = new PiSdkAdapter(session);
	const events = [];
	adapter.subscribe((event) => events.push(event));
	const turn = adapter.prompt('work');
	await assert.rejects(adapter.prompt('overlap'), /already active/);
	assert.equal(await adapter.interrupt(), true);
	await turn;
	assert.deepEqual(events, [{ type: 'turn_settled', outcome: 'interrupted' }]);
	assert.equal(await adapter.interrupt(), false);
});
