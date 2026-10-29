import { beforeEach, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import AppShell from '../AppShell';
import { Controls } from '../components/Controls';
import { ExportDialog } from '../components/ExportDialog';
import { useAppStore } from '../state/store';
import { bridge } from '../ipc/bridge';
import { opened } from './fixtures';
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
it('announces loading and export progress while preventing conflicting actions', () => {
  useAppStore.setState({ phase: 'loading' }); const { rerender } = render(<AppShell />);
  expect(screen.getByRole('dialog', { name: 'Abrindo imagem' })).toHaveAttribute('aria-busy', 'true');
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
