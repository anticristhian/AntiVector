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
