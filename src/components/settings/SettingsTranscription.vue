<template>
  <base-fieldset :legend="t('settings.transcription.legend')" :label="t('settings.transcription.legendLabel')">
    <div class="grid grid-cols-1 xl:grid-cols-2 gap-x-8 gap-y-4">
      <label class="form-control">
        <span class="label-text">{{ t('settings.transcription.model.label') }}</span>
        <select v-model="settings.transcription.model" class="select select-bordered">
          <option v-for="model in models" :key="model" :value="model">{{ model }}</option>
        </select>
        <span class="label-text-alt">{{ t('settings.transcription.model.hint') }}</span>
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.transcription.device.label') }}</span>
        <select v-model="settings.transcription.device" class="select select-bordered">
          <option v-for="device in devices" :key="device" :value="device">{{ device }}</option>
        </select>
        <span class="label-text-alt">{{ t('settings.transcription.device.hint') }}</span>
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.transcription.language.label') }}</span>
        <select v-model="settings.transcription.language" class="select select-bordered">
          <option v-for="language in languages" :key="language" :value="language">{{ language }}</option>
        </select>
        <span class="label-text-alt">{{ t('settings.transcription.language.hint') }}</span>
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.transcription.chunkMinutes.label') }}</span>
        <input v-model.number="settings.transcription.chunkMinutes" type="number" min="0" class="input input-bordered" />
        <span class="label-text-alt">{{ t('settings.transcription.chunkMinutes.hint') }}</span>
      </label>

      <label class="form-control xl:col-span-2">
        <span class="label-text">{{ t('settings.transcription.whisperPath.label') }}</span>
        <input
            :value="settings.transcription.whisperPath ?? ''"
            type="text"
            class="input input-bordered"
            @input="settings.transcription.whisperPath = ($event.target as HTMLInputElement).value || null"
        />
        <span class="label-text-alt">{{ t('settings.transcription.whisperPath.hint') }}</span>
      </label>

      <label class="label cursor-pointer justify-start gap-3">
        <input v-model="settings.transcription.fp16" type="checkbox" class="checkbox" />
        <span class="label-text">{{ t('settings.transcription.fp16.label') }}</span>
      </label>

      <label class="label cursor-pointer justify-start gap-3">
        <input v-model="settings.transcription.keepAudio" type="checkbox" class="checkbox" />
        <span class="label-text">{{ t('settings.transcription.keepAudio.label') }}</span>
      </label>

      <label class="label cursor-pointer justify-start gap-3 xl:col-span-2">
        <input v-model="settings.transcription.conditionOnPreviousText" type="checkbox" class="checkbox" />
        <span class="label-text">{{ t('settings.transcription.conditionOnPreviousText.label') }}</span>
      </label>
      <p class="text-sm opacity-70 xl:col-span-2">{{ t('settings.transcription.conditionOnPreviousText.hint') }}</p>
    </div>
  </base-fieldset>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import BaseFieldset from '../base/BaseFieldset.vue';
import {
  Settings,
  TranscriptionDevice,
  TranscriptionLanguage,
  TranscriptionModel,
} from '../../tauri/types/config.ts';

const { t } = useI18n();
const settings = defineModel<Settings>({ required: true });

const models = computed(() => Object.values(TranscriptionModel));
const devices = computed(() => Object.values(TranscriptionDevice));
const languages = computed(() => Object.values(TranscriptionLanguage));
</script>
