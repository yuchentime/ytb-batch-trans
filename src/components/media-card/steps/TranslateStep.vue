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
      {{ t('media.steps.translate.progress', { index: blockIndex + 1, total: blockTotal }) }}
    </base-progress>

    <div class="w-full flex gap-4">
      <p>
        {{ t('media.steps.translate.tokens', { prompt: usage.promptTokens, completion: usage.completionTokens }) }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, PropType } from 'vue';
import BaseProgress from '../../base/BaseProgress.vue';
import { Group } from '../../../tauri/types/group';
import { useI18n } from 'vue-i18n';
import { useTranscriptionStore } from '../../../stores/transcription';

const { t } = useI18n();
const transcriptionStore = useTranscriptionStore();

const { group } = defineProps({
  group: {
    type: Object as PropType<Group>,
    required: true,
  },
});

const itemId = computed(() => Object.keys(group.items)[0] ?? group.id);
const progress = computed(() => transcriptionStore.blockFor(itemId.value));

const blockIndex = computed(() => progress.value?.blockIndex ?? 0);
const blockTotal = computed(() => progress.value?.blockTotal ?? 0);
const percent = computed(() => {
  const block = progress.value;
  if (!block || block.blockTotal <= 0) return 0;
  return Math.min(Math.max(((block.blockIndex + 1) / block.blockTotal) * 100, 0), 100);
});
const usage = computed(() => transcriptionStore.usageFor(itemId.value));
</script>
