import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import type { OpenedImage, Params, JobMessage, ExportOptions } from './types';

export const bridge = {
  async subscribe(handler: (message: JobMessage) => void) {
    const removers = await Promise.all((['progress', 'preview', 'done', 'error'] as const)
      .map(kind => listen<Omit<JobMessage, 'kind'>>(kind, e => handler({ ...e.payload, kind }))));
    return () => removers.forEach(remove => remove());
  },
  async pickImage() { return open({ multiple: false, directory: false, title: 'Abrir imagem',
    filters: [{ name: 'Imagens', extensions: ['png', 'jpg', 'jpeg', 'bmp', 'gif', 'tif', 'tiff', 'webp'] }] }); },
  openPath: (path: string, reduceTo?: number) => invoke<OpenedImage>('open_image', { path, reduceTo }),
  openBytes: (name: string, bytes: number[], reduceTo?: number) => invoke<OpenedImage>('open_image_bytes', { name, bytes, reduceTo }),
  openClipboard: (reduceTo?:number) => invoke<OpenedImage>('open_clipboard',{reduceTo}),
  vectorize: (sessionId: number, params: Params, requestId: number) => invoke<number>('vectorize', { sessionId, params, requestId }),
  cancel: (sessionId: number) => invoke<void>('cancel', { sessionId }),
  close: (sessionId: number) => invoke<void>('close_session', { sessionId }),
  async savePath(name: string, options: ExportOptions) {
    return save({ defaultPath: `${name.replace(/\.[^.]+$/, '')}-vector.${options.format}`,
      filters: [{ name: options.format.toUpperCase(), extensions: [options.format] }] });
  },
  export: (sessionId: number, path: string, options: ExportOptions) => invoke<void>('export', { sessionId, path, options }),
  reveal: (path: string) => revealItemInDir(path),
};
