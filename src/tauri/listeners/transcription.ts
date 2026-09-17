import { listen } from '@tauri-apps/api/event';
import { useTranscriptionStore } from '../../stores/transcription';
import {
  ArtifactWrittenPayload,
  BatchSummaryPayload,
  TranscribeProgressPayload,
  TranscribeStagePayload,
  TranslateProgressPayload,
} from '../types/transcription';

export function registerTranscriptionListeners() {
  const store = useTranscriptionStore();

  void listen<TranscribeStagePayload>('transcribe_stage', (event) => {
    store.processStage(event.payload);
  });

  void listen<TranscribeProgressPayload>('transcribe_progress', (event) => {
    store.processChunkProgress(event.payload);
  });

  void listen<TranslateProgressPayload>('translate_progress', (event) => {
    store.processTranslateProgress(event.payload);
  });

  void listen<ArtifactWrittenPayload>('artifact_written', (event) => {
    store.processArtifact(event.payload);
  });

  void listen<BatchSummaryPayload>('batch_summary', (event) => {
    store.processBatchSummary(event.payload);
  });
}
