<template>
  <SettingsTranslation v-model="settings" />
</template>

<script setup lang="ts">
import { Settings } from '../../../tauri/types/config.ts';
import SettingsTranslation from '../../../components/settings/SettingsTranslation.vue';

const settings = defineModel<Settings>({ required: true });
</script>
