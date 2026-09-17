<template>
  <div class="flex flex-col">
    <SettingsSystem v-model="settings" />
    <div class="divider my-0" />
    <SettingsAppearance v-model="settings" />
    <div class="divider my-0" />
    <SettingsNotifications v-model="settings" />
    <div class="divider my-0" />
    <SettingsInput v-model="settings" />
    <div class="divider my-0" />
    <SettingsUpdate v-model="settings" />
  </div>
</template>

<script setup lang="ts">
import { Settings } from '../../../tauri/types/config.ts';
import SettingsSystem from '../../../components/settings/SettingsSystem.vue';
import SettingsAppearance from '../../../components/settings/SettingsAppearance.vue';
import SettingsNotifications from '../../../components/settings/SettingsNotifications.vue';
import SettingsInput from '../../../components/settings/SettingsInput.vue';
import SettingsUpdate from '../../../components/settings/SettingsUpdate.vue';

const settings = defineModel<Settings>({ required: true });
</script>
