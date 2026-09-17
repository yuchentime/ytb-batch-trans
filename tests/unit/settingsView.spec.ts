import { mount } from '@vue/test-utils';
import { createMemoryHistory, createRouter } from 'vue-router';
import { defineComponent, nextTick, reactive, ref } from 'vue';
import { describe, expect, it, vi } from 'vitest';
import SettingsView from '../../src/views/app/SettingsView.vue';
import SettingsTranscriptionTab from '../../src/views/app/settings/SettingsTranscriptionTab.vue';
import SettingsTranslationTab from '../../src/views/app/settings/SettingsTranslationTab.vue';
import SettingsOutputTab from '../../src/views/app/settings/SettingsOutputTab.vue';
import SettingsNetworkTab from '../../src/views/app/settings/SettingsNetworkTab.vue';
import SettingsSystemTab from '../../src/views/app/settings/SettingsSystemTab.vue';
import SettingsAboutTab from '../../src/views/app/settings/SettingsAboutTab.vue';
import { i18n } from '../../src/i18n';
import { defaultSettings, type Settings } from '../../src/tauri/types/config';

const patch = vi.fn();
const reset = vi.fn();
const showToast = vi.fn();
const setTheme = vi.fn();
const openInternalPath = vi.fn();
const aiApiKeyDraft = ref<string | null>(null);
const aiApiKeyDirty = ref(false);
const setAiApiKey = vi.fn();
const runProbe = vi.fn();

const settingsState = reactive<Settings>(structuredClone(defaultSettings));

vi.mock('../../src/stores/settings', () => ({
  useSettingsStore: () => ({
    settings: settingsState,
    patch,
    reset,
  }),
}));

vi.mock('../../src/stores/toast', () => ({
  useToastStore: () => ({
    showToast,
  }),
}));

vi.mock('../../src/composables/useTheme', () => ({
  useTheme: () => ({
    setTheme,
  }),
}));

vi.mock('../../src/composables/useOpener', () => ({
  useOpener: () => ({
    openInternalPath,
  }),
}));

vi.mock('../../src/stores/stronghold', () => ({
  useStrongholdStore: () => ({
    get aiApiKeyDraft() {
      return aiApiKeyDraft.value;
    },
    set aiApiKeyDraft(value: string | null) {
      aiApiKeyDraft.value = value;
    },
    get aiApiKeyDirty() {
      return aiApiKeyDirty.value;
    },
    set aiApiKeyDirty(value: boolean) {
      aiApiKeyDirty.value = value;
    },
    setAiApiKey,
  }),
}));

vi.mock('../../src/stores/transcription', () => ({
  useTranscriptionStore: () => ({
    probe: null,
    runProbe,
  }),
}));

function createRoot() {
  return defineComponent({
    template: '<router-view />',
  });
}

function createBlankView() {
  return defineComponent({
    template: '<div />',
  });
}

function createSettingsRouter() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: '/',
        name: 'home',
        component: createBlankView(),
      },
      {
        path: '/',
        component: createRoot(),
        children: [
          {
            path: 'settings',
            component: SettingsView,
            children: [
              {
                path: '',
                name: 'settings.transcription',
                component: SettingsTranscriptionTab,
              },
              {
                path: 'translation',
                name: 'settings.translation',
                component: SettingsTranslationTab,
              },
              {
                path: 'output',
                name: 'settings.output',
                component: SettingsOutputTab,
              },
              {
                path: 'network',
                name: 'settings.network',
                component: SettingsNetworkTab,
              },
              {
                path: 'system',
                name: 'settings.system',
                component: SettingsSystemTab,
              },
              {
                path: 'about',
                name: 'settings.about',
                component: SettingsAboutTab,
              },
            ],
          },
        ],
      },
    ],
  });
  return router;
}

describe('SettingsView', () => {
  it('renders the transcribe settings tabs and switches between them', async () => {
    const router = createSettingsRouter();
    await router.push('/settings');
    await router.isReady();

    const wrapper = mount(createRoot(), {
      global: {
        plugins: [router, i18n],
      },
    });

    await nextTick();

    const tabs = wrapper.findAll('a[role="tab"]');
    expect(tabs.map(tab => tab.text())).toEqual([
      'Transcription',
      'Translation',
      'Output',
      'Network',
      'System',
      'About',
    ]);
    expect(tabs[0].classes()).toContain('tab-active');

    await router.push('/settings/translation');
    await nextTick();

    const updatedTabs = wrapper.findAll('a[role="tab"]');
    expect(updatedTabs[0].classes()).not.toContain('tab-active');
    expect(updatedTabs[1].classes()).toContain('tab-active');
  });

  it('keeps save and reset operating on the same draft settings object', async () => {
    patch.mockReset();
    reset.mockReset();
    showToast.mockReset();
    setTheme.mockReset();
    openInternalPath.mockReset();
    Object.assign(settingsState, structuredClone(defaultSettings));

    patch.mockImplementation(async (draft: Settings) => {
      Object.assign(settingsState, JSON.parse(JSON.stringify(draft)) as Settings);
    });
    reset.mockImplementation(async () => {
      const next = structuredClone(defaultSettings);
      Object.assign(settingsState, next);
      return next;
    });

    const router = createSettingsRouter();
    await router.push('/settings');
    await router.isReady();

    const wrapper = mount(createRoot(), {
      global: {
        plugins: [router, i18n],
      },
    });

    await nextTick();
    await wrapper.get('input[type="checkbox"]').setValue(false);
    await wrapper.get('form').trigger('submit');
    await nextTick();

    expect(patch).toHaveBeenCalledTimes(1);
    expect(patch.mock.calls[0][0].transcription.fp16).toBe(false);
    expect(showToast).toHaveBeenCalledWith('Settings saved!', { style: 'success' });

    const resetButton = wrapper
      .findAll('button')
      .find(button => button.text().includes('Reset'));

    expect(resetButton).toBeTruthy();
    await resetButton!.trigger('click');
    await nextTick();

    expect(reset).toHaveBeenCalledTimes(1);
    expect(showToast).toHaveBeenCalledWith('Reset settings to defaults.', { style: 'success' });
    expect(wrapper.get('input[type="checkbox"]').element).toBeInstanceOf(HTMLInputElement);
  });

  it('commits a pending API key through the global save action', async () => {
    patch.mockReset();
    showToast.mockReset();
    setAiApiKey.mockReset();
    runProbe.mockReset();
    aiApiKeyDraft.value = null;
    aiApiKeyDirty.value = false;
    patch.mockImplementation(async () => {});
    setAiApiKey.mockImplementation(async () => {});
    runProbe.mockImplementation(async () => {});

    const router = createSettingsRouter();
    await router.push('/settings/translation');
    await router.isReady();

    const wrapper = mount(createRoot(), {
      global: {
        plugins: [router, i18n],
      },
    });

    await nextTick();
    await wrapper.get('#deepseek-api-key').setValue('sk-test');
    await wrapper.get('form').trigger('submit');
    await nextTick();

    expect(setAiApiKey).toHaveBeenCalledWith('sk-test');
    expect(runProbe).toHaveBeenCalled();
    // The typed key stays visible after saving so the save does not look like data loss.
    expect(aiApiKeyDirty.value).toBe(false);
    expect((wrapper.get('#deepseek-api-key').element as HTMLInputElement).value).toBe('sk-test');
  });
});
