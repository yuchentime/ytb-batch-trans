<template>
  <div class="card-body py-0 pr-0 grow w-full overflow-hidden">
    <h2
        :title="group.title ?? group.url"
        class="card-title block leading-8 overflow-hidden whitespace-nowrap overflow-ellipsis text-base"
    >
      {{ group.title ?? group.url }}
    </h2>

    <base-progress :id="`${group.id}-progress`">
      {{ t('media.steps.audioDownload.indeterminate') }}
    </base-progress>
  </div>
</template>

<script setup lang="ts">
import { PropType } from 'vue';
import BaseProgress from '../../base/BaseProgress.vue';
import { Group } from '../../../tauri/types/group';
import { useI18n } from 'vue-i18n';

const { t } = useI18n();

const { group } = defineProps({
  group: {
    type: Object as PropType<Group>,
    required: true,
  },
});
</script>
