import { create } from 'zustand';
import { bridge } from '../ipc/bridge';
import type { OpenedImage, Params, UiError, VectorFrame, JobMessage, ExportOptions, View } from '../ipc/types';

type Phase = 'empty' | 'loading' | 'vectorizing' | 'ready' | 'cancelled' | 'error' | 'exporting';
type Recent = { name: string; path: string; thumbnail: string };
type Large = { width: number; height: number; retry: (limit: number) => Promise<void> };
type State = {
  image: OpenedImage | null; params: Params | null; doc: VectorFrame | null; completed: VectorFrame | null;
  phase: Phase; request: number; stage: string | null; fraction: number; preview: boolean;
  error: UiError | null; large: Large | null; recents: Recent[]; exported: string | null;
  view: View; zoom: number | null; pan: [number, number]; startedAt: number;
  openPicker: () => Promise<void>; openPath: (path: string, limit?: number) => Promise<void>;
  openFile: (file: File) => Promise<void>; close: () => void; start: () => void;
  paste: () => Promise<void>;
  update: (patch: Partial<Params>) => void; cancel: () => void;
  setView: (view: View) => void; setZoom: (zoom: number | null) => void; setPan: (pan: [number, number]) => void;
  export: (options: ExportOptions) => Promise<void>; dismiss: () => void;
};
let serial = 0, loadSerial = 0;
let timer: ReturnType<typeof setTimeout> | undefined;
let listening: Promise<unknown> | undefined;
function errorOf(error: unknown): UiError {
  if (typeof error === 'object' && error && 'code' in error && 'msg' in error) return error as UiError;
  return { code: 'E_INTERNAL', msg: error instanceof Error ? error.message : String(error) };
}
function readRecents(): Recent[] {
  try { const entries: unknown = JSON.parse(localStorage.getItem('antivector.recents') ?? '[]');
    return Array.isArray(entries) ? entries.filter(x => x && typeof x.path === 'string' && typeof x.name === 'string' && typeof x.thumbnail === 'string').slice(0, 10) : [];
  } catch { return []; }
}
function handle(message: JobMessage) {
  const state = useAppStore.getState();
  if (message.session_id !== state.image?.session_id || message.request_id !== state.request || state.phase === 'cancelled') return;
  if (message.kind === 'progress') useAppStore.setState({ stage: message.stage ?? null, fraction: message.frac ?? 0 });
  if (message.kind === 'preview' && message.doc) useAppStore.setState({ doc: message.doc, preview: true });
  if (message.kind === 'done' && message.doc) useAppStore.setState({ doc: message.doc, completed: message.doc, phase: 'ready', preview: false, fraction: 1 });
  if (message.kind === 'error') { const error = message.error ?? { code: 'E_INTERNAL', msg: 'Falha desconhecida' };
    useAppStore.setState({ phase: error.code === 'E_CANCELLED' ? 'cancelled' : 'error', error: error.code === 'E_CANCELLED' ? null : error, doc: state.completed, preview: false }); }
}
function ensureListening() { listening ??= bridge.subscribe(handle); return listening; }
async function run(request: number) {
  const state = useAppStore.getState();
  if (!state.image || !state.params || state.request !== request) return;
  try { await ensureListening(); if (useAppStore.getState().request !== request) return;
    await bridge.vectorize(state.image.session_id, state.params, request);
  } catch (error) { if (useAppStore.getState().request === request) useAppStore.setState({ error: errorOf(error), phase: 'error', doc: state.completed }); }
}
function schedule(delay: number) {
  const state = useAppStore.getState(); if (!state.image || !state.params) return;
  clearTimeout(timer); const request = ++serial;
  void bridge.cancel(state.image.session_id).catch(() => {});
  useAppStore.setState({ request, phase: 'vectorizing', stage: null, fraction: 0, error: null,
    doc: state.completed, preview: false, startedAt: performance.now(), exported: null });
  timer = setTimeout(() => { void run(request); }, delay);
}
async function load(loader: (limit?: number) => Promise<OpenedImage>, path?: string, limit?: number) {
  const token = ++loadSerial; const previous = useAppStore.getState(); clearTimeout(timer);
  useAppStore.setState({ phase: 'loading', request: ++serial, error: null, large: null });
  if (previous.image) void bridge.cancel(previous.image.session_id).catch(() => {});
  try { await ensureListening(); const image = await loader(limit);
    if (token !== loadSerial) { void bridge.close(image.session_id).catch(() => {}); return; }
    if (previous.image) void bridge.close(previous.image.session_id).catch(() => {});
    const recents = path ? [{ name: image.name, path, thumbnail: image.thumb_png_b64 }, ...previous.recents.filter(r => r.path !== path)].slice(0, 10) : previous.recents;
    if (path) { try { localStorage.setItem('antivector.recents', JSON.stringify(recents)); } catch { /* A full recent-list storage never blocks image opening. */ } }
    useAppStore.setState({ image, params: image.params, doc: null, completed: null, zoom: null, pan: [0, 0], recents, large: null }); schedule(0);
  } catch (error) { if (token !== loadSerial) return; const failure = errorOf(error);
    useAppStore.setState({ phase: 'error', error: failure, large: failure.code === 'E_TOO_LARGE' && failure.width && failure.height
      && failure.width * failure.height <= 64_000_000 ? { width: failure.width, height: failure.height, retry: size => load(loader, path, size) } : null }); }
}

export const useAppStore = create<State>((set, get) => ({
  image: null, params: null, doc: null, completed: null, phase: 'empty', request: 0, stage: null,
  fraction: 0, preview: false, error: null, large: null, recents: readRecents(), exported: null,
  view: 'side', zoom: null, pan: [0, 0], startedAt: 0,
  openPicker: async () => { try { const path = await bridge.pickImage(); if (path) await get().openPath(path); } catch (error) { set({ error: errorOf(error) }); } },
  openPath: (path, limit) => load(size => bridge.openPath(path, size), path, limit),
  paste: () => load(limit => bridge.openClipboard(limit)),
  openFile: async file => { try { const bytes = Array.from(new Uint8Array(await file.arrayBuffer())); await load(limit => bridge.openBytes(file.name, bytes, limit)); } catch (error) { set({ error: errorOf(error), phase: 'error' }); } },
  close: () => { const image = get().image; ++loadSerial; clearTimeout(timer); if (image) void bridge.close(image.session_id).catch(() => {});
    set({ image: null, params: null, doc: null, completed: null, phase: 'empty', request: ++serial, error: null, large: null, exported: null }); },
  start: () => schedule(0), update: patch => { const params = get().params; if (!params) return; set({ params: { ...params, ...patch } }); schedule(250); },
  cancel: () => { clearTimeout(timer); const state = get(); if (state.image) void bridge.cancel(state.image.session_id).catch(() => {});
    set({ phase: 'cancelled', request: ++serial, doc: state.completed, preview: false, stage: null }); },
  setView: view => set({ view }), setZoom: zoom => set({ zoom: zoom === null ? null : Math.min(6400, Math.max(1, zoom)) }),
  setPan: pan => set({ pan }), dismiss: () => set({ error: null, large: null, exported: null }),
  export: async options => { const state = get(); if (!state.image || !state.completed) return;
    try { const path = await bridge.savePath(state.image.name, options); if (!path) return; set({ phase: 'exporting', error: null });
      await bridge.export(state.image.session_id, path, options); set({ phase: 'ready', exported: path });
    } catch (error) { set({ phase: 'error', error: errorOf(error) }); }
  },
}));
