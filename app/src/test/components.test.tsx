import { beforeEach, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import AppShell from '../AppShell';
import { Controls } from '../components/Controls';
import { ExportDialog } from '../components/ExportDialog';
import { PalettePanel } from '../components/PalettePanel';
import { CanvasStage } from '../components/CanvasStage';
import { useAppStore } from '../state/store';
import { bridge } from '../ipc/bridge';
import { opened, frame } from './fixtures';
beforeEach(() => { useAppStore.setState(useAppStore.getInitialState(), true); });
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
