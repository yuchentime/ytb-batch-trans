import { describe, expect, it } from 'vitest';
import {
  TranscriptionDevice,
  TranscriptionLanguage,
  TranscriptionModel,
  defaultSettings,
} from '../../src/tauri/types/config';

// Guards the mirror contract between `src-tauri/src/state/config_models.rs` defaults and the
// UI-side `defaultSettings` (docs/current/domains/settings-preferences/data-model.md).
describe('transcription/translation config defaults', () => {
  it('mirrors the Rust schema defaults', () => {
    expect(defaultSettings.transcription).toEqual({
      model: TranscriptionModel.small,
      device: TranscriptionDevice.cuda,
      fp16: true,
      language: TranscriptionLanguage.en,
      chunkMinutes: 20,
      keepAudio: false,
      whisperPath: null,
      conditionOnPreviousText: false,
    });

    expect(defaultSettings.translation).toEqual({
      baseUrl: 'https://api.deepseek.com',
      model: 'deepseek-chat',
      temperature: 0.3,
      concurrency: 2,
      maxRetries: 2,
      maxSegmentsPerBlock: 6,
      maxCharsPerBlock: 3000,
      glossary: '',
      dropFillers: true,
    });

    expect(defaultSettings.logging).toEqual({ verbose: false });
    expect(defaultSettings.output.rootDir).toBeNull();
    expect(defaultSettings.output.overwrite).toBe(false);
  });

  it('serialises enum values the way the Rust schema expects', () => {
    expect(TranscriptionModel.small).toBe('small');
    expect(TranscriptionModel.medium).toBe('medium');
    expect(TranscriptionModel.largeV3).toBe('large-v3');
    expect(TranscriptionDevice.cuda).toBe('cuda');
    expect(TranscriptionDevice.cpu).toBe('cpu');
    expect(TranscriptionLanguage.en).toBe('en');
    expect(TranscriptionLanguage.auto).toBe('auto');
  });
});
