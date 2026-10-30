import { act, renderHook } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useUiPerformance } from '../state/performance';
import { useAppStore } from '../state/store';
import { opened } from './fixtures';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => {}) }));
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.unstubAllEnvs(); });

it('counts a long task overlapping the beginning of loading and excludes tasks outside the interval', () => {
  vi.useFakeTimers(); vi.stubEnv('VITE_AV_MEASURE_UI', '1');
  vi.spyOn(performance, 'now').mockReturnValue(100);
  let deliver: (entries: PerformanceEntry[]) => void = () => {};
  class Observer {
    static supportedEntryTypes = ['longtask'];
    constructor(callback: PerformanceObserverCallback) {
      deliver = entries => callback({ getEntries: () => entries } as PerformanceObserverEntryList, this as unknown as PerformanceObserver);
    }
    observe() {} disconnect() {} takeRecords() { return []; }
  }
  vi.stubGlobal('PerformanceObserver', Observer);
  useAppStore.setState(useAppStore.getInitialState(), true);
  renderHook(useUiPerformance);
  act(() => useAppStore.setState({ phase: 'loading', request: 1, image: opened }));
  act(() => useAppStore.setState({ phase: 'ready' }));
  const entry = (startTime: number, duration: number) => ({ startTime, duration, toJSON: () => ({ startTime, duration }) }) as PerformanceEntry;
  act(() => { deliver([entry(90, 60), entry(10, 20), entry(1000, 60)]); vi.advanceTimersByTime(650); });
  expect(invoke).toHaveBeenCalledWith('record_ui_metrics', { report: expect.objectContaining({ count: 1, max_longtask_ms: 60 }) });
});
