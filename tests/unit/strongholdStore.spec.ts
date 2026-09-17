import { describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useStrongholdStore } from '../../src/stores/stronghold';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

type InvokeResult = unknown;

function mockBackend(unlockedOnStatus: boolean, unlockedAfterInit = true) {
  const invokeMock = vi.mocked(invoke);
  invokeMock.mockReset();
  invokeMock.mockImplementation((async (cmd: string): Promise<InvokeResult> => {
    switch (cmd) {
      case 'stronghold_status':
        return { unlocked: unlockedOnStatus };
      case 'stronghold_init':
        return { unlocked: unlockedAfterInit };
      case 'stronghold_keys':
        return [];
      case 'stronghold_set':
        return undefined;
      default:
        throw new Error(`unexpected command: ${cmd}`);
    }
  }) as typeof invoke);
  return invokeMock;
}

describe('stronghold store', () => {
  it('creates the vault before writing the AI key when it is still locked', async () => {
    const invokeMock = mockBackend(false);
    const store = useStrongholdStore();

    await store.setAiApiKey('sk-test-DO-NOT-LEAK');

    expect(invokeMock.mock.calls.map(call => call[0])).toEqual([
      'stronghold_status',
      'stronghold_init',
      'stronghold_keys',
      'stronghold_set',
    ]);
    const setArgs = invokeMock.mock.calls[3][1] as { entries: Record<string, number[]> };
    expect(Object.keys(setArgs.entries)).toEqual(['ai.apiKey']);
    expect(setArgs.entries['ai.apiKey']).toHaveLength('sk-test-DO-NOT-LEAK'.length);
    expect(store.status.unlocked).toBe(true);
  });

  it('skips initialization when the vault is already unlocked', async () => {
    const invokeMock = mockBackend(true);
    const store = useStrongholdStore();

    await store.setAiApiKey('sk-test');

    expect(invokeMock.mock.calls.map(call => call[0])).toEqual([
      'stronghold_status',
      'stronghold_keys',
      'stronghold_set',
    ]);
  });

  it('deletes the stored key when the value is null', async () => {
    const invokeMock = mockBackend(false);
    const store = useStrongholdStore();

    await store.setAiApiKey(null);

    const setCall = invokeMock.mock.calls.find(call => call[0] === 'stronghold_set');
    const args = setCall?.[1] as { entries: Record<string, number[] | null> };
    expect(args.entries['ai.apiKey']).toBeNull();
  });
});
