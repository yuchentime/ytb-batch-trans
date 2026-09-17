import { listen } from '@tauri-apps/api/event';
import { useMediaStore } from '../../stores/media/media';
import { MediaAddPayload } from '../types/media';

export function registerMediaListeners() {
  const mediaStore = useMediaStore();

  void listen<MediaAddPayload>('media_add', (event) => {
    mediaStore.processMediaAddPayload(event.payload);
  });
}
