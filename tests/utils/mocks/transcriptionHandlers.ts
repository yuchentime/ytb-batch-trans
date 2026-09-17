import { IPCHandler } from '../tauriMock';
import { TranscriptionProbe } from '../../../src/tauri/types/transcription';

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

export const transcriptionHandlers: Record<string, IPCHandler> = {
  transcription_probe: (): TranscriptionProbe => readyProbe,
};
