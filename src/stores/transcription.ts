import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { MediaState, useMediaStateStore } from './media/state';
import { useMediaGroupStore } from './media/group';
import {
  ArtifactKind,
  ArtifactWrittenPayload,
  BatchSummaryPayload,
  TranscribeProgressPayload,
  TranscribeStage,
  TranscribeStagePayload,
  TranslateProgressPayload,
  TranscriptionProbe,
} from '../tauri/types/transcription';

export type ChunkProgress = {
  chunkIndex: number;
  chunkTotal: number;
  percent: number;
};

export type BlockProgress = {
  blockIndex: number;
  blockTotal: number;
  promptTokens: number;
  completionTokens: number;
};

export type TokenUsage = {
  promptTokens: number;
  completionTokens: number;
};

export type ArtifactPaths = {
  en?: string;
  zh?: string;
};

const stageStates: Record<TranscribeStage, MediaState> = {
  [TranscribeStage.DownloadingAudio]: MediaState.downloadingAudio,
  [TranscribeStage.Transcribing]: MediaState.transcribing,
  [TranscribeStage.Translating]: MediaState.translating,
  [TranscribeStage.Writing]: MediaState.writing,
};

/**
 * Session-only state for the transcribe pipeline: environment probe, per-video stage and
 * progress, written artifacts, token usage and the finished batch summaries.
 * Not persisted; the backend owns every durable fact.
 */
export const useTranscriptionStore = defineStore('transcription', () => {
  const stateStore = useMediaStateStore();
  const groupStore = useMediaGroupStore();

  const probe = ref<TranscriptionProbe | null>(null);
  const stages = ref<Record<string, TranscribeStage>>({});
  const chunkProgress = ref<Record<string, ChunkProgress>>({});
  const blockProgress = ref<Record<string, BlockProgress>>({});
  const artifacts = ref<Record<string, ArtifactPaths>>({});
  const usage = ref<Record<string, TokenUsage>>({});
  const summaries = ref<BatchSummaryPayload[]>([]);

  const latestSummary = computed(() => summaries.value[summaries.value.length - 1] ?? null);

  const isProbeLoaded = computed(() => probe.value !== null);

  // Required before a batch may start: whisper (AC-01 gate) plus the two ffmpeg tools
  // the chunking stage needs. CUDA and the DeepSeek key are shown but not blocking.
  const isEnvironmentReady = computed(() => {
    const result = probe.value;
    return result !== null && result.whisperFound && !!result.ffmpegPath && !!result.ffprobePath;
  });

  async function runProbe() {
    try {
      probe.value = await invoke<TranscriptionProbe>('transcription_probe');
    } catch (error) {
      // Keep the previous result; the gate stays on the last known environment.
      console.error('transcription_probe failed', error);
    }
  }

  const totalUsage = computed<TokenUsage>(() => {
    return Object.values(usage.value).reduce<TokenUsage>(
      (total, item) => ({
        promptTokens: total.promptTokens + item.promptTokens,
        completionTokens: total.completionTokens + item.completionTokens,
      }),
      { promptTokens: 0, completionTokens: 0 },
    );
  });

  function setProbe(result: TranscriptionProbe | null) {
    probe.value = result;
  }

  function processStage(payload: TranscribeStagePayload) {
    stages.value[payload.id] = payload.stage;
    stateStore.setState(payload.id, stageStates[payload.stage]);
  }

  function processChunkProgress(payload: TranscribeProgressPayload) {
    chunkProgress.value[payload.id] = {
      chunkIndex: payload.chunkIndex,
      chunkTotal: payload.chunkTotal,
      percent: Math.min(Math.max(payload.percent, 0), 100),
    };
  }

  function processTranslateProgress(payload: TranslateProgressPayload) {
    blockProgress.value[payload.id] = {
      blockIndex: payload.blockIndex,
      blockTotal: payload.blockTotal,
      promptTokens: payload.promptTokens,
      completionTokens: payload.completionTokens,
    };
    const current = usage.value[payload.id] ?? { promptTokens: 0, completionTokens: 0 };
    usage.value[payload.id] = {
      promptTokens: current.promptTokens + payload.promptTokens,
      completionTokens: current.completionTokens + payload.completionTokens,
    };
  }

  function processArtifact(payload: ArtifactWrittenPayload) {
    const current = artifacts.value[payload.id] ?? {};
    artifacts.value[payload.id] = payload.kind === ArtifactKind.En
      ? { ...current, en: payload.path }
      : { ...current, zh: payload.path };
  }

  function processBatchSummary(payload: BatchSummaryPayload) {
    summaries.value.push(payload);
  }

  function stageFor(id: string): TranscribeStage | undefined {
    return stages.value[id];
  }

  function chunkFor(id: string): ChunkProgress | undefined {
    return chunkProgress.value[id];
  }

  function blockFor(id: string): BlockProgress | undefined {
    return blockProgress.value[id];
  }

  function artifactsFor(id: string): ArtifactPaths {
    return artifacts.value[id] ?? {};
  }

  function usageFor(id: string): TokenUsage {
    return usage.value[id] ?? { promptTokens: 0, completionTokens: 0 };
  }

  function usageForGroup(groupId: string): TokenUsage {
    const group = groupStore.findGroupById(groupId);
    if (!group) return { promptTokens: 0, completionTokens: 0 };
    return Object.keys(group.items).reduce<TokenUsage>(
      (total, itemId) => {
        const item = usageFor(itemId);
        return {
          promptTokens: total.promptTokens + item.promptTokens,
          completionTokens: total.completionTokens + item.completionTokens,
        };
      },
      { promptTokens: 0, completionTokens: 0 },
    );
  }

  function forgetItem(id: string) {
    delete stages.value[id];
    delete chunkProgress.value[id];
    delete blockProgress.value[id];
    delete artifacts.value[id];
    delete usage.value[id];
  }

  function reset() {
    probe.value = null;
    stages.value = {};
    chunkProgress.value = {};
    blockProgress.value = {};
    artifacts.value = {};
    usage.value = {};
    summaries.value = [];
  }

  return {
    probe,
    stages,
    chunkProgress,
    blockProgress,
    artifacts,
    usage,
    summaries,
    latestSummary,
    totalUsage,
    isProbeLoaded,
    isEnvironmentReady,
    setProbe,
    runProbe,
    processStage,
    processChunkProgress,
    processTranslateProgress,
    processArtifact,
    processBatchSummary,
    stageFor,
    chunkFor,
    blockFor,
    artifactsFor,
    usageFor,
    usageForGroup,
    forgetItem,
    reset,
  };
});
