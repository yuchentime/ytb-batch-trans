<template>
  <base-fieldset :legend="t('settings.translation.legend')" :label="t('settings.translation.legendLabel')">
    <div class="grid grid-cols-1 xl:grid-cols-2 gap-x-8 gap-y-4">
      <div class="form-control xl:col-span-2">
        <div class="flex flex-wrap items-end gap-3">
          <base-secret-input
              id="deepseek-api-key"
              v-model="apiKey"
              :label="t('settings.translation.key.label')"
              password
              :disabled="isSavingKey"
              class="grow"
          />
          <button
              type="button"
              class="btn btn-primary"
              :disabled="isSavingKey || !apiKey"
              @click="saveApiKey"
          >
            {{ t('settings.translation.key.save') }}
          </button>
          <button
              v-if="apiKeyConfigured"
              type="button"
              class="btn btn-ghost"
              :disabled="isSavingKey"
              @click="clearApiKey"
          >
            {{ t('settings.translation.key.clear') }}
          </button>
          <span class="badge badge-lg gap-1" :class="apiKeyConfigured ? 'badge-success' : 'badge-warning'">
            <check-circle-icon v-if="apiKeyConfigured" class="w-4 h-4" />
            {{ apiKeyConfigured ? t('settings.translation.key.configured') : t('settings.translation.key.missing') }}
          </span>
        </div>
        <div v-if="justSaved" class="alert alert-success alert-soft mt-3">
          <check-circle-icon class="w-5 h-5" />
          <span>{{ t('settings.translation.key.savedHint') }}</span>
        </div>
        <span class="label-text-alt mt-2">{{ t('settings.translation.key.hint') }}</span>
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
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { CheckCircleIcon } from '@heroicons/vue/24/solid';
import BaseFieldset from '../base/BaseFieldset.vue';
import BaseSecretInput from '../base/BaseSecretInput.vue';
import { Settings } from '../../tauri/types/config.ts';
import { useStrongholdStore } from '../../stores/stronghold.ts';
import { useTranscriptionStore } from '../../stores/transcription.ts';
import { useToastStore } from '../../stores/toast.ts';

const { t } = useI18n();
const settings = defineModel<Settings>({ required: true });

const strongholdStore = useStrongholdStore();
const transcriptionStore = useTranscriptionStore();
const toastStore = useToastStore();

const apiKey = ref<string | null>(null);
const isSavingKey = ref(false);
const justSaved = ref(false);
const apiKeyConfigured = computed(
  () => transcriptionStore.probe?.apiKeyConfigured === true || justSaved.value,
);

// Hide the confirmation once the user starts typing a replacement key.
watch(apiKey, (value) => {
  if (value) justSaved.value = false;
});

async function saveApiKey() {
  const value = apiKey.value?.trim();
  if (!value) return;

  isSavingKey.value = true;
  try {
    await strongholdStore.setAiApiKey(value);
    apiKey.value = null;
    justSaved.value = true;
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

async function clearApiKey() {
  if (!window.confirm(t('settings.translation.key.clearConfirm'))) return;

  isSavingKey.value = true;
  try {
    await strongholdStore.setAiApiKey(null);
    justSaved.value = false;
    toastStore.showToast(t('settings.translation.key.cleared'));
    await transcriptionStore.runProbe();
  } catch (error) {
    console.error(error);
    toastStore.showToast(String(error), { style: 'error' });
  } finally {
    isSavingKey.value = false;
  }
}
</script>
