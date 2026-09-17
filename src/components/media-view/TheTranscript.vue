<template>
  <section class="py-4 px-8">
    <div v-if="path" class="flex flex-col gap-4">
      <p class="break-all font-mono text-sm">{{ path }}</p>
      <div class="flex flex-wrap gap-2">
        <button class="btn btn-primary" @click="openDirectory">
          {{ t('media.view.transcript.openDirectory') }}
        </button>
        <button class="btn btn-primary" @click="openFile">
          {{ t('media.view.transcript.openFile') }}
        </button>
      </div>
    </div>
    <div v-else class="alert alert-info alert-soft">
      {{ t('media.view.transcript.missing') }}
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, PropType } from 'vue';
import { useI18n } from 'vue-i18n';
import { useMediaGroupStore } from '../../stores/media/group';
import { useTranscriptionStore } from '../../stores/transcription';
import { useOpener } from '../../composables/useOpener';

const props = defineProps({
  groupId: {
    type: String,
    required: true,
  },
  kind: {
    type: String as PropType<'en' | 'zh'>,
    required: true,
  },
});

const { t } = useI18n();
const groupStore = useMediaGroupStore();
const transcriptionStore = useTranscriptionStore();
const { openPath } = useOpener();

const itemId = computed(() => {
  const group = groupStore.findGroupById(props.groupId);
  if (!group) return '';
  return Object.keys(group.items)[0] ?? props.groupId;
});

const path = computed(() => {
  const artifacts = transcriptionStore.artifactsFor(itemId.value);
  return props.kind === 'en' ? artifacts.en : artifacts.zh;
});

const directory = computed(() => {
  const file = path.value;
  if (!file) return undefined;
  const separator = Math.max(file.lastIndexOf('\\'), file.lastIndexOf('/'));
  return separator > 0 ? file.slice(0, separator) : undefined;
});

const openFile = async () => {
  if (path.value) await openPath(path.value);
};

const openDirectory = async () => {
  if (directory.value) await openPath(directory.value);
};
</script>
