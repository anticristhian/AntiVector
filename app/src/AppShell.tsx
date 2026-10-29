import { useEffect, useRef, useState } from 'react';
import { CanvasStage } from './components/CanvasStage';
import { Controls } from './components/Controls';
import { ExportDialog } from './components/ExportDialog';
import { PalettePanel } from './components/PalettePanel';
import { useAppStore } from './state/store';
import { bridge } from './ipc/bridge';
import type { View } from './ipc/types';
import './App.css';
import { useUiPerformance } from './state/performance';

const stages: Record<string, string> = { detect: 'Analisando imagem', preprocess: 'Preparando imagem', quantize: 'Selecionando cores', resolve_aa: 'Resolvendo anti-aliasing', label: 'Identificando regiões', merge_small: 'Preservando detalhes', extract_boundaries: 'Extraindo fronteiras', fit_edges: 'Ajustando curvas', assemble: 'Montando vetor' };
function StatusLine() {
  const { phase, stage, fraction, cancel, startedAt, request, preview } = useAppStore();
  const [time, setTime] = useState({ request: -1, value: 0 });
  useEffect(() => { if (phase !== 'vectorizing') return;
    const id = setInterval(() => setTime({ request, value: (performance.now() - startedAt) / 1000 }), 200); return () => clearInterval(id);
  }, [phase, request, startedAt]);
  const busy = ['vectorizing', 'loading', 'exporting', 'saving'].includes(phase);
  const text = phase === 'vectorizing' ? (stage ? stages[stage] : 'Preparando vetorização') : phase === 'loading' ? 'Abrindo imagem ou projeto' : phase === 'saving' ? 'Salvando projeto' : phase === 'exporting' ? 'Exportando' : phase === 'cancelled' ? 'Cancelado · resultado anterior mantido' : phase === 'ready' ? 'Vetor pronto para exportar' : phase === 'error' ? 'Verifique a mensagem acima' : 'Pronto para começar';
  return <footer className="status-bar"><span className={busy ? 'spinner' : 'status-dot'} /><span role="status" aria-live="polite">{text}{preview ? ' · prévia disponível' : ''}</span>
    {phase === 'vectorizing' && <><progress aria-label="Progresso da vetorização" max={9} value={(stage ? Object.keys(stages).indexOf(stage) : 0) + fraction} /><span className="elapsed">{(time.request === request ? time.value : 0).toFixed(1)} s</span><button onClick={cancel}>Cancelar <kbd>Esc</kbd></button></>}
    <span className="status-right">OFFLINE · RUST</span></footer>;
}
export default function AppShell() {
  useUiPerformance();
  const state = useAppStore(); const [exportOpen, setExportOpen] = useState(false); const [dragging, setDragging] = useState(false); const dragCount = useRef(0);
  const canExport = !!state.completed && !['vectorizing', 'loading', 'exporting', 'saving'].includes(state.phase);
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => { const current = useAppStore.getState();
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'o') { event.preventDefault(); void current.openPicker(); return; }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void current.saveProject(); return; }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z' && !(event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement && ['text','number','search','email','url','password','tel'].includes(event.target.type))) { event.preventDefault(); void current.travel(event.shiftKey ? 'redo' : 'undo'); return; }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'e') { event.preventDefault(); if (current.completed && !['vectorizing','loading','exporting','saving'].includes(current.phase)) setExportOpen(true); return; }
      if (event.key === 'Escape') { if (exportOpen) setExportOpen(false); else if (current.large) current.dismiss(); else if (current.phase === 'vectorizing') current.cancel(); return; }
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target instanceof HTMLTextAreaElement) return;
      const views: Record<string, View> = { '1': 'side', '2': 'slider', '3': 'vector', '4': 'outline', '5': 'difference' }; if (views[event.key]) current.setView(views[event.key]);
      if (event.key.toLowerCase() === 'e' && !event.ctrlKey && !event.metaKey) current.setTool(current.tool === 'erase' ? 'move' : 'erase');
      if (event.key === '+' || event.key === '=') current.setZoom((current.zoom ?? 100) * 1.25); if (event.key === '-') current.setZoom((current.zoom ?? 100) * 0.8); if (event.key === '0') { current.setZoom(null); current.setPan([0, 0]); }
    };
    const paste = (event: ClipboardEvent) => { const file = event.clipboardData?.files[0]; if (file) { event.preventDefault(); void useAppStore.getState().openFile(file); } };
    window.addEventListener('keydown', keyboard); window.addEventListener('paste', paste); return () => { window.removeEventListener('keydown', keyboard); window.removeEventListener('paste', paste); };
  }, [exportOpen]);
  return <main className="application" onDragEnter={event => { if (event.dataTransfer.types.includes('Files')) { event.preventDefault(); dragCount.current++; setDragging(true); } }} onDragOver={event => { if (event.dataTransfer.types.includes('Files')) event.preventDefault(); }} onDragLeave={event => { event.preventDefault(); if (--dragCount.current <= 0) setDragging(false); }} onDrop={event => { event.preventDefault(); dragCount.current = 0; setDragging(false); const file = event.dataTransfer.files[0]; if (file) void state.openFile(file); }}>
    <header className="topbar"><div className="brand"><span className="brand-mark">AV</span><div>AntiVector<span>RASTER → VETOR</span></div></div><div className="toolbar"><button onClick={() => { void state.openPicker(); }}><span>＋</span> Abrir imagem <kbd>Ctrl O</kbd></button><button onClick={() => { void state.paste(); }}>Colar</button>{state.image && <span className="file-tab" title={state.image.name}>{state.image.name}<button aria-label="Fechar imagem" onClick={state.close}>×</button></span>}</div><button className="primary" disabled={!canExport} onClick={() => setExportOpen(true)}>Exportar <span>↗</span></button></header>
    {state.error && <div className="error-banner" role="alert"><div><strong>{state.large ? 'Imagem grande' : 'Não foi possível concluir'}</strong><span>{state.error.msg}</span></div><button onClick={() => { void state.openPicker(); }}>Abrir outra imagem</button><button aria-label="Fechar mensagem" onClick={state.dismiss}>×</button></div>}
    {state.exported && <div className="success-banner" role="status">{state.notice ?? 'Arquivo exportado.'}<button onClick={() => { void bridge.reveal(state.exported!); }}>Abrir pasta</button><button aria-label="Fechar aviso" onClick={state.dismiss}>×</button></div>}
    {state.image && <div className="history-toolbar" role="toolbar" aria-label="Projeto e histórico"><button disabled={!canExport} onClick={() => { void state.saveProject(); }}>Salvar projeto <kbd>Ctrl S</kbd></button><button disabled={!state.completed || ['loading','exporting','saving'].includes(state.phase) || state.history.undo_depth === 0 && !['vectorizing','cancelled'].includes(state.phase)} onClick={() => { void state.travel('undo'); }}>Desfazer <kbd>Ctrl Z</kbd></button><button disabled={!canExport || state.history.redo_depth === 0} onClick={() => { void state.travel('redo'); }}>Refazer <kbd>Ctrl Shift Z</kbd></button><span>{state.history.undo_depth} alterações no histórico</span></div>}
    {state.image ? <div className="work-layout"><Controls key={`${state.image.session_id}:${state.params?.mode}:${state.params?.experimental.force_pixel_art}`} /><CanvasStage /><PalettePanel /></div> : <section className="welcome"><div className="welcome-card"><span className="eyebrow">PRECISÃO EM CADA CONTORNO</span><h1>Da imagem ao vetor.<br /><em>Sem perder o detalhe.</em></h1><p>Transforme logos e ilustrações em caminhos editáveis.<br />Cores fiéis, curvas suaves e fronteiras compartilhadas.</p>
      <button className="drop-zone" onClick={() => { void state.openPicker(); }}><svg viewBox="0 0 64 64" aria-hidden="true"><path d="M12 50 32 12l20 38H12ZM25 37h14" /><circle cx="32" cy="12" r="3" /><rect x="9" y="47" width="6" height="6" /><rect x="49" y="47" width="6" height="6" /></svg><strong>Solte sua imagem aqui</strong><span>ou clique para abrir um arquivo</span><small>PNG · JPEG · BMP · GIF · TIFF · WebP</small></button><p className="local-note"><span className="status-dot" />Sem uploads. Todo o processamento acontece no seu computador.</p></div>
      {state.recents.length > 0 && <div className="recent-list"><h2>Recentes</h2><div>{state.recents.map(recent => <button key={recent.path} onClick={() => { void state.openPath(recent.path); }}><img src={`data:image/png;base64,${recent.thumbnail}`} alt="" /><span>{recent.name}</span></button>)}</div></div>}</section>}
    <StatusLine />{dragging && <div className="drop-overlay">Solte para abrir a imagem</div>}{exportOpen && <ExportDialog onClose={() => setExportOpen(false)} />}
    {(['loading','exporting','saving'].includes(state.phase)) && <div className="modal-backdrop"><section className="modal" role="dialog" aria-modal="true" aria-labelledby="busy-title" aria-busy="true"><h2 id="busy-title">{state.phase === 'loading' ? 'Abrindo imagem ou projeto' : state.phase === 'saving' ? 'Salvando projeto' : 'Exportando vetor'}</h2><p>{state.phase === 'loading' ? 'Decodificando a imagem e preparando a comparação.' : 'Gravando o arquivo no destino escolhido.'}</p><progress aria-label={state.phase === 'loading' ? 'Carregamento da imagem' : 'Gravação do vetor'} /></section></div>}
    {state.large && <div className="modal-backdrop"><section role="dialog" aria-modal="true" aria-labelledby="large-title" className="modal"><h2 id="large-title">Imagem com mais de 16 MP</h2><p>{state.large.width.toLocaleString('pt-BR')} × {state.large.height.toLocaleString('pt-BR')} pixels. Você pode reduzir antes de vetorizar.</p><div className="modal-actions"><button onClick={state.dismiss}>Cancelar</button><button onClick={() => { void state.large!.retry(64_000_000); }}>Manter original</button><button className="primary" onClick={() => { void state.large!.retry(4_000_000); }}>Reduzir para 4 MP</button></div></section></div>}
  </main>;
}
