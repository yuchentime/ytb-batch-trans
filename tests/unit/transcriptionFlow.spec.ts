import { describe, expect, it, vi } from 'vitest';
import { installTauriMock } from '../utils/tauriMock';
import { mediaHandlers } from '../utils/mocks/mediaHandlers';
import { transcriptionHandlers } from '../utils/mocks/transcriptionHandlers';
import { useMediaStore } from '../../src/stores/media/media';
import { useMediaGroupStore } from '../../src/stores/media/group';
import { useMediaStateStore, MediaState } from '../../src/stores/media/state';
import { EntryItem } from '../../src/tauri/types/media';

vi.mock('@tauri-apps/api/event', () => ({ emit: vi.fn() }));

type ItemOverrides = {
  isLeader?: boolean;
  entries?: EntryItem[];
  playlistCount?: number;
};

const item = (id: string, url: string, overrides: ItemOverrides = {}) => ({
  id,
  url,
  title: 'T',
  audioCodecs: [],
  formats: [],
  filesize: 0,
  ...overrides,
});

describe('transcribe flow store', () => {
  it('creates one transcribe group per url and starts it in fetching', async () => {
    installTauriMock({ ...mediaHandlers, ...transcriptionHandlers });
    const mediaStore = useMediaStore();
    const groupStore = useMediaGroupStore();
    const stateStore = useMediaStateStore();

    const ids = await mediaStore.startTranscriptionBatch([' https://example.com/a ', '']);

    expect(ids).toHaveLength(1);
    const group = groupStore.findGroupById(ids[0]);
    expect(group?.transcribeMode).toBe(true);
    expect(group?.url).toBe('https://example.com/a');
    expect(Object.values(group?.items ?? {})[0]?.url).toBe('https://example.com/a');
    expect(stateStore.getGroupState(ids[0])).toBe(MediaState.fetching);
  });

  it('splits a playlist group into one group per video once all items arrived', async () => {
    installTauriMock({ ...mediaHandlers, ...transcriptionHandlers });
    const mediaStore = useMediaStore();
    const groupStore = useMediaGroupStore();

    const [groupId] = await mediaStore.startTranscriptionBatch(['https://example.com/playlist']);

    mediaStore.processMediaAddPayload({
      groupId,
      total: 2,
      item: item(`${groupId}-playlist`, 'https://example.com/playlist', {
        entries: [
          { index: 0, videoUrl: 'https://example.com/v1' },
          { index: 1, videoUrl: 'https://example.com/v2' },
        ],
        playlistCount: 2,
      }),
    });
    mediaStore.processMediaAddPayload({
      groupId,
      total: 2,
      item: item('video-1', 'https://example.com/v1'),
    });

    expect(groupStore.findGroupById(groupId)).toBeDefined();

    mediaStore.processMediaAddPayload({
      groupId,
      total: 2,
      item: item('video-2', 'https://example.com/v2'),
    });

    expect(groupStore.findGroupById(groupId)).toBeUndefined();
    const splitGroups = groupStore.orderedGroups.filter(group => group.transcribeMode);
    expect(splitGroups).toHaveLength(2);
    expect(splitGroups.map(group => Object.keys(group.items)[0]).sort()).toEqual(['video-1', 'video-2']);
  });
});
