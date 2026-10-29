import { useState } from 'react';
import { useAppStore } from '../state/store';
import type { Mode } from '../ipc/types';

function Range({ label, value, min = 0, max = 1, step = 0.01, onChange }:
  { label: string; value: number; min?: number; max?: number; step?: number; onChange: (value: number) => void }) {
  return <label className="parameter"><span>{label}<output>{max === 1 ? `${Math.round(value * 100)}%` : Math.round(value)}</output></span>
    <input type="range" aria-label={label} min={min} max={max} step={step} value={value} onChange={e => onChange(Number(e.target.value))} /></label>;
}

export function Controls() {
  const { image, params, update, start, phase } = useAppStore();
  const [choice,setChoice]=useState(() => params?.experimental.force_pixel_art ? 'pixelart' : params?.mode === image?.detection.mode ? 'auto' : params?.mode ?? 'auto');
  if (!params || !image) return null;
  function changeMode(value: string) {
    if (!params || !image) return;
    setChoice(value);
    const selected = value === 'auto' ? image.detection.mode : value === 'pixelart' ? 'logo' : value as Mode;
    const pixel = value === 'pixelart' || value === 'auto' && image.detection.is_pixel_art;
    update({ mode: selected, denoise: pixel ? 0 : selected === 'photo' ? 0.6 : selected === 'illustration' ? 0.3 : image.detection.is_jpeg_like ? 0.5 : 0.2,
      colors: 'auto', experimental: { ...params.experimental, force_pixel_art: pixel },
      fit_tolerance_px: (0.3 + 1.2 * params.smoothness) * (selected === 'photo' ? 1.5 : 1) });
  }
  return <aside className="control-panel"><div className="panel-heading"><h2>Vetorizar</h2><span className="eyebrow">CONTROLES</span></div>
    <label className="field-label">Modo<select aria-label="Modo" value={choice} onChange={e => changeMode(e.target.value)}>
      <option value="auto">Automático</option><option value="logo">Logo / Arte</option><option value="illustration">Ilustração</option>
      <option value="photo">Foto estilizada</option><option value="line_art">Preto e branco / Linha</option><option value="pixelart">Pixel art</option></select></label>
    <p className="field-hint">Sugestão: {image.detection.colors} cores{image.detection.is_jpeg_like ? ' · ruído JPEG' : ''}</p>
    <div className="section-label"><h3>Cores</h3><label className="check-label"><input type="checkbox" checked={params.colors === 'auto'} onChange={e => update({ colors: e.target.checked ? 'auto' : { fixed: Math.min(64, Math.max(2, image.detection.colors)) } })} />Auto</label></div>
    {params.colors !== 'auto' && <Range label="Número de cores" value={params.colors.fixed} min={2} max={64} step={1} onChange={value => update({ colors: { fixed: value } })} />}
    <div className="parameter-section"><Range label="Detalhe" value={params.detail} onChange={detail => update({ detail, min_region_area: Math.max(2, Math.round((64 - 62 * detail) * Math.sqrt(image.w * image.h / 1e6))) })} />
      <p className="field-hint">Preserve traços finos e pequenos elementos.</p>
      <Range label="Suavidade" value={params.smoothness} onChange={smoothness => update({ smoothness, fit_tolerance_px: (0.3 + 1.2 * smoothness) * (params.mode === 'photo' ? 1.5 : 1) })} />
      <p className="field-hint">Equilibre curvas suaves e fidelidade.</p>
      <Range label="Cantos" value={params.corner_threshold_deg} min={30} max={170} step={1} onChange={corner_threshold_deg => update({ corner_threshold_deg })} />
    </div>
    <details className="advanced"><summary>Avançado</summary><Range label="Redução de ruído" value={params.denoise} onChange={denoise => update({ denoise })} />
      <Range label="Tolerância" value={params.fit_tolerance_px} min={0.1} max={3} step={0.05} onChange={fit_tolerance_px => update({ fit_tolerance_px })} />
      <label className="check-label"><input type="checkbox" checked={params.antialias_resolve} onChange={e => update({ antialias_resolve: e.target.checked })} />Remover cores de anti-aliasing</label>
    </details>
    <button className="primary full-width" onClick={start} disabled={phase === 'loading' || phase === 'exporting' || phase === 'saving'}>Vetorizar</button>
    <p className="local-note"><span className="status-dot" />Processamento local. Sua imagem fica aqui.</p>
  </aside>;
}
