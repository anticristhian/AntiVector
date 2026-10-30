import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from './store';

// Opt-in QA, no analytics/network and no product-facing diagnostic controls.
export function useUiPerformance() {
  useEffect(() => {
    if (import.meta.env.VITE_AV_MEASURE_UI !== '1') return;
    type Sample = { request: number; width: number; height: number; start: number; stop: number; max: number; count: number; outcome: string; longtasks: object[]; longframes: object[] };
    let sample: Sample | null = null; let timer: ReturnType<typeof setTimeout> | undefined;
    const supported = typeof PerformanceObserver !== 'undefined' && PerformanceObserver.supportedEntryTypes.includes('longtask');
    const collect = (entries: PerformanceEntry[]) => { if (!sample) return;
      for (const entry of entries) if (entry.startTime >= sample.start && entry.startTime <= sample.stop) { sample.max = Math.max(sample.max, entry.duration); sample.count++; sample.longtasks.push(entry.toJSON()); } };
    const observer = supported ? new PerformanceObserver(list => collect(list.getEntries())) : null;
    observer?.observe({ type: 'longtask' });
    const collectFrames = (entries: PerformanceEntry[]) => { if (!sample) return;
      for (const entry of entries) if (entry.startTime >= sample.start && entry.startTime <= sample.stop) sample.longframes.push(entry.toJSON()); };
    const frames = PerformanceObserver.supportedEntryTypes.includes('long-animation-frame') ? new PerformanceObserver(list => collectFrames(list.getEntries())) : null;
    frames?.observe({ type: 'long-animation-frame' });
    const unsubscribe = useAppStore.subscribe((state, previous) => {
      if (state.phase === 'loading' && state.request !== previous.request || state.phase === 'vectorizing' && state.request !== previous.request) {
        clearTimeout(timer); const prior=previous.phase==='loading'?sample:null;
        sample = { request: state.request, width: state.image?.w ?? 0, height: state.image?.h ?? 0, start:prior?.start??performance.now(), stop: Infinity, max: prior?.max??0, count: prior?.count??0, outcome: '', longtasks: prior?.longtasks??[], longframes: prior?.longframes??[] };
        document.documentElement.dataset.avMaxDrawMs = '0';
      }
      if (sample && ['ready', 'cancelled', 'error'].includes(state.phase) && ['vectorizing', 'loading'].includes(previous.phase)) {
        sample.width = state.image?.w ?? 0; sample.height = state.image?.h ?? 0;
        sample.stop = performance.now() + 600; sample.outcome = state.phase; clearTimeout(timer);
        timer = setTimeout(() => { if (!sample) return; collect(observer?.takeRecords() ?? []); collectFrames(frames?.takeRecords() ?? []);
          const report = { ...sample, duration_ms: sample.stop - sample.start, longtask_supported: supported,
            longtask_threshold_ms: 50, max_longtask_ms: supported ? sample.max : null, max_draw_ms: Number(document.documentElement.dataset.avMaxDrawMs ?? 0),
            document_visible: document.visibilityState === 'visible', frontend_profile: import.meta.env.DEV ? 'development' : 'production' };
          document.documentElement.dataset.avUiMetrics = JSON.stringify(report);
          void invoke('record_ui_metrics', { report }).catch(() => {});
        }, 650);
      }
    });
    return () => { clearTimeout(timer); unsubscribe(); observer?.disconnect(); frames?.disconnect(); };
  }, []);
}
