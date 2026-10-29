import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { useAppStore } from '../state/store';
import { bridge } from '../ipc/bridge';
import type { JobMessage } from '../ipc/types';
import { opened, frame } from './fixtures';
let handle: (message: JobMessage) => void;
beforeEach(() => {
  vi.useFakeTimers(); useAppStore.setState(useAppStore.getInitialState(), true);
  vi.spyOn(bridge, 'subscribe').mockImplementation(async fn => { handle = fn; return () => {}; });
  vi.spyOn(bridge, 'openPath').mockResolvedValue(structuredClone(opened));
  vi.spyOn(bridge, 'cancel').mockResolvedValue(); vi.spyOn(bridge, 'close').mockResolvedValue(); vi.spyOn(bridge, 'vectorize').mockResolvedValue(1);
});
afterEach(() => { useAppStore.getState().close(); vi.useRealTimers(); });
async function open() { await useAppStore.getState().openPath('logo.png'); await vi.runOnlyPendingTimersAsync(); }
function done(request = useAppStore.getState().request) { handle({ kind: 'done', session_id: 1, request_id: request, job_id: 1, doc: frame }); }
describe('session state', () => {
  it('opens projects without discarding restored edits through automatic vectorization', async () => {
    vi.mocked(bridge.openPath).mockResolvedValue({ ...opened, restored: { doc: frame, params: { ...opened.params, seed: '18446744073709551615' }, history: { undo_depth: 0, redo_depth: 0 } } });
    await useAppStore.getState().openPath('work.avproj'); await vi.runOnlyPendingTimersAsync();
    expect(bridge.vectorize).not.toHaveBeenCalled(); expect(useAppStore.getState().completed).toEqual(frame);
    expect(useAppStore.getState().params?.seed).toBe('18446744073709551615'); expect(useAppStore.getState().phase).toBe('ready');
  });
  it('publishes edit history with its document and restores parameters on undo', async () => {
    await open(); done(); vi.spyOn(bridge, 'edit').mockResolvedValue(2);
    await useAppStore.getState().edit({ kind: 'recolor_palette', color: 0, rgb: [10, 20, 30] });
    const request = useAppStore.getState().request;
    handle({ kind: 'done', session_id: 1, request_id: request - 1, job_id: 1, doc: frame, history: { undo_depth: 99, redo_depth: 0 } });
    expect(useAppStore.getState().history.undo_depth).toBe(0);
    handle({ kind: 'done', session_id: 1, request_id: request, job_id: 2, doc: frame, params: { ...opened.params, smoothness: 0.7 }, history: { undo_depth: 1, redo_depth: 0 } });
    expect(useAppStore.getState().history.undo_depth).toBe(1);
    vi.spyOn(bridge, 'travel').mockResolvedValue({ doc: frame, params: opened.params, history: { undo_depth: 0, redo_depth: 1 } });
    await useAppStore.getState().travel('undo'); expect(bridge.travel).toHaveBeenCalledWith(1, 'undo');
    expect(useAppStore.getState().params?.smoothness).toBe(0.5); expect(useAppStore.getState().history.redo_depth).toBe(1);
  });
  it('saves projects only after a native destination is chosen', async () => {
    await open(); done(); const path = vi.spyOn(bridge, 'pickProjectPath').mockResolvedValue(null);
    const save = vi.spyOn(bridge, 'saveProject').mockResolvedValue();
    await useAppStore.getState().saveProject(); expect(save).not.toHaveBeenCalled();
    path.mockResolvedValue('work.avproj'); await useAppStore.getState().saveProject();
    expect(save).toHaveBeenCalledWith(1, 'work.avproj', 'logo.png'); expect(useAppStore.getState().notice).toBe('Projeto salvo.');
  });
  it('opens, vectorizes automatically and ignores stale job/session messages', async () => {
    await open(); expect(bridge.vectorize).toHaveBeenCalledTimes(1); expect(useAppStore.getState().phase).toBe('vectorizing');
    const request = useAppStore.getState().request;
    handle({ kind: 'done', session_id: 99, request_id: request, job_id: 1, doc: frame }); expect(useAppStore.getState().doc).toBeNull();
    done(request - 1); expect(useAppStore.getState().doc).toBeNull(); done(); expect(useAppStore.getState().phase).toBe('ready');
  });
  it('debounces slider changes and keeps the last completed result on cancellation', async () => {
    await open(); done(); const previous = useAppStore.getState().doc;
    useAppStore.getState().update({ smoothness: 0.6 }); useAppStore.getState().update({ smoothness: 0.7 });
    await vi.advanceTimersByTimeAsync(249); expect(bridge.vectorize).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1); expect(bridge.vectorize).toHaveBeenCalledTimes(2);
    const old = useAppStore.getState().request; useAppStore.getState().cancel(); done(old);
    expect(useAppStore.getState().phase).toBe('cancelled'); expect(useAppStore.getState().doc).toBe(previous);
  });
  it('shows a decode error and refuses unbounded large-image retry', async () => {
    vi.mocked(bridge.openPath).mockRejectedValue({ code: 'E_DECODE', msg: 'Arquivo inválido' });
    await useAppStore.getState().openPath('corrupt.png'); expect(useAppStore.getState().error?.code).toBe('E_DECODE');
    vi.mocked(bridge.openPath).mockRejectedValue({ code: 'E_TOO_LARGE', msg: 'Muito grande', width: 100000, height: 100000 });
    await useAppStore.getState().openPath('huge.png'); expect(useAppStore.getState().large).toBeNull();
  });
});
