<template>
  <section class="p-4 sm:p-6 max-w-3xl mx-auto w-full">
    <div class="card bg-base-300 shadow-md">
      <div class="card-body gap-4">
        <div class="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h1 class="card-title">{{ t('setup.title') }}</h1>
            <p class="text-sm opacity-70">{{ t('setup.subtitle') }}</p>
          </div>
          <button class="btn btn-primary" :disabled="isChecking" @click="recheck">
            {{ isChecking ? t('setup.checking') : t('setup.recheck') }}
          </button>
        </div>

        <div v-if="!store.probe && !isChecking" class="alert alert-warning">
          {{ t('setup.notChecked') }}
        </div>

        <div v-else-if="store.probe" class="flex flex-col">
          <div
              v-for="row in rows"
              :key="row.label"
              class="flex items-start justify-between gap-4 border-b border-base-content/10 py-2 last:border-b-0"
          >
            <div class="min-w-0">
              <p class="font-semibold">{{ row.label }}</p>
              <p class="text-sm opacity-70 break-all">{{ row.value }}</p>
            </div>
            <span
                class="badge shrink-0"
                :class="row.required ? (row.ok ? 'badge-success' : 'badge-error') : 'badge-ghost'"
            >
              {{ row.required ? (row.ok ? t('setup.ok') : t('setup.missing')) : t('setup.optional') }}
            </span>
          </div>
        </div>

        <div v-if="store.isProbeLoaded" class="alert" :class="store.isEnvironmentReady ? 'alert-success' : 'alert-error'">
          {{ store.isEnvironmentReady ? t('setup.allReady') : t('setup.gate') }}
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useTranscriptionStore } from '../../stores/transcription';
import { formatBytes } from '../../helpers/units';

const { t } = useI18n();
const store = useTranscriptionStore();
const isChecking = ref(false);

async function recheck() {
  isChecking.value = true;
  try {
    await store.runProbe();
  } finally {
    isChecking.value = false;
  }
}

onMounted(() => {
  void recheck();
});

const rows = computed(() => {
  const probe = store.probe;
  if (!probe) return [];

  const whisperLocation = [probe.whisperPath, probe.whisperVersion].filter(Boolean).join(' · ');
  const modelSize = probe.modelSizeBytes != null ? ` (${formatBytes(probe.modelSizeBytes)})` : '';

  return [
    {
      label: t('setup.fields.whisper'),
      value: probe.whisperFound ? whisperLocation : t('setup.values.notFound'),
      ok: probe.whisperFound,
      required: true,
    },
    {
      label: t('setup.fields.cuda'),
      value: probe.cudaAvailable ? (probe.cudaDevice ?? '') : t('setup.values.cudaUnavailable'),
      ok: probe.cudaAvailable,
      required: false,
    },
    {
      label: t('setup.fields.model'),
      value: probe.modelCached
        ? `${t('setup.values.modelCached', { model: probe.model })}${modelSize}`
        : t('setup.values.modelNotCached', { model: probe.model }),
      ok: true,
      required: false,
    },
    {
      label: t('setup.fields.ffmpeg'),
      value: probe.ffmpegPath ?? t('setup.values.notFound'),
      ok: !!probe.ffmpegPath,
      required: true,
    },
    {
      label: t('setup.fields.ffprobe'),
      value: probe.ffprobePath ?? t('setup.values.notFound'),
      ok: !!probe.ffprobePath,
      required: true,
    },
    {
      label: t('setup.fields.apiKey'),
      value: probe.apiKeyConfigured ? t('setup.values.apiKeySet') : t('setup.values.apiKeyMissing'),
      ok: probe.apiKeyConfigured,
      required: false,
    },
    {
      label: t('setup.fields.logs'),
      value: `${probe.logFile} (${formatBytes(probe.logSizeBytes)})`,
      ok: true,
      required: false,
    },
  ];
});
</script>
