import { createRouter, createWebHistory } from 'vue-router';
import type { RouteLocationNormalized } from 'vue-router';
import HomeView from './views/app/HomeView.vue';
import SetupView from './views/app/SetupView.vue';
import MediaView from './views/app/MediaView.vue';
import SubtitleView from './views/app/SubtitleView.vue';
import FullLayout from './layouts/FullLayout.vue';
import AppLayout from './layouts/AppLayout.vue';
import InstallView from './views/full/InstallView.vue';
import TheMediaLogs from './components/media-view/TheMediaLogs.vue';
import TheTranscript from './components/media-view/TheTranscript.vue';
import LocationView from './views/app/LocationView.vue';
import SettingsView from './views/app/SettingsView.vue';
import AuthenticationView from './views/app/AuthenticationView.vue';
import MediaPreferencesView from './views/app/MediaPreferencesView.vue';
import InputFiltersView from './views/app/InputFiltersView.vue';
import TheDownloadPreferences from './components/media-view/TheDownloadPreferences.vue';
import TheNetworkPreferences from './components/media-view/TheNetworkPreferences.vue';
import TheOutputPreferences from './components/media-view/TheOutputPreferences.vue';
import TheSubtitlePreferences from './components/media-view/TheSubtitlePreferences.vue';
import SettingsAboutTab from './views/app/settings/SettingsAboutTab.vue';
import SettingsTranscriptionTab from './views/app/settings/SettingsTranscriptionTab.vue';
import SettingsTranslationTab from './views/app/settings/SettingsTranslationTab.vue';
import SettingsOutputTab from './views/app/settings/SettingsOutputTab.vue';
import SettingsNetworkTab from './views/app/settings/SettingsNetworkTab.vue';
import SettingsSystemTab from './views/app/settings/SettingsSystemTab.vue';

const routes = [
  {
    path: '/install',
    name: 'install',
    component: FullLayout,
    children: [
      {
        path: '',
        name: 'install.index',
        component: InstallView,
        meta: { index: 0 },
      },
    ],
  },
  {
    path: '/',
    name: 'app',
    component: AppLayout,
    children: [
      {
        path: '',
        name: 'home',
        component: HomeView,
        meta: { index: 0 },
      },
      {
        path: 'setup',
        name: 'setup',
        component: SetupView,
        meta: { index: 1 },
      },
      {
        path: 'settings',
        name: 'settings',
        component: SettingsView,
        meta: { index: 1 },
        children: [
          {
            path: '',
            name: 'settings.transcription',
            component: SettingsTranscriptionTab,
            meta: { index: 0 },
          },
          {
            path: 'translation',
            name: 'settings.translation',
            component: SettingsTranslationTab,
            meta: { index: 1 },
          },
          {
            path: 'output',
            name: 'settings.output',
            component: SettingsOutputTab,
            meta: { index: 1 },
          },
          {
            path: 'network',
            name: 'settings.network',
            component: SettingsNetworkTab,
            meta: { index: 1 },
          },
          {
            path: 'system',
            name: 'settings.system',
            component: SettingsSystemTab,
            meta: { index: 1 },
          },
          {
            path: 'about',
            name: 'settings.about',
            component: SettingsAboutTab,
            meta: { index: 1 },
          },
        ],
      },
      {
        path: 'location',
        name: 'location',
        component: LocationView,
        meta: { index: 1 },
      },
      {
        path: 'authentication',
        name: 'authentication',
        component: AuthenticationView,
        meta: { index: 1 },
      },
      {
        path: 'subtitles',
        name: 'subtitles',
        component: SubtitleView,
        meta: { index: 1 },
      },
      {
        path: 'input-filters',
        name: 'input-filters',
        component: InputFiltersView,
        meta: { index: 1 },
      },
      {
        path: 'group/:groupId',
        name: 'group',
        component: MediaView,
        props: true,
        meta: { index: 1, requiresGroup: true },
        children: [
          {
            path: '',
            name: 'group.en',
            component: TheTranscript,
            props: (route: RouteLocationNormalized) => ({ groupId: route.params.groupId, kind: 'en' }),
            meta: { index: 0 },
          },
          {
            path: 'zh',
            name: 'group.zh',
            component: TheTranscript,
            props: (route: RouteLocationNormalized) => ({ groupId: route.params.groupId, kind: 'zh' }),
            meta: { index: 1 },
          },
          {
            path: 'logs',
            name: 'group.logs',
            component: TheMediaLogs,
            props: true,
            meta: { index: 2 },
          },
        ],
      },
      {
        path: 'preferences/:groupId',
        name: 'preferences',
        component: MediaPreferencesView,
        props: true,
        meta: { index: 1, requiresGroup: true },
        children: [
          {
            path: '',
            name: 'preferences.quality',
            component: TheDownloadPreferences,
            props: true,
            meta: { index: 0 },
          },
          {
            path: 'network',
            name: 'preferences.network',
            component: TheNetworkPreferences,
            props: true,
            meta: { index: 1 },
          },
          {
            path: 'output',
            name: 'preferences.output',
            component: TheOutputPreferences,
            props: true,
            meta: { index: 1 },
          },
          {
            path: 'subtitles',
            name: 'preferences.subtitles',
            component: TheSubtitlePreferences,
            props: true,
            meta: { index: 1 },
          },
        ],
      },
    ],
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;
