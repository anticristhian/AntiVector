import { invoke } from '@tauri-apps/api/core';
import { t } from '../i18n';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import type { OpenedImage, Params, JobMessage, ExportOptions, EditOp, DocumentState, FidelityEstimate } from './types';

export const bridge = {
  async subscribe(handler: (message: JobMessage) => void) {
    const removers = await Promise.all((['progress', 'preview', 'done', 'error'] as const)
      .map(kind => listen<Omit<JobMessage, 'kind'>>(kind, e => handler({ ...e.payload, kind }))));
    return () => removers.forEach(remove => remove());
  },
  async pickImage() { return open({ multiple: false, directory: false, title: t('Abrir imagem'),
    filters: [{ name: 'AntiVector', extensions: ['png', 'jpg', 'jpeg', 'bmp', 'gif', 'tif', 'tiff', 'webp', 'avproj'] }] }); },
  openPath: (path: string, reduceTo?: number) => invoke<OpenedImage>(/\.avproj$/i.test(path) ? 'open_project' : 'open_image', { path, reduceTo }),
  openBytes: (name: string, bytes: number[], reduceTo?: number) => invoke<OpenedImage>(/\.avproj$/i.test(name) ? 'open_project_bytes' : 'open_image_bytes', { name, bytes, reduceTo }),
  openClipboard: (reduceTo?:number) => invoke<OpenedImage>('open_clipboard',{reduceTo}),
  vectorize: (sessionId: number, params: Params, requestId: number) => invoke<number>('vectorize', { sessionId, params, requestId }),
  edit: (sessionId: number, op: EditOp, requestId: number) => invoke<number>('edit', { sessionId, op, requestId }),
  travel: (sessionId: number, command: 'undo' | 'redo' | 'current_document') => invoke<DocumentState>(command, { sessionId }),
  fidelity: (sessionId: number) => invoke<FidelityEstimate>('estimate_fidelity', { sessionId }),
  pickProjectPath: (name: string) => save({ title: t('Salvar projeto'), defaultPath: `${name.replace(/\.[^.]+$/, '')}.avproj`, filters: [{ name: 'AntiVector', extensions: ['avproj'] }] }),
  saveProject: (sessionId: number, path: string, name: string) => invoke<void>('save_project', { sessionId, path, name }),
  cancel: (sessionId: number) => invoke<void>('cancel', { sessionId }),
  close: (sessionId: number) => invoke<void>('close_session', { sessionId }),
  async savePath(name: string, options: ExportOptions) {
    return save({ defaultPath: `${name.replace(/\.[^.]+$/, '')}-vector.${options.format}`,
      filters: [{ name: options.format.toUpperCase(), extensions: [options.format] }] });
  },
  export: (sessionId: number, path: string, options: ExportOptions) => invoke<void>('export', { sessionId, path, options }),
  reveal: (path: string) => revealItemInDir(path),
};
