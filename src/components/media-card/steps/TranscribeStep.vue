<template>
  <div class="card-body py-0 pr-0 grow w-full overflow-hidden">
    <h2
        :title="group.title ?? group.url"
        class="card-title block leading-8 overflow-hidden whitespace-nowrap overflow-ellipsis text-base"
    >
      {{ group.title ?? group.url }}
    </h2>

    <base-progress
        :id="`${group.id}-progress`"
        :max="100"
        :value="percent"
    >
      {{ t('media.steps.transcribe.progress', { index: chunkIndex + 1, total: chunkTotal }) }}
    </base-progress>

    <div class="w-full flex gap-4">
      <p>{{ t('media.steps.transcribe.model', { model: modelDisplay }) }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, PropType } from 'vue';
import BaseProgress from '../../base/BaseProgress.vue';
import { Group } from '../../../tauri/types/group';
import { useI18n } from 'vue-i18n';
import { useTranscriptionStore } from '../../../stores/transcription';
import { useSettingsStore } from '../../../stores/settings';

const { t } = useI18n();
const transcriptionStore = useTranscriptionStore();
const settingsStore = useSettingsStore();

const { group } = defineProps({
  group: {
    type: Object as PropType<Group>,
    required: true,
  },
});

// One card tracks one video; playlist groups keep their videos as items and are split
// into per-video groups before transcription starts.
const itemId = computed(() => Object.keys(group.items)[0] ?? group.id);
const progress = computed(() => transcriptionStore.chunkFor(itemId.value));

const chunkIndex = computed(() => progress.value?.chunkIndex ?? 0);
const chunkTotal = computed(() => progress.value?.chunkTotal ?? 0);
const percent = computed(() => {
  const chunk = progress.value;
  if (!chunk || chunk.chunkTotal <= 0) return 0;
  const done = (chunk.chunkIndex + chunk.percent / 100) / chunk.chunkTotal;
  return Math.min(Math.max(done * 100, 0), 100);
});
const modelDisplay = computed(() => settingsStore.settings.transcription.model);
</script>
