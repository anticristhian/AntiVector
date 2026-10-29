import { t } from '../i18n';
import { useState } from 'react';
import { useAppStore } from '../state/store';
import type { ExportOptions } from '../ipc/types';
export function ExportDialog({ onClose }: { onClose: () => void }) {
  const exportImage = useAppStore(state => state.export);
  const [options, setOptions] = useState<ExportOptions>({ format: 'svg', precision: 2, overlap: true, group_by_color: false });
  return <div className="modal-backdrop" onClick={onClose}><section className="modal" role="dialog" aria-modal="true" aria-labelledby="export-title" onClick={e => e.stopPropagation()}>
    <div className="modal-heading"><h2 id="export-title">{t('Exportar vetor')}</h2><button aria-label={t("Fechar exportação")} onClick={onClose}>×</button></div>
    <label className="field-label">{t("Formato")}<select aria-label={t("Formato de exportação")} value={options.format} onChange={e => setOptions({ ...options, format: e.target.value as ExportOptions['format'] })}>
      <option value="svg">{t("SVG · Editável")}</option><option value="pdf">{t("PDF · Vetorial")}</option><option value="eps">{t("EPS · PostScript")}</option><option value="dxf">{t("DXF · Contornos R12")}</option><option value="png">{t("PNG · Imagem com alfa")}</option></select></label>
    {(options.format === 'svg' || options.format === 'png') && <label className="field-label">{t("Casas decimais")}<input aria-label={t("Precisão decimal")} type="number" min={0} max={8} value={options.precision} onChange={e => setOptions({ ...options, precision: Math.min(8, Math.max(0, Number(e.target.value))) })} /></label>}
    {options.format !== 'dxf' && <><label className="check-label"><input type="checkbox" checked={options.overlap} disabled={options.group_by_color} onChange={e => setOptions({ ...options, overlap: e.target.checked })} />{t("Sobreposição opaca segura")}</label>
      {(options.format === 'svg' || options.format === 'png') && <label className="check-label"><input type="checkbox" checked={options.group_by_color} onChange={e => setOptions({ ...options, group_by_color: e.target.checked, overlap: e.target.checked ? false : options.overlap })} />{t("Agrupar caminhos por cor")}</label>}</>}
    <p className="field-hint">{options.format === 'dxf' ? t("Somente contornos, uma fronteira por entidade. Precisão de 0,1 px.") : options.format === 'png' ? t("Mesma resolução da imagem de entrada, preservando transparência.") : t("Caminhos editáveis com dimensões da imagem original.")}</p>
    <div className="modal-actions"><button onClick={onClose}>{t("Cancelar")}</button><button className="primary" onClick={() => { onClose(); void exportImage(options); }}>{t("Escolher destino…")}</button></div>
  </section></div>;
}
