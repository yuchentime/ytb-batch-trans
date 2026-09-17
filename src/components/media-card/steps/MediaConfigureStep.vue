<template>
  <div
    class="card-body py-0 pr-0 grow w-full min-w-0 grid grid-cols-2"
    :class="showExpandedOptions ? 'auto-rows-min' : 'grid-rows-[1fr_1fr_1fr]'"
  >
    <h2 :title="group.title ?? group.url" class="card-title block leading-8 overflow-hidden text-nowrap text-ellipsis text-base row-auto col-span-2">{{ group.title ?? group.url }}</h2>
    <media-download-options
        :formats="group.formats"
        :default-value="optionsStore.getGlobalOptions()"
        v-model="selectedOptions"
        class="flex gap-4 w-full col-start-1 col-end-3"
        approximate
    />
    <media-encoding-options
        v-if="expandedOptionsType === 'encodings'"
        v-model="selectedEncodings"
        :default-value="optionsStore.getGlobalEncodings()"
        :audio-options="audioCodecOptions"
        :video-options="videoCodecOptions"
        :track-type="selectedTrackType"
        class="flex gap-4 w-full col-start-1 col-end-3"
    />
    <media-track-options
        v-if="expandedOptionsType === 'tracks'"
        v-model="selectedTracks"
        :default-value="optionsStore.getGlobalTracks()"
        :audio-options="audioTrackOptions"
        :video-options="videoTrackOptions"
        :track-type="selectedTrackType"
        class="flex gap-4 w-full col-start-1 col-end-3"
    />
    <p class="flex items-center">
      {{ t('media.steps.configure.metadata.duration', { duration: useDuration(group).value }) }}
    </p>
    <p v-if="group.isCombined" class="flex items-center">
      {{ t('media.steps.configure.metadata.items', { amount: group.total, details: itemOutcomeDisplay }) }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, PropType, watch } from 'vue';
import { DownloadOptions, EncodingOptions, TrackOptions, TrackType } from '../../../tauri/types/media';
import { useDuration } from '../../../composables/useDuration';
import { useMediaResolutionSelection } from '../../../composables/useMediaResolutionSelection';
import { useSettingsStore } from '../../../stores/settings';
import { Group } from '../../../tauri/types/group';
import { useMediaOptionsStore } from '../../../stores/media/options';
import { useI18n } from 'vue-i18n';
import MediaDownloadOptions from '../MediaDownloadOptions.vue';
import MediaEncodingOptions from '../MediaEncodingOptions.vue';
import MediaTrackOptions from '../MediaTrackOptions.vue';
import { countSkippedDiagnostics } from '../../../helpers/skippedDiagnostics.ts';
import { useMediaDiagnosticsStore } from '../../../stores/media/diagnostics.ts';

const i18n = useI18n();
const t = i18n.t;

const { group } = defineProps({
  group: {
    type: Object as PropType<Group>,
    required: true,
  },
});

const settingsStore = useSettingsStore();
const diagnosticsStore = useMediaDiagnosticsStore();
const optionsStore = useMediaOptionsStore();
const expandedOptionsType = computed(() => settingsStore.settings.appearance.expandedOptions);
const showExpandedOptions = computed(() => expandedOptionsType.value === 'encodings' || expandedOptionsType.value === 'tracks');
const unavailableTrackSuffix = computed(() => t('media.steps.configure.tracks.unavailableForSelectedResolution'));
const availableTrackPrefix = computed(() => t('media.steps.configure.tracks.availableInResolutions'));

const skippedCount = computed(() => countSkippedDiagnostics(diagnosticsStore.findDiagnosticsByGroupId(group.id)));

const itemOutcomeDisplay = computed(() => {
  if (group.errored > 0 && skippedCount.value > 0) {
    return t('media.steps.configure.metadata.failedAndSkippedCount', {
      failed: group.errored,
      skipped: skippedCount.value,
    });
  }

  if (group.errored > 0) {
    return t('media.steps.configure.metadata.failedCount', { amount: group.errored });
  }

  if (skippedCount.value > 0) {
    return t('media.steps.configure.metadata.skippedCount', { amount: skippedCount.value });
  }

  return '';
});

const selectedOptions = computed({
  get: () => optionsStore.getOptions(group.id),
  set: (value: DownloadOptions) => optionsStore.setOptions(group.id, value),
});

const selectedEncodings = computed({
  get: () => optionsStore.getEncodings(group.id),
  set: (value: EncodingOptions | undefined) => {
    if (value) optionsStore.setEncodings(group.id, value);
    else optionsStore.removeEncodings(group.id);
  },
});
const selectedTracks = computed({
  get: () => optionsStore.getTracks(group.id),
  set: (value: TrackOptions | undefined) => {
    if (value) optionsStore.setTracks(group.id, value);
    else optionsStore.removeTracks(group.id);
  },
});
const selectedTrackType = computed(() => selectedOptions.value?.trackType ?? TrackType.both);
const {
  audioCodecOptions,
  videoCodecOptions,
  audioTrackOptions,
  videoTrackOptions,
} = useMediaResolutionSelection({
  formats: () => group.formats,
  audioCodecs: () => group.audioCodecs,
  videoCodecs: () => group.videoCodecs ?? [],
  audioTracks: () => group.audioTracks ?? [],
  videoTracks: () => group.videoTracks ?? [],
  selectedOptions,
  approximate: true,
  unavailableTrackSuffix,
  availableTrackPrefix,
});

watch(selectedOptions, () => {
  const globalOptions = optionsStore.getGlobalOptions();
  if (!selectedOptions.value && globalOptions) {
    optionsStore.applyOptionsToGroup(
      group,
      globalOptions,
    );
  }
});

watch(selectedEncodings, () => {
  const globalEncodings = optionsStore.getGlobalEncodings();
  if (!selectedEncodings.value && globalEncodings) {
    optionsStore.applyEncodingsToGroup(
      group,
      globalEncodings,
    );
  }
});

watch(selectedTracks, () => {
  const globalTracks = optionsStore.getGlobalTracks();
  if (!selectedTracks.value && globalTracks) {
    optionsStore.applyTracksToGroup(
      group,
      globalTracks,
    );
  }
});

</script>
