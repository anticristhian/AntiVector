import { beforeEach, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import AppShell from '../AppShell';
import { Controls } from '../components/Controls';
import { ExportDialog } from '../components/ExportDialog';
import { PalettePanel } from '../components/PalettePanel';
import { CanvasStage } from '../components/CanvasStage';
import { useAppStore } from '../state/store';
import { bridge } from '../ipc/bridge';
import { opened, frame } from './fixtures';
import { usePreferences } from '../i18n';
beforeEach(() => { useAppStore.setState(useAppStore.getInitialState(), true); usePreferences.setState({ language: 'pt-BR', theme: 'light' }); });
it('uses the worker bitmap and closes resources, including a late result after unmount', () => {
  useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready' });
  const view = render(<CanvasStage />);
  type BitmapWorker = { onmessage: (event: { data: ImageBitmap }) => void; postMessage: ReturnType<typeof vi.fn>; terminate: ReturnType<typeof vi.fn> };
  const workers = (Worker as unknown as { instances: BitmapWorker[] }).instances;
  const worker = workers[workers.length - 1];
  expect(worker.postMessage).toHaveBeenCalledWith(opened.original_png_b64);
  const bitmap = { close: vi.fn() } as unknown as ImageBitmap;
  act(() => worker.onmessage({ data: bitmap }));
  expect(document.querySelector('canvas')!.getContext('2d')!.drawImage).toHaveBeenCalledWith(bitmap, 0, 0, opened.w, opened.h);
  view.unmount(); expect(worker.terminate).toHaveBeenCalledOnce(); expect(bitmap.close).toHaveBeenCalledOnce();
  const late = { close: vi.fn() } as unknown as ImageBitmap;
  act(() => worker.onmessage({ data: late })); expect(late.close).toHaveBeenCalledOnce();
});
it('switches language and theme without replacing the active document', () => {
  useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready' });
  render(<AppShell />);
  fireEvent.change(screen.getByLabelText('Idioma'), { target: { value: 'en' } });
  expect(screen.getByRole('button', { name: /Save project/ })).toBeVisible();
  expect(screen.getByRole('slider', { name: 'Detail' })).toBeVisible();
  expect(document.documentElement.lang).toBe('en');
  fireEvent.change(screen.getByLabelText('Theme'), { target: { value: 'dark' } });
  expect(document.documentElement.dataset.theme).toBe('dark');
  expect(useAppStore.getState().completed).toBe(frame);
  fireEvent.click(screen.getByRole('button', { name: /^Export/ }));
  expect(screen.getByRole('dialog', { name: 'Export vector' })).toBeVisible();
});
it('contains dialog focus and restores the invoking export control', () => {
  useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready' });
  render(<AppShell />); const button = screen.getByRole('button', { name: /^Exportar/ }); button.focus(); fireEvent.click(button);
  expect(screen.getByRole('dialog')).toContainElement(document.activeElement as HTMLElement);
  const close = screen.getByLabelText('Fechar exportação'); close.focus(); fireEvent.keyDown(close, { key: 'Tab', shiftKey: true });
  expect(screen.getByRole('button', { name: 'Escolher destino…' })).toHaveFocus();
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' }); expect(button).toHaveFocus();
});
it('edits a region and changes comparison views using keyboard controls', () => {
  const edit = vi.fn(async () => {}); useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready', edit });
  render(<CanvasStage />);
  fireEvent.change(screen.getByLabelText('Selecionar região pelo teclado'), { target: { value: '0' } });
  fireEvent.click(screen.getByRole('button', { name: 'Remover região selecionada' }));
  expect(edit).toHaveBeenCalledWith({ kind: 'remove_region', region: 0 });
  fireEvent.keyDown(screen.getByRole('tab', { name: 'Lado a lado' }), { key: 'ArrowRight' });
  expect(screen.getByRole('tab', { name: 'Slider' })).toHaveFocus(); expect(useAppStore.getState().view).toBe('slider');
});
it('zooms from fit consistently using buttons or keyboard shortcuts', () => {
  useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready' });
  render(<CanvasStage />); fireEvent.click(screen.getByRole('button', { name: 'Aumentar zoom' }));
  const buttonZoom=useAppStore.getState().zoom;
  fireEvent.keyDown(document.body, { key: '0' }); expect(useAppStore.getState().zoom).toBeNull();
  fireEvent.keyDown(document.body, { key: '+' }); expect(useAppStore.getState().zoom).toBe(buttonZoom);
});
it('offers a labeled opening action and keeps export disabled without a document', () => {
  const picker = vi.fn(async () => {}); useAppStore.setState({ openPicker: picker }); render(<AppShell />);
  fireEvent.click(screen.getByRole('button', { name: /Abrir imagem/ })); expect(picker).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('button', { name: /Exportar/ })).toBeDisabled(); expect(screen.getByText('Solte sua imagem aqui')).toBeVisible();
});
it('maps detail and smoothness to the documented pixel parameters', () => {
  const update = vi.fn(); useAppStore.setState({ image: opened, params: opened.params, update }); render(<Controls />);
  fireEvent.change(screen.getByRole('slider', { name: 'Detalhe' }), { target: { value: '0.8' } });
  expect(update).toHaveBeenCalledWith({ detail: 0.8, min_region_area: 2 });
  fireEvent.change(screen.getByRole('slider', { name: 'Suavidade' }), { target: { value: '0.7' } });
  expect(update).toHaveBeenCalledWith(expect.objectContaining({ smoothness: 0.7, fit_tolerance_px: 1.14 }));
});
it('recolors the selected palette entry and provides document undo from a focused slider', () => {
  const edit = vi.fn(async () => {}), travel = vi.fn(async () => {});
  useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready', selectedColor: 0, edit, travel, history: { undo_depth: 1, redo_depth: 0 } });
  const { unmount } = render(<PalettePanel />);
  fireEvent.change(screen.getByLabelText('Nova cor'), { target: { value: '#4080c0' } }); fireEvent.click(screen.getByRole('button', { name: 'Aplicar cor' }));
  expect(edit).toHaveBeenCalledWith({ kind: 'recolor_palette', color: 0, rgb: [64, 128, 192] }); unmount();
  render(<AppShell />); fireEvent.keyDown(screen.getByRole('slider', { name: 'Detalhe' }), { key: 'z', ctrlKey: true }); expect(travel).toHaveBeenCalledWith('undo');
});
it('erases a hit region with Shift and selects palette colors with the eyedropper', () => {
  const edit = vi.fn(async () => {}); useAppStore.setState({ image: opened, params: opened.params, doc: frame, completed: frame, phase: 'ready', tool: 'erase', edit });
  const context = document.createElement('canvas').getContext('2d')!; vi.spyOn(context, 'isPointInPath').mockReturnValue(true);
  render(<CanvasStage />); const canvas = screen.getByLabelText('Pré-visualização vetorial');
  fireEvent.pointerDown(canvas, { button: 0, shiftKey: true, clientX: 100, clientY: 100 }); expect(edit).toHaveBeenCalledWith({ kind: 'remove_region', region: 0 });
  fireEvent.click(screen.getByRole('button', { name: 'Conta-gotas' })); fireEvent.pointerDown(canvas, { button: 0, clientX: 100, clientY: 100 });
  expect(useAppStore.getState().selectedColor).toBe(0); vi.mocked(context.isPointInPath).mockReturnValue(false);
});
it('announces loading and export progress while preventing conflicting actions', () => {
  useAppStore.setState({ phase: 'loading' }); const { rerender } = render(<AppShell />);
  expect(screen.getByRole('dialog', { name: 'Abrindo imagem ou projeto' })).toHaveAttribute('aria-busy', 'true');
  expect(screen.getByRole('progressbar', { name: 'Carregamento da imagem' })).toBeVisible();
  useAppStore.setState({ phase: 'exporting' }); rerender(<AppShell />);
  expect(screen.getByRole('dialog', { name: 'Exportando vetor' })).toBeVisible();
  expect(screen.getByRole('button', { name: /Exportar/ })).toBeDisabled();
});
it('color grouping selects exact stacking instead of changing paint order silently', () => {
  render(<ExportDialog onClose={vi.fn()} />); fireEvent.click(screen.getByRole('checkbox', { name: 'Agrupar caminhos por cor' }));
  expect(screen.getByRole('checkbox', { name: 'Sobreposição opaca segura' })).not.toBeChecked();
  expect(screen.getByRole('checkbox', { name: 'Sobreposição opaca segura' })).toBeDisabled();
});
it('exports only after the native destination dialog returns a path', async () => {
  vi.spyOn(bridge, 'savePath').mockResolvedValue(null); const exportCall = vi.spyOn(bridge, 'export').mockResolvedValue();
  useAppStore.setState({ image: opened, completed: { width: 128, height: 128, shapes: [], palette: [], stats: { paths: 0, nodes: 0, colors: 0 } } });
  await useAppStore.getState().export({ format: 'svg', precision: 2, overlap: true, group_by_color: false }); expect(exportCall).not.toHaveBeenCalled();
});
