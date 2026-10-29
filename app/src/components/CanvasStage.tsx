import { t } from '../i18n';
import { useEffect, useMemo, useRef, useState, type PointerEvent } from 'react';
import { useAppStore } from '../state/store';
import type { OpenedImage, VectorFrame, View } from '../ipc/types';

type Size = { width: number; height: number };
function useSize() {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<Size>({ width: 800, height: 500 });
  useEffect(() => { const element = ref.current; if (!element) return;
    const observer = new ResizeObserver(([entry]) => setSize({ width: entry.contentRect.width, height: entry.contentRect.height }));
    observer.observe(element); return () => observer.disconnect(); }, []);
  return { ref, size };
}

function PaintCanvas({ image, doc, bitmap, mode, width, height, scale, pan, split, highlight }:
  { image: OpenedImage; doc: VectorFrame | null; bitmap: ImageBitmap | null; mode: 'original' | 'vector' | 'outline' | 'slider' | 'difference';
    width: number; height: number; scale: number; pan: [number, number]; split: number; highlight: number | null }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const paths = useMemo(() => doc?.shapes.map(shape => ({ ...shape, path: new Path2D(shape.d) })) ?? [], [doc]);
  useEffect(() => { const canvas = ref.current; const ctx = canvas?.getContext('2d'); if (!canvas || !ctx) return;
    const drawStart=performance.now();
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * ratio); canvas.height = Math.round(height * ratio);
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0); ctx.clearRect(0, 0, width, height);
    const x = (width - image.w * scale) / 2 + pan[0], y = (height - image.h * scale) / 2 + pan[1];
    const original = () => { if (!bitmap) return; ctx.save(); ctx.translate(x, y); ctx.scale(scale, scale);
      ctx.imageSmoothingEnabled = scale <= 4; ctx.drawImage(bitmap, 0, 0, image.w, image.h); ctx.restore(); };
    if (mode === 'difference') { ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, width, height); }
    if (mode === 'original' || mode === 'slider' || mode === 'difference') original();
    if (mode === 'difference' && doc) {
      const offscreen = document.createElement('canvas'); offscreen.width = canvas.width; offscreen.height = canvas.height;
      const off = offscreen.getContext('2d'); if (off) {
        off.setTransform(ratio, 0, 0, ratio, 0, 0); off.fillStyle = '#fff'; off.fillRect(0, 0, width, height);
        off.translate(x, y); const ds = scale * image.w / doc.width; off.scale(ds, ds);
        for (const shape of paths) { off.fillStyle = shape.color; off.fill(shape.path, 'evenodd'); }
        ctx.globalCompositeOperation = 'difference'; ctx.drawImage(offscreen, 0, 0, width, height); ctx.globalCompositeOperation = 'source-over';
      }
    }
    if (mode !== 'original' && mode !== 'difference' && doc) { ctx.save();
      if (mode === 'slider') { ctx.beginPath(); ctx.rect(width * split / 100, 0, width, height); ctx.clip(); }
      ctx.translate(x, y); const docScale = scale * image.w / doc.width; ctx.scale(docScale, docScale);
      if (mode === 'slider') ctx.clearRect(0, 0, doc.width, doc.height);
      for (const shape of paths) {
        if (mode === 'outline') { ctx.strokeStyle = shape.color; ctx.lineWidth = 1 / docScale; ctx.stroke(shape.path);
          ctx.fillStyle = '#23324b'; for (const node of shape.nodes) { const [nx, ny] = node.point;
            if (node.corner) ctx.fillRect(nx - 2 / docScale, ny - 2 / docScale, 4 / docScale, 4 / docScale);
            else { ctx.beginPath(); ctx.arc(nx, ny, 2 / docScale, 0, Math.PI * 2); ctx.fill(); } }
        } else { ctx.fillStyle = shape.color; ctx.fill(shape.path, 'evenodd'); }
      } ctx.restore();
    }
    if (doc && highlight !== null) { const selected = paths.find(shape => shape.region === highlight); if (selected) {
      ctx.save(); ctx.translate(x, y); const ds = scale * image.w / doc.width; ctx.scale(ds, ds);
      ctx.fillStyle = '#4387ef44'; ctx.fill(selected.path, 'evenodd'); ctx.strokeStyle = '#2672d8'; ctx.lineWidth = 2 / ds; ctx.stroke(selected.path); ctx.restore();
    } }
    if(import.meta.env.VITE_AV_MEASURE_UI==='1') document.documentElement.dataset.avMaxDrawMs=String(Math.max(Number(document.documentElement.dataset.avMaxDrawMs??0),performance.now()-drawStart));
  }, [image, doc, bitmap, mode, width, height, scale, pan, split, paths, highlight]);
  return <canvas ref={ref} style={{ width, height }} aria-label={mode === 'original' ? t("Imagem original") : t("Pré-visualização vetorial")} />;
}

const names: Record<View, string> = { side: 'Lado a lado', slider: 'Slider', vector: 'Vetor', outline: 'Contorno', difference: 'Diferença' };
export function CanvasStage() {
  const { image, doc, view, setView, zoom, setZoom, pan, setPan, preview, tool, setTool, edit, selectColor, phase } = useAppStore();
  const { ref, size } = useSize();
  const [bitmap, setBitmap] = useState<{ session: number; image: ImageBitmap } | null>(null);
  const [split, setSplit] = useState(50);
  const [hovered, setHovered] = useState<{ doc: VectorFrame; region: number } | null>(null);
  const [selection, setSelection] = useState<{ doc: VectorFrame; region: number } | null>(null);
  const hitPaths = useMemo(() => doc?.shapes.map(shape => ({ ...shape, path: new Path2D(shape.d) })) ?? [], [doc]);
  const hitContext = useRef<CanvasRenderingContext2D | null>(null);
  const space = useRef(false);
  useEffect(() => { const down = (e: KeyboardEvent) => { if (e.code === 'Space' && !(e.target instanceof HTMLElement && e.target.closest('input,select,textarea,button,a,summary,[role=tab]'))) { space.current = true; e.preventDefault(); } };
    const up = (e: KeyboardEvent) => { if (e.code === 'Space') space.current = false; }; const blur = () => { space.current = false; };
    window.addEventListener('keydown', down); window.addEventListener('keyup', up); window.addEventListener('blur', blur);
    return () => { window.removeEventListener('keydown', down); window.removeEventListener('keyup', up); window.removeEventListener('blur', blur); }; }, []);
  const dragging = useRef<{ x: number; y: number; pan: [number, number] } | null>(null);
  useEffect(() => { if (!image) return; let disposed = false; let resource: ImageBitmap | undefined;
    const source = new Image(); source.src = `data:image/png;base64,${image.original_png_b64}`;
    void source.decode().then(() => createImageBitmap(source)).then(value => {
      resource = value; if (disposed) value.close(); else setBitmap({ session: image.session_id, image: value });
    }).catch(() => { if (!disposed) setBitmap(null); }); return () => { disposed = true; resource?.close(); }; }, [image]);
  const paneWidth = view === 'side' ? (size.width - 16) / 2 : size.width;
  const paneHeight = Math.max(120, size.height - 50);
  const fit = image ? Math.min((paneWidth - 40) / image.w, (paneHeight - 40) / image.h) : 1;
  const scale = zoom === null ? fit : zoom / 100;
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey || document.querySelector('[role=dialog]') || event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target instanceof HTMLTextAreaElement) return;
      if (event.key === '+' || event.key === '=') { event.preventDefault(); setZoom(scale * 125); }
      if (event.key === '-') { event.preventDefault(); setZoom(scale * 80); }
      if (event.key === '0') { event.preventDefault(); setZoom(null); setPan([0, 0]); }
    };
    window.addEventListener('keydown', keyboard); return () => window.removeEventListener('keydown', keyboard);
  }, [scale, setZoom, setPan]);
  if (!image) return null;
  const currentBitmap = bitmap?.session === image.session_id ? bitmap.image : null;
  const selectedRegion = selection?.doc === doc ? selection.region : null;
  const highlight = hovered?.doc === doc ? hovered.region : selectedRegion;
  const props = { image, doc, bitmap: currentBitmap, width: paneWidth, height: paneHeight, scale, pan, split, highlight };
  const editable = !!doc && !preview && !['vectorizing', 'loading', 'exporting', 'saving'].includes(phase);
  const hit = (e: PointerEvent<HTMLDivElement>) => {
    if (!doc || !image) return null;
    const canvas = (e.target as Element).closest('canvas'); if (!canvas) return null;
    hitContext.current ??= document.createElement('canvas').getContext('2d'); const context = hitContext.current; if (!context) return null;
    const rect = canvas.getBoundingClientRect(), ds = scale * image.w / doc.width;
    const x = (e.clientX - rect.left - (paneWidth - image.w * scale) / 2 - pan[0]) / ds;
    const y = (e.clientY - rect.top - (paneHeight - image.h * scale) / 2 - pan[1]) / ds;
    for (let i = hitPaths.length - 1; i >= 0; i--) if (context.isPointInPath(hitPaths[i].path, x, y, 'evenodd')) return hitPaths[i];
    return null;
  };
  return <section className="workspace" aria-label={t("Área de comparação")}>
    <div className="view-toolbar"><div className="tabs" role="tablist" aria-label={t("Vista")}>
      {(Object.keys(names) as View[]).map(mode => <button key={mode} role="tab" aria-selected={view === mode} tabIndex={view === mode ? 0 : -1} aria-controls="comparison-panel" onKeyDown={e => {
        const views = Object.keys(names) as View[];
        if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
        e.preventDefault(); const index = e.key === 'Home' ? 0 : e.key === 'End' ? views.length-1 : (views.indexOf(mode) + (e.key === 'ArrowRight' ? 1 : -1) + views.length) % views.length;
        setView(views[index]); (e.currentTarget.parentElement?.children[index] as HTMLElement).focus();
      }} onClick={() => setView(mode)}>{t(names[mode])}</button>)}
    </div><div className="zoom-tools"><button aria-label={t("Diminuir zoom")} onClick={() => setZoom(scale * 80)}>−</button>
      <span>{Math.round(scale * 100)}%</span><button aria-label={t("Aumentar zoom")} onClick={() => setZoom(scale * 125)}>+</button>
      <button onClick={() => { setZoom(null); setPan([0, 0]); }}>{t("Ajustar")}</button></div></div>
    <div className="canvas-tools" role="toolbar" aria-label={t("Ferramentas de edição")}>
      {([['move', t("Mover")], ['erase', t("Borracha")], ['eyedropper', t("Conta-gotas")], ['merge', t("Mesclar regiões")]] as const).map(([value, label]) => <button key={value} aria-pressed={tool === value} disabled={value !== 'move' && !editable} onClick={() => { setTool(value); setSelection(null); setHovered(null); }}>{label}</button>)}
      <span>{tool === 'erase' ? t("Clique: mesclar · Shift: remover") : tool === 'merge' ? selectedRegion === null ? t("Escolha a região de origem") : t("Escolha uma região vizinha") : tool === 'eyedropper' ? t("Selecione uma cor do vetor") : t("Espaço + arraste para mover")}</span>
    </div>
    <div className="region-keyboard"><label>{t('Região')}<select aria-label={t('Selecionar região pelo teclado')} disabled={!editable} value={selectedRegion ?? ''} onChange={e => setSelection(e.target.value === '' || !doc ? null : { doc, region: Number(e.target.value) })}>
      <option value="">—</option>{doc?.shapes.filter(shape => shape.region !== null).map(shape => <option key={shape.region} value={shape.region!}>{shape.region! + 1} · {shape.color}</option>)}
    </select></label><button disabled={!editable || selectedRegion === null} onClick={() => { if (selectedRegion !== null) void edit({ kind: 'merge_region', region: selectedRegion, into: null }); }}>{t('Mesclar região selecionada')}</button>
      <button disabled={!editable || selectedRegion === null} onClick={() => { if (selectedRegion !== null) void edit({ kind: 'remove_region', region: selectedRegion }); }}>{t('Remover região selecionada')}</button></div>
    <div ref={ref} id="comparison-panel" role="tabpanel" aria-label={t(names[view])} tabIndex={0} className="canvas-area" data-tool={tool} onKeyDown={e => {
      if (e.target !== e.currentTarget || !['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)) return;
      e.preventDefault(); const distance = e.shiftKey ? 10 : 40;
      setPan([pan[0] + (e.key === 'ArrowLeft' ? -distance : e.key === 'ArrowRight' ? distance : 0), pan[1] + (e.key === 'ArrowUp' ? -distance : e.key === 'ArrowDown' ? distance : 0)]);
    }} onPointerDown={e => { if (e.button !== 0 && e.button !== 1) return;
      if (e.button === 1 || tool === 'move' || space.current) { e.currentTarget.setPointerCapture(e.pointerId); dragging.current = { x: e.clientX, y: e.clientY, pan }; return; }
      if (!editable || !doc) return; const shape = hit(e); if (shape?.region == null) return;
      if (tool === 'erase') void edit(e.shiftKey ? { kind: 'remove_region', region: shape.region } : { kind: 'merge_region', region: shape.region, into: null });
      if (tool === 'eyedropper') { const color = doc.palette.find(c => `#${c.rgb.map(v => v.toString(16).padStart(2, '0')).join('')}` === shape.color); selectColor(color?.id ?? null); }
      if (tool === 'merge') { if (selectedRegion === null) setSelection({ doc, region: shape.region }); else { if (selectedRegion !== shape.region) void edit({ kind: 'merge_region', region: selectedRegion, into: shape.region }); setSelection(null); } }
    }}
      onPointerMove={e => { const drag = dragging.current; if (drag) setPan([drag.pan[0] + e.clientX - drag.x, drag.pan[1] + e.clientY - drag.y]);
        else if (tool !== 'move' && editable && doc) { const shape = hit(e); setHovered(shape?.region == null ? null : { doc, region: shape.region }); } }}
      onPointerLeave={() => setHovered(null)}
      onPointerUp={() => { dragging.current = null; }} onPointerCancel={() => { dragging.current = null; }}
      onWheel={e => setZoom(scale * 100 * (e.deltaY > 0 ? 0.8 : 1.25))}>
      {view === 'side' ? <><div className="canvas-pane"><div className="pane-label">{t("Original")} <span>{image.w} × {image.h}</span></div><PaintCanvas {...props} mode="original" /></div>
        <div className="canvas-pane"><div className="pane-label">{t("Vetor")} <span>{preview ? t("Prévia · refinando") : t("Fronteiras compartilhadas")}</span></div><PaintCanvas {...props} mode="vector" /></div></>
        : <div className="canvas-pane"><div className="pane-label">{t(names[view])}<span>{preview ? t("Prévia · refinando") : t("Arraste para mover · role para ampliar")}</span></div>
          <PaintCanvas {...props} mode={view} />{view === 'slider' && <div className="slider-divider" style={{ left: `${split}%` }} />}</div>}
    </div>
    {view === 'slider' && <label className="comparison-range">{t("Original")}<input aria-label={t("Comparação entre original e vetor")} type="range" min="0" max="100" value={split} onChange={e => setSplit(Number(e.target.value))} />{t("Vetor")}</label>}
    {view === 'outline' && doc && doc.stats.nodes > 50_000 && <p className="canvas-note">{t("Muitos nós para exibir marcadores. Mostrando apenas os contornos.")}</p>}
  </section>;
}
