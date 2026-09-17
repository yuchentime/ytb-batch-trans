<template>
  <SettingsTranscription v-model="settings" />
</template>

<script setup lang="ts">
import { Settings } from '../../../tauri/types/config.ts';
import SettingsTranscription from '../../../components/settings/SettingsTranscription.vue';

const settings = defineModel<Settings>({ required: true });
</script>
