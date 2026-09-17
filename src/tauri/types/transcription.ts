// IPC payload mirrors for the transcribe pipeline:
// `src-tauri/src/models/transcribe.rs` and `commands/transcription/transcription_probe.rs`.

export enum TranscribeStage {
  DownloadingAudio = 'downloadingAudio',
  Transcribing = 'transcribing',
  Translating = 'translating',
  Writing = 'writing',
}

export interface TranscribeStagePayload {
  id: string;
  groupId: string;
  stage: TranscribeStage;
}

export interface TranscribeProgressPayload {
  id: string;
  groupId: string;
  chunkIndex: number;
  chunkTotal: number;
  percent: number;
}

export interface TranslateProgressPayload {
  id: string;
  groupId: string;
  blockIndex: number;
  blockTotal: number;
  promptTokens: number;
  completionTokens: number;
}

export enum ArtifactKind {
  En = 'en',
  Zh = 'zh',
}

export interface ArtifactWrittenPayload {
  id: string;
  groupId: string;
  kind: ArtifactKind;
  path: string;
}

export enum VideoStatus {
  Done = 'done',
  Failed = 'failed',
  Cancelled = 'cancelled',
}

export type VideoErrorCode
  = | 'whisperMissing'
    | 'whisperOutOfMemory'
    | 'whisperFailed'
    | 'ffmpegMissing'
    | 'ffmpegChunkFailed'
    | 'durationUnknown'
    | 'noSpeechDetected'
    | 'deepseekAuthFailed'
    | 'deepseekRateLimited'
    | 'deepseekServerError'
    | 'deepseekTimeout'
    | 'translationContractViolation'
    | 'outputWriteFailed'
    | 'downloadFailed'
    | 'fetchFailed';

export interface BatchSummaryItem {
  groupId: string;
  url: string;
  status: VideoStatus;
  errorCode?: VideoErrorCode;
  outputs: string[];
  skipped: boolean;
  promptTokens: number;
  completionTokens: number;
}

export interface BatchSummaryPayload {
  path: string;
  items: BatchSummaryItem[];
}

export interface TranscriptionProbe {
  whisperPath?: string;
  whisperVersion?: string;
  whisperFound: boolean;
  cudaAvailable: boolean;
  cudaDevice?: string;
  model: string;
  modelCached: boolean;
  modelPath?: string;
  modelSizeBytes?: number;
  ffmpegPath?: string;
  ffprobePath?: string;
  apiKeyConfigured: boolean;
  logDir: string;
  logFile: string;
  logSizeBytes: number;
}
