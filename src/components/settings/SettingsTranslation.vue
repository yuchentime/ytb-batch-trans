<template>
  <base-fieldset :legend="t('settings.translation.legend')" :label="t('settings.translation.legendLabel')">
    <div class="grid grid-cols-1 xl:grid-cols-2 gap-x-8 gap-y-4">
      <div class="form-control xl:col-span-2">
        <span class="label-text">{{ t('settings.translation.key.label') }}</span>
        <div class="flex gap-2 items-center">
          <input
              v-model="apiKey"
              type="password"
              autocomplete="new-password"
              class="input input-bordered grow"
              :placeholder="t('settings.translation.key.placeholder')"
              @keydown.enter.prevent="saveApiKey"
          />
          <button type="button" class="btn btn-primary" :disabled="isSavingKey || apiKey.length === 0" @click="saveApiKey">
            {{ t('settings.translation.key.save') }}
          </button>
          <span class="badge" :class="apiKeyConfigured ? 'badge-success' : 'badge-warning'">
            {{ apiKeyConfigured ? t('settings.translation.key.configured') : t('settings.translation.key.missing') }}
          </span>
        </div>
        <span class="label-text-alt">{{ t('settings.translation.key.hint') }}</span>
      </div>

      <label class="form-control">
        <span class="label-text">{{ t('settings.translation.baseUrl.label') }}</span>
        <input v-model="settings.translation.baseUrl" type="text" class="input input-bordered" />
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.translation.model.label') }}</span>
        <input v-model="settings.translation.model" type="text" class="input input-bordered" />
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.translation.temperature.label') }}</span>
        <input v-model.number="settings.translation.temperature" type="number" step="0.1" min="0" max="2" class="input input-bordered" />
        <span class="label-text-alt">{{ t('settings.translation.temperature.hint') }}</span>
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.translation.concurrency.label') }}</span>
        <input v-model.number="settings.translation.concurrency" type="number" min="1" class="input input-bordered" />
      </label>

      <label class="form-control">
        <span class="label-text">{{ t('settings.translation.maxRetries.label') }}</span>
        <input v-model.number="settings.translation.maxRetries" type="number" min="0" class="input input-bordered" />
      </label>

      <label class="label cursor-pointer justify-start gap-3">
        <input v-model="settings.translation.dropFillers" type="checkbox" class="checkbox" />
        <span class="label-text">{{ t('settings.translation.dropFillers.label') }}</span>
      </label>

      <label class="form-control xl:col-span-2">
        <span class="label-text">{{ t('settings.translation.glossary.label') }}</span>
        <textarea v-model="settings.translation.glossary" rows="6" class="textarea textarea-bordered font-mono"></textarea>
        <span class="label-text-alt">{{ t('settings.translation.glossary.hint') }}</span>
      </label>
    </div>
  </base-fieldset>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import BaseFieldset from '../base/BaseFieldset.vue';
import { Settings } from '../../tauri/types/config.ts';
import { useStrongholdStore } from '../../stores/stronghold.ts';
import { useTranscriptionStore } from '../../stores/transcription.ts';
import { useToastStore } from '../../stores/toast.ts';

const { t } = useI18n();
const settings = defineModel<Settings>({ required: true });

const strongholdStore = useStrongholdStore();
const transcriptionStore = useTranscriptionStore();
const toastStore = useToastStore();

const apiKey = ref('');
const isSavingKey = ref(false);
const apiKeyConfigured = computed(() => transcriptionStore.probe?.apiKeyConfigured === true);

async function saveApiKey() {
  const value = apiKey.value.trim();
  if (value.length === 0) return;

  isSavingKey.value = true;
  try {
    await strongholdStore.setAiApiKey(value);
    apiKey.value = '';
    toastStore.showToast(t('settings.translation.key.saved'));
    // Refresh the "configured" badge; the probe only reports presence, never the value.
    await transcriptionStore.runProbe();
  } catch (error) {
    console.error(error);
    toastStore.showToast(String(error), { style: 'error' });
  } finally {
    isSavingKey.value = false;
  }
}
</script>
