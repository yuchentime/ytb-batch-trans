<template>
  <article>
    <base-sub-nav>
      <template v-slot:title>
        <div role="tablist" class="tabs tabs-box flex gap-2">
          <router-link
              exactActiveClass="tab-active"
              role="tab"
              :to="{ name: 'group.en', params: { groupId } }"
              class="tab"
          >
            {{ t('media.view.tabs.en') }}
          </router-link>
          <router-link
              exactActiveClass="tab-active"
              role="tab"
              :to="{ name: 'group.zh', params: { groupId } }"
              class="tab"
          >
            {{ t('media.view.tabs.zh') }}
          </router-link>
          <router-link
              exactActiveClass="tab-active"
              role="tab"
              :to="{ name: 'group.logs', params: { groupId } }"
              class="tab"
          >
            {{ t('media.view.tabs.logs') }}
          </router-link>
        </div>
      </template>
    </base-sub-nav>
    <router-view/>
  </article>
</template>

<script setup lang="ts">

import BaseSubNav from '../../components/base/BaseSubNav.vue';
import { useI18n } from 'vue-i18n';

const { groupId } = defineProps({
  groupId: {
    type: String,
    required: true,
  },
});

const { t } = useI18n();

</script>

<style scoped>
.tabs.tabs-box {
  background-color: var(--color-base-200);
  border-radius: var(--radius-box);
}

.tab {
  color: var(--color-base-content);
  border-radius: var(--radius-field);
  transition: background-color 0.15s ease, color 0.15s ease;
}

.tab:hover:not(.tab-active) {
  background-color: var(--color-neutral);
}

.tab-active {
  background-color: var(--color-neutral);
  border: var(--border) solid var(--color-primary);
}
</style>
