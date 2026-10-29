import { t, locale } from '../i18n';
import { useState } from 'react';
import { useAppStore } from '../state/store';
import type { VectorFrame } from '../ipc/types';
type Color = VectorFrame['palette'][number];
const hex = (rgb: number[]) => `#${rgb.map(v => v.toString(16).padStart(2, '0')).join('')}`;
function ColorControls({ color, colors, busy }: { color: Color; colors: Color[]; busy: boolean }) {
  const edit = useAppStore(s => s.edit);
  const [draft, setDraft] = useState(hex(color.rgb));
  const valid = /^#[0-9a-f]{6}$/i.test(draft);
  const targets = colors.filter(c => c.id !== color.id);
  const [target, setTarget] = useState(String(targets[0]?.id ?? ''));
  return <div className="palette-editor">
    <label className="field-label">{t("Nova cor")}<input aria-label={t("Nova cor")} type="color" value={valid ? draft : hex(color.rgb)} disabled={busy} onChange={e => setDraft(e.target.value)} /></label>
    <label className="field-label">{t("RGB hexadecimal")}<input aria-label={t("Cor hexadecimal")} type="text" value={draft} maxLength={7} pattern="#[0-9a-fA-F]{6}" disabled={busy} onChange={e => setDraft(e.target.value)} /></label>
    <button disabled={busy || !valid || draft.toLowerCase() === hex(color.rgb)} onClick={() => { void edit({ kind: 'recolor_palette', color: color.id, rgb: [1, 3, 5].map(i => parseInt(draft.slice(i, i + 2), 16)) }); }}>{t("Aplicar cor")}</button>
    <label className="check-label"><input type="checkbox" aria-label={t("Travar cor")} checked={color.locked} disabled={busy} onChange={e => { void edit({ kind: 'lock_color', color: color.id, locked: e.target.checked }); }} />{t("Travar na quantização")}</label>
    <label className="field-label">{t("Mesclar em")}<select aria-label={t("Cor de destino")} value={target} disabled={busy || !targets.length} onChange={e => setTarget(e.target.value)}>{targets.map(c => <option key={c.id} value={c.id}>{hex(c.rgb).toUpperCase()}</option>)}</select></label>
    <div className="palette-actions"><button disabled={busy || !targets.length} onClick={() => { void edit({ kind: 'merge_palette', from: color.id, into: Number(target) }); }}>{t("Mesclar")}</button>
      <button disabled={busy} onClick={() => { void edit({ kind: 'delete_palette', color: color.id }); }}>{t("Excluir cor")}</button></div>
  </div>;
}
export function PalettePanel() {
  const { doc, image, phase, selectedColor, selectColor, edit, fidelity, estimating, estimate } = useAppStore();
  const busy = ['loading', 'vectorizing', 'exporting', 'saving'].includes(phase);
  const selected = doc?.palette.find(c => c.id === selectedColor);
  return <aside className="palette-panel"><div className="panel-heading"><h2>{t("Paleta")}</h2><span className="pill">{doc?.palette.length ?? '—'}</span></div>
    {doc ? <><div className="swatch-list">{doc.palette.map(color => <div className={`swatch-row ${selectedColor === color.id ? 'selected' : ''}`} key={color.id}
      onDragOver={e => { if (e.dataTransfer.types.includes('application/x-antivector-color')) e.preventDefault(); }}
      onDrop={e => { e.preventDefault(); const from = Number(e.dataTransfer.getData('application/x-antivector-color')); if (!busy && Number.isInteger(from) && from !== color.id && doc.palette.some(c => c.id === from)) void edit({ kind: 'merge_palette', from, into: color.id }); }}>
      <button className="swatch" style={{ background: hex(color.rgb) }} aria-label={`${t("Selecionar cor")} ${hex(color.rgb).toUpperCase()}`} aria-pressed={selectedColor === color.id}
        draggable={!busy} onDragStart={e => e.dataTransfer.setData('application/x-antivector-color', String(color.id))} onClick={() => selectColor(color.id)} />
      <span>{hex(color.rgb).toUpperCase()}</span>{color.locked && <span title={t("Cor travada")} aria-label={t("Cor travada")}>◈</span>}</div>)}</div>
      <p className="field-hint">{t("Selecione para editar. Arraste uma cor sobre outra para mesclar.")}</p>
      {selected && <ColorControls key={`${image?.session_id}:${selected.id}:${hex(selected.rgb)}`} color={selected} colors={doc.palette} busy={busy} />}</> : <p className="field-hint">{t("As cores aparecem quando o vetor estiver pronto.")}</p>}
    <div className="statistics"><span className="eyebrow">{t("ESTATÍSTICAS")}</span><dl>{(['paths', 'nodes', 'colors'] as const).map((key, index) => <div key={key}><dt>{[t("Caminhos"), t("Nós"), t("Cores")][index]}</dt><dd>{doc?.stats[key].toLocaleString(locale()) ?? '—'}</dd></div>)}
      <div><dt>{t("SSIM estimado")}</dt><dd>{fidelity ? fidelity.ssim.toFixed(4) : '—'}</dd></div></dl>
      <button disabled={!doc || busy || estimating} onClick={() => { void estimate(); }}>{estimating ? t("Calculando…") : t("Estimar fidelidade")}</button>
      {fidelity && <p className="field-hint">{t("Amostra")} {fidelity.width} × {fidelity.height} {t("px, composta sobre branco.")}</p>}</div>
    <div className="source-info"><span className="eyebrow">{t("IMAGEM ORIGINAL")}</span><p>{image?.name}</p><span>{image?.w.toLocaleString(locale())} × {image?.h.toLocaleString(locale())} px</span><p className="field-hint">{image?.detection.has_alpha ? t("Com transparência") : t("Imagem opaca")}<br />{t("Contornos preservados entre regiões.")}</p></div>
  </aside>;
}
