import { InvokeArgs } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { ensureInvokeArgsObject, IPCHandler } from '../tauriMock';
import { MediaAddPayload } from '../../../src/tauri/types/media';
import {
  TranscribeStage,
  TranscribeStagePayload,
  TranscribeProgressPayload,
  TranslateProgressPayload,
  TranscriptionProbe,
} from '../../../src/tauri/types/transcription';

export const readyProbe: TranscriptionProbe = {
  whisperPath: 'C:\\Python314\\Scripts\\whisper.exe',
  whisperVersion: '20250625',
  whisperFound: true,
  cudaAvailable: true,
  cudaDevice: 'NVIDIA Test GPU',
  model: 'small',
  modelCached: true,
  modelPath: 'C:\\Users\\Test\\.cache\\whisper\\small.pt',
  modelSizeBytes: 461000000,
  ffmpegPath: 'ffmpeg',
  ffprobePath: 'ffprobe',
  apiKeyConfigured: true,
  logDir: 'C:\\logs',
  logFile: 'C:\\logs\\transcribe.log',
  logSizeBytes: 2048,
};

let batchCounter = 0;

function emitStage(groupId: string, id: string, stage: TranscribeStage) {
  void emit<TranscribeStagePayload>('transcribe_stage', { id, groupId, stage });
}

/** Drives one video through the pipeline stages so cards can be asserted in E2E. */
function emitProgress(groupId: string, id: string) {
  setTimeout(() => {
    emitStage(groupId, id, TranscribeStage.DownloadingAudio);
  }, 5);
  setTimeout(() => {
    emitStage(groupId, id, TranscribeStage.Transcribing);
    void emit<TranscribeProgressPayload>('transcribe_progress', {
      id, groupId, chunkIndex: 0, chunkTotal: 2, percent: 50,
    });
  }, 10);
  setTimeout(() => {
    emitStage(groupId, id, TranscribeStage.Translating);
    void emit<TranslateProgressPayload>('translate_progress', {
      id, groupId, blockIndex: 0, blockTotal: 1, promptTokens: 12, completionTokens: 6,
    });
  }, 15);
}

function emitSingle(groupId: string, id: string, url: string) {
  void emit<MediaAddPayload>('media_add', {
    groupId,
    total: 1,
    item: {
      id,
      url,
      title: 'Test Video',
      audioCodecs: ['aac'],
      formats: [],
      filesize: 0,
      isLeader: true,
    },
  });
  emitProgress(groupId, id);
}

function emitPlaylist(groupId: string, url: string) {
  const entries = [
    { index: 0, videoUrl: `${url}?v=1` },
    { index: 1, videoUrl: `${url}?v=2` },
  ];
  void emit<MediaAddPayload>('media_add', {
    groupId,
    total: entries.length,
    item: {
      id: `${groupId}-playlist`,
      url,
      title: 'Playlist',
      audioCodecs: [],
      formats: [],
      filesize: 0,
      entries,
      playlistCount: entries.length,
    },
  });
  for (const entry of entries) {
    const id = `${groupId}-item-${entry.index}`;
    void emit<MediaAddPayload>('media_add', {
      groupId,
      total: entries.length,
      item: {
        id,
        url: entry.videoUrl,
        title: `Item ${entry.index + 1}`,
        audioCodecs: [],
        formats: [],
        filesize: 0,
      },
    });
    emitProgress(groupId, id);
  }
}

export const transcriptionHandlers: Record<string, IPCHandler> = {
  transcription_probe: (): TranscriptionProbe => readyProbe,
  transcribe_start: (_cmd: string, args: InvokeArgs | undefined): string[] => {
    const { urls } = ensureInvokeArgsObject<{ urls: string[] }>(args, 'transcribe_start');
    const batch = ++batchCounter;
    return urls.map((url, index) => {
      const groupId = `transcribe-${batch}-${index}`;
      // Emit after the invoke settles so the placeholder groups exist first.
      setTimeout(() => {
        if (url.includes('playlist')) {
          emitPlaylist(groupId, url);
        } else {
          emitSingle(groupId, `${groupId}-item`, url);
        }
      }, 0);
      return groupId;
    });
  },
};
