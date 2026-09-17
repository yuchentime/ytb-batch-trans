import { flushPromises, mount, RouterLinkStub } from '@vue/test-utils';
import { createMemoryHistory, createRouter } from 'vue-router';
import { defineComponent, reactive, ref, nextTick } from 'vue';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import TheHeader from '../../src/components/TheHeader.vue';
import { i18n } from '../../src/i18n';
import { createDefaultInputFilterSettings } from '../../src/helpers/inputFilters';
import { useTranscriptionStore } from '../../src/stores/transcription';

const startTranscriptionBatch = vi.fn();
const showToast = vi.fn();
const toggleClipboard = vi.fn();
const markSeen = vi.fn();
const poll = vi.fn();
const clipboardContent = ref('');
const watchClipboardState = reactive({
  isActive: false,
});
const settingsState = reactive({
  input: {
    autoFillClipboard: false,
    preferVideoInMixedLinks: false,
  },
  inputFilters: createDefaultInputFilterSettings(),
});

vi.mock('../../src/stores/media/media', () => ({
  useMediaStore: () => ({
    startTranscriptionBatch,
  }),
}));

vi.mock('../../src/stores/toast', () => ({
  useToastStore: () => ({
    showToast,
  }),
}));

vi.mock('../../src/stores/watchClipboard', () => ({
  useWatchClipboardStore: () => ({
    get isActive() {
      return watchClipboardState.isActive;
    },
    toggle: toggleClipboard,
    hasSeen: vi.fn(() => false),
    markSeen,
  }),
}));

vi.mock('../../src/stores/settings', () => ({
  useSettingsStore: () => ({
    settings: settingsState,
  }),
}));

vi.mock('../../src/composables/useClipboard', () => ({
  useClipboard: () => ({
    content: clipboardContent,
    poll,
  }),
}));

function createBlankView() {
  return defineComponent({
    template: '<div />',
  });
}

describe('TheHeader', () => {
  beforeEach(() => {
    useTranscriptionStore().setProbe(null);
    startTranscriptionBatch.mockReset();
    showToast.mockReset();
    toggleClipboard.mockReset();
    markSeen.mockReset();
    poll.mockReset();
    clipboardContent.value = '';
    watchClipboardState.isActive = false;
    settingsState.input.autoFillClipboard = false;
    settingsState.inputFilters = createDefaultInputFilterSettings();
  });

  async function createRouterForHeader() {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: '/',
          name: 'home',
          component: createBlankView(),
        },
        {
          path: '/settings',
          name: 'settings.transcription',
          component: createBlankView(),
        },
        {
          path: '/setup',
          name: 'setup',
          component: createBlankView(),
        },
        {
          path: '/input-filters',
          name: 'input-filters',
          component: createBlankView(),
        },
      ],
    });

    await router.push('/');
    await router.isReady();
    return router;
  }

  it('navigates the settings button to the downloads tab route', async () => {
    const router = await createRouterForHeader();

    const wrapper = mount(TheHeader, {
      global: {
        plugins: [router, i18n],
        stubs: {
          RouterLink: RouterLinkStub,
        },
      },
    });

    const links = wrapper.findAllComponents(RouterLinkStub).map(link => link.props('to'));
    expect(links).toContainEqual({ name: 'settings.transcription' });
  });

  it('queues parsed urls from the input field', async () => {
    const router = await createRouterForHeader();
    const wrapper = mount(TheHeader, {
      global: {
        plugins: [router, i18n],
      },
    });

    await wrapper.get('#queue-url-input').setValue('https://example.com/a https://example.com/b');

    const addButton = wrapper.findAll('button').find(button => button.text().trim() === 'Add');
    expect(addButton).toBeTruthy();
    await addButton!.trigger('click');

    expect(startTranscriptionBatch).toHaveBeenCalledWith([
      'https://example.com/a',
      'https://example.com/b',
    ]);
    expect((wrapper.get('#queue-url-input').element as HTMLInputElement).value).toBe('');
  });

  it('blocks adding links while the environment is not ready', async () => {
    const transcriptionStore = useTranscriptionStore();
    transcriptionStore.setProbe({
      whisperFound: false,
      cudaAvailable: true,
      model: 'small',
      modelCached: false,
      apiKeyConfigured: false,
      ffmpegPath: 'ffmpeg',
      ffprobePath: 'ffprobe',
      logDir: '/logs',
      logFile: '/logs/transcribe.log',
      logSizeBytes: 0,
    });

    const router = await createRouterForHeader();
    const wrapper = mount(TheHeader, {
      global: {
        plugins: [router, i18n],
      },
    });

    await wrapper.get('#queue-url-input').setValue('https://example.com/a');

    const addButton = wrapper.findAll('button').find(button => button.text().trim() === 'Add');
    expect(addButton!.attributes('disabled')).toBeDefined();

    await wrapper.get('form').trigger('submit');

    expect(startTranscriptionBatch).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalled();
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('setup');
  });

  it('uses clipboard content as the placeholder when it contains a valid url', async () => {
    clipboardContent.value = 'https://example.com/from-clipboard';
    const router = await createRouterForHeader();
    const wrapper = mount(TheHeader, {
      global: {
        plugins: [router, i18n],
      },
    });

    expect((wrapper.get('#queue-url-input').element as HTMLInputElement).placeholder)
      .toBe('https://example.com/from-clipboard');
  });

  it('highlights input filters and navigates to the input filters route', async () => {
    settingsState.inputFilters.matchFilters = '!is_live';
    const router = await createRouterForHeader();
    const wrapper = mount(TheHeader, {
      global: {
        plugins: [router, i18n],
      },
    });

    const filtersButton = wrapper.findAll('button')
      .find(button => button.text().replace(/\s+/g, ' ').includes('Filters'));

    expect(wrapper.text()).toContain('Filters');
    expect(filtersButton).toBeTruthy();
    expect(filtersButton?.classes()).toContain('text-primary');
    await filtersButton!.trigger('click');
    await nextTick();
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(router.currentRoute.value.name).toBe('input-filters');
  });
});
