<template>
  <SettingsOutput v-model="settings" />
</template>

<script setup lang="ts">
import { Settings } from '../../../tauri/types/config.ts';
import SettingsOutput from '../../../components/settings/SettingsOutput.vue';

const settings = defineModel<Settings>({ required: true });
</script>
