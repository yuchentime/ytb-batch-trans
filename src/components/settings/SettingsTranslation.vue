<template>
  <base-fieldset :legend="t('settings.translation.legend')" :label="t('settings.translation.legendLabel')">
    <div class="grid grid-cols-1 xl:grid-cols-2 gap-x-8 gap-y-4">
      <div class="form-control xl:col-span-2">
        <div class="flex flex-wrap items-end gap-3">
          <base-secret-input
              id="deepseek-api-key"
              v-model="apiKeyDraft"
              :label="t('settings.translation.key.label')"
              password
              class="grow"
          />
        </div>
        <span v-if="hasPendingKey" class="label-text-alt mt-2 text-warning">
          {{ t('settings.translation.key.pending') }}
        </span>
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
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import BaseFieldset from '../base/BaseFieldset.vue';
import BaseSecretInput from '../base/BaseSecretInput.vue';
import { Settings } from '../../tauri/types/config.ts';
import { useStrongholdStore } from '../../stores/stronghold.ts';

const { t } = useI18n();
const settings = defineModel<Settings>({ required: true });

const strongholdStore = useStrongholdStore();

// The key is a secret, so it is not part of the persisted settings draft; it lives here
// until the page's global Save action commits it to the vault.
const apiKeyDraft = computed({
  get: () => strongholdStore.aiApiKeyDraft,
  set: (value: string | null) => {
    strongholdStore.aiApiKeyDraft = value;
    strongholdStore.aiApiKeyDirty = true;
  },
});

const hasPendingKey = computed(
  () => strongholdStore.aiApiKeyDirty && !!strongholdStore.aiApiKeyDraft?.trim(),
);
</script>
