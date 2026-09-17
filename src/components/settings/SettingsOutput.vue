<template>
  <base-fieldset :legend="t('settings.output.legend')" :label="t('settings.output.legendLabel')">
    <div class="grid grid-cols-1 gap-y-4">
      <label class="form-control">
        <span class="label-text">{{ t('settings.output.rootDir.label') }}</span>
        <input
            :value="settings.output.rootDir ?? ''"
            type="text"
            class="input input-bordered"
            @input="settings.output.rootDir = ($event.target as HTMLInputElement).value || null"
        />
        <span class="label-text-alt">{{ t('settings.output.rootDir.hint') }}</span>
      </label>

      <label class="label cursor-pointer justify-start gap-3">
        <input v-model="settings.output.overwrite" type="checkbox" class="checkbox" />
        <span class="label-text">{{ t('settings.output.overwrite.label') }}</span>
      </label>

      <label class="label cursor-pointer justify-start gap-3">
        <input v-model="settings.output.restrictFilenames" type="checkbox" class="checkbox" />
        <span class="label-text">{{ t('settings.output.restrictFilenames.label') }}</span>
      </label>
    </div>
  </base-fieldset>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import BaseFieldset from '../base/BaseFieldset.vue';
import { Settings } from '../../tauri/types/config.ts';

const { t } = useI18n();
const settings = defineModel<Settings>({ required: true });
</script>
