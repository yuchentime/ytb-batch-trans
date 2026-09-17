import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import TheTranscript from '../../src/components/media-view/TheTranscript.vue';
import { i18n } from '../../src/i18n';
import { useMediaGroupStore } from '../../src/stores/media/group';
import { useTranscriptionStore } from '../../src/stores/transcription';
import { ArtifactKind } from '../../src/tauri/types/transcription';

const openPath = vi.fn();

vi.mock('@tauri-apps/plugin-opener', () => ({
  openPath: (...args: unknown[]) => openPath(...args),
  openUrl: vi.fn(),
  revealItemInDir: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

function createGroup() {
  useMediaGroupStore().createGroup({
    id: 'g1',
    url: 'https://example.com/video',
    total: 1,
    processed: 1,
    errored: 0,
    isCombined: false,
    transcribeMode: true,
    audioCodecs: [],
    formats: [],
    filesize: 0,
    items: {
      'item-1': {
        id: 'item-1',
        url: 'https://example.com/video',
        title: 'Test Video',
        audioCodecs: [],
        formats: [],
        filesize: 0,
        isLeader: true,
      },
    },
  });
}

describe('TheTranscript', () => {
  it('opens the written transcript and its output folder', async () => {
    createGroup();
    useTranscriptionStore().processArtifact({
      id: 'item-1',
      groupId: 'g1',
      kind: ArtifactKind.En,
      path: 'C:\\out\\video\\transcript.en.txt',
    });

    const wrapper = mount(TheTranscript, {
      props: { groupId: 'g1', kind: 'en' },
      global: { plugins: [i18n] },
    });

    expect(wrapper.text()).toContain('C:\\out\\video\\transcript.en.txt');
    const buttons = wrapper.findAll('button');
    expect(buttons.map(button => button.text())).toEqual([
      'Open output folder',
      'Open with the default app',
    ]);

    await buttons[1].trigger('click');
    await flushPromises();
    expect(openPath).toHaveBeenCalledWith('C:\\out\\video\\transcript.en.txt');

    await buttons[0].trigger('click');
    await flushPromises();
    expect(openPath).toHaveBeenCalledWith('C:\\out\\video');
  });

  it('shows the empty state when the transcript is missing', () => {
    const wrapper = mount(TheTranscript, {
      props: { groupId: 'missing', kind: 'zh' },
      global: { plugins: [i18n] },
    });

    expect(wrapper.text()).toContain('Not written yet.');
    expect(wrapper.findAll('button')).toHaveLength(0);
  });
});
