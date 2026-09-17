import { describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useTranscriptionStore } from '../../src/stores/transcription';
import { useMediaStateStore, MediaState } from '../../src/stores/media/state';
import { useMediaGroupStore } from '../../src/stores/media/group';
import { ArtifactKind, TranscribeStage, VideoStatus } from '../../src/tauri/types/transcription';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('transcription store', () => {
  it('maps transcribe stages onto the card state machine', () => {
    const store = useTranscriptionStore();
    const stateStore = useMediaStateStore();

    store.processStage({ id: 'a', groupId: 'g', stage: TranscribeStage.DownloadingAudio });
    expect(store.stageFor('a')).toBe(TranscribeStage.DownloadingAudio);
    expect(stateStore.getState('a')).toBe(MediaState.downloadingAudio);

    store.processStage({ id: 'a', groupId: 'g', stage: TranscribeStage.Transcribing });
    expect(stateStore.getState('a')).toBe(MediaState.transcribing);

    store.processStage({ id: 'a', groupId: 'g', stage: TranscribeStage.Translating });
    expect(stateStore.getState('a')).toBe(MediaState.translating);

    store.processStage({ id: 'a', groupId: 'g', stage: TranscribeStage.Writing });
    expect(stateStore.getState('a')).toBe(MediaState.writing);
  });

  it('keeps chunk progress per item and clamps the percentage', () => {
    const store = useTranscriptionStore();

    store.processChunkProgress({ id: 'a', groupId: 'g', chunkIndex: 1, chunkTotal: 3, percent: 42.5 });
    expect(store.chunkFor('a')).toEqual({ chunkIndex: 1, chunkTotal: 3, percent: 42.5 });

    store.processChunkProgress({ id: 'a', groupId: 'g', chunkIndex: 1, chunkTotal: 3, percent: 150 });
    expect(store.chunkFor('a')?.percent).toBe(100);

    store.processChunkProgress({ id: 'a', groupId: 'g', chunkIndex: 2, chunkTotal: 3, percent: -5 });
    expect(store.chunkFor('a')?.percent).toBe(0);
  });

  it('accumulates token usage from translate progress', () => {
    const store = useTranscriptionStore();

    store.processTranslateProgress({
      id: 'a', groupId: 'g', blockIndex: 0, blockTotal: 2, promptTokens: 10, completionTokens: 5,
    });
    store.processTranslateProgress({
      id: 'a', groupId: 'g', blockIndex: 1, blockTotal: 2, promptTokens: 4, completionTokens: 2,
    });

    expect(store.blockFor('a')).toEqual({
      blockIndex: 1, blockTotal: 2, promptTokens: 4, completionTokens: 2,
    });
    expect(store.usageFor('a')).toEqual({ promptTokens: 14, completionTokens: 7 });
    expect(store.totalUsage).toEqual({ promptTokens: 14, completionTokens: 7 });
  });

  it('merges artifact paths by kind and keeps the latest summary', () => {
    const store = useTranscriptionStore();

    store.processArtifact({
      id: 'a', groupId: 'g', kind: ArtifactKind.En, path: 'a/transcript.en.txt',
    });
    store.processArtifact({
      id: 'a', groupId: 'g', kind: ArtifactKind.Zh, path: 'a/transcript.zh.txt',
    });
    expect(store.artifactsFor('a')).toEqual({
      en: 'a/transcript.en.txt', zh: 'a/transcript.zh.txt',
    });

    const summary = {
      path: 'summary.md',
      items: [{
        groupId: 'g',
        url: 'https://example.com/a',
        status: VideoStatus.Done,
        outputs: ['a/transcript.en.txt'],
        skipped: false,
        promptTokens: 10,
        completionTokens: 5,
      }],
    };
    store.processBatchSummary(summary);
    expect(store.latestSummary).toEqual(summary);
    expect(store.summaries).toHaveLength(1);
  });

  it('runs the environment probe and derives the gate', async () => {
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockResolvedValue({
      whisperFound: true,
      cudaAvailable: true,
      model: 'small',
      modelCached: true,
      apiKeyConfigured: true,
      ffmpegPath: 'ffmpeg',
      ffprobePath: 'ffprobe',
      logDir: '/logs',
      logFile: '/logs/transcribe.log',
      logSizeBytes: 0,
    });

    const store = useTranscriptionStore();
    expect(store.isProbeLoaded).toBe(false);
    expect(store.isEnvironmentReady).toBe(false);

    await store.runProbe();
    expect(invokeMock).toHaveBeenCalledWith('transcription_probe');
    expect(store.isProbeLoaded).toBe(true);
    expect(store.isEnvironmentReady).toBe(true);

    // whisper missing: the gate closes even though the probe itself succeeded.
    invokeMock.mockResolvedValueOnce({
      whisperFound: false,
      cudaAvailable: true,
      model: 'small',
      modelCached: false,
      apiKeyConfigured: false,
      logDir: '/logs',
      logFile: '/logs/transcribe.log',
      logSizeBytes: 0,
    });
    await store.runProbe();
    expect(store.isProbeLoaded).toBe(true);
    expect(store.isEnvironmentReady).toBe(false);
  });

  it('aggregates usage per group and forgets per-item data on reset', () => {
    const groupStore = useMediaGroupStore();
    const store = useTranscriptionStore();

    groupStore.createGroup({
      id: 'g',
      url: 'g',
      total: 2,
      processed: 0,
      errored: 0,
      isCombined: false,
      audioCodecs: [],
      formats: [],
      filesize: 0,
      items: {
        a: { id: 'a', url: 'a', title: 'A', audioCodecs: [], formats: [], filesize: 0, isLeader: true },
        b: { id: 'b', url: 'b', title: 'B', audioCodecs: [], formats: [], filesize: 0 },
      },
    });

    store.processTranslateProgress({
      id: 'a', groupId: 'g', blockIndex: 0, blockTotal: 1, promptTokens: 10, completionTokens: 5,
    });
    store.processTranslateProgress({
      id: 'b', groupId: 'g', blockIndex: 0, blockTotal: 1, promptTokens: 4, completionTokens: 2,
    });
    expect(store.usageForGroup('g')).toEqual({ promptTokens: 14, completionTokens: 7 });
    expect(store.usageForGroup('missing')).toEqual({ promptTokens: 0, completionTokens: 0 });

    store.forgetItem('a');
    expect(store.usageFor('a')).toEqual({ promptTokens: 0, completionTokens: 0 });
    expect(store.stageFor('a')).toBeUndefined();

    store.setProbe({
      whisperFound: true,
      cudaAvailable: true,
      model: 'small',
      modelCached: true,
      apiKeyConfigured: false,
      logDir: '/logs',
      logFile: '/logs/transcribe.log',
      logSizeBytes: 0,
    });
    expect(store.probe?.model).toBe('small');

    store.reset();
    expect(store.probe).toBeNull();
    expect(store.totalUsage).toEqual({ promptTokens: 0, completionTokens: 0 });
    expect(store.summaries).toHaveLength(0);
  });
});
