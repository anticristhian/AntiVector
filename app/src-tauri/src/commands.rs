//! Thin IPC adapter: decode/encode on blocking workers, jobs and cache in av-session.
use av_core::{Control, CoreError, Params, RgbaImage, VectorDoc};
use av_session::{JobEvent, Session};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Default)]
pub struct DesktopState {
    entries: Mutex<BTreeMap<u64, Entry>>,
    next: AtomicU64,
}
struct Entry {
    session: Arc<Session>,
    path: Option<PathBuf>,
    project: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct UiError {
    code: &'static str,
    msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
}
impl UiError {
    fn new(code: &'static str, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
            width: None,
            height: None,
        }
    }
    fn internal(error: impl std::fmt::Display) -> Self {
        Self::new("E_INTERNAL", error.to_string())
    }
}
impl From<CoreError> for UiError {
    fn from(error: CoreError) -> Self {
        let (code, width, height) = match &error {
            CoreError::ImageTooLarge { w, h, .. } => ("E_TOO_LARGE", Some(*w), Some(*h)),
            CoreError::Cancelled => ("E_CANCELLED", None, None),
            CoreError::InvalidParams(_) | CoreError::EmptyImage => ("E_PARAMS", None, None),
            CoreError::Internal(_) => ("E_INTERNAL", None, None),
        };
        Self {
            code,
            msg: error.to_string(),
            width,
            height,
        }
    }
}
impl From<av_session::decode::DecodeError> for UiError {
    fn from(error: av_session::decode::DecodeError) -> Self {
        match error {
            av_session::decode::DecodeError::Input(error) => error.into(),
            av_session::decode::DecodeError::Io(e) => Self::new("E_DECODE", e.to_string()),
            av_session::decode::DecodeError::Decode(e) => Self::new("E_DECODE", e.to_string()),
        }
    }
}

impl DesktopState {
    fn session(&self, id: u64) -> Result<Arc<Session>, UiError> {
        self.entries
            .lock()
            .map_err(UiError::internal)?
            .get(&id)
            .map(|e| e.session.clone())
            .ok_or_else(|| UiError::new("E_PARAMS", "Sessão não encontrada"))
    }
}

#[derive(Serialize)]
pub struct OpenedImage {
    session_id: u64,
    w: u32,
    h: u32,
    name: String,
    detection: av_core::detect::Detection,
    #[serde(serialize_with = "serialize_ui_params")]
    params: Params,
    thumb_png_b64: String,
    original_png_b64: String,
    restored: Option<DocumentState>,
}

// JavaScript numbers cannot represent every u64 seed. Keep the core/CLI JSON
// contract numeric, and encode only the desktop transport's seed as decimal text.
fn params_json(params: &Params) -> Result<serde_json::Value, UiError> {
    let mut value = serde_json::to_value(params).map_err(UiError::internal)?;
    value["seed"] = params.seed.to_string().into();
    Ok(value)
}
fn serialize_ui_params<S: serde::Serializer>(
    params: &Params,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    params_json(params)
        .map_err(|e| serde::ser::Error::custom(e.msg))?
        .serialize(serializer)
}
fn parse_ui_params(mut value: serde_json::Value) -> Result<Params, UiError> {
    if let Some(seed) = value.get("seed").and_then(|v| v.as_str()) {
        let seed = seed
            .parse::<u64>()
            .map_err(|e| UiError::new("E_PARAMS", e.to_string()))?;
        value["seed"] = seed.into();
    }
    let params: Params =
        serde_json::from_value(value).map_err(|e| UiError::new("E_PARAMS", e.to_string()))?;
    params.validate()?;
    Ok(params)
}

fn register(
    image: RgbaImage,
    name: String,
    path: Option<PathBuf>,
    state: &DesktopState,
) -> Result<OpenedImage, UiError> {
    register_session(Arc::new(Session::new(image)?), name, path, false, state)
}
fn register_session(
    session: Arc<Session>,
    name: String,
    path: Option<PathBuf>,
    project: bool,
    state: &DesktopState,
) -> Result<OpenedImage, UiError> {
    let image = session.source();
    let detection = av_core::detect(&image);
    let restored = session
        .revision()?
        .map(|revision| document_state(&session, &revision))
        .transpose()?;
    let params = session
        .revision()?
        .map(|r| r.params)
        .unwrap_or_else(|| av_core::params::resolve_defaults(detection.mode, &detection));
    let thumb = av_session::jobs::preview_raster(&image, 65_536, &Control::default())?;
    let thumb_png_b64 = STANDARD.encode(av_session::decode::png_bytes(&thumb)?);
    let original_png_b64 = if thumb.w == image.w && thumb.h == image.h {
        thumb_png_b64.clone()
    } else {
        STANDARD.encode(av_session::decode::png_bytes(&image)?)
    };
    let (w, h) = (image.w, image.h);
    let id = state.next.fetch_add(1, Ordering::Relaxed) + 1;
    state.entries.lock().map_err(UiError::internal)?.insert(
        id,
        Entry {
            session,
            path,
            project,
        },
    );
    Ok(OpenedImage {
        session_id: id,
        w,
        h,
        name,
        detection,
        params,
        thumb_png_b64,
        original_png_b64,
        restored,
    })
}

#[tauri::command]
pub async fn open_image(
    path: String,
    reduce_to: Option<u64>,
    state: State<'_, Arc<DesktopState>>,
) -> Result<OpenedImage, UiError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        let limit = if reduce_to.is_some() {
            av_core::raster::DEFAULT_PIXEL_LIMIT
        } else {
            16_000_000
        };
        let image = av_session::decode::load(&path, limit)?;
        let image = if let Some(limit) = reduce_to {
            av_session::jobs::preview_raster(
                &image,
                limit.min(64_000_000) as usize,
                &Control::default(),
            )?
        } else {
            image
        };
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        register(image, name, Some(path), &state)
    })
    .await
    .map_err(UiError::internal)?
}

#[tauri::command]
pub async fn open_image_bytes(
    name: String,
    bytes: Vec<u8>,
    reduce_to: Option<u64>,
    state: State<'_, Arc<DesktopState>>,
) -> Result<OpenedImage, UiError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let limit = if reduce_to.is_some() {
            64_000_000
        } else {
            16_000_000
        };
        let image = av_session::decode::load_bytes(&bytes, limit)?;
        let image = if let Some(limit) = reduce_to {
            av_session::jobs::preview_raster(
                &image,
                limit.min(64_000_000) as usize,
                &Control::default(),
            )?
        } else {
            image
        };
        register(image, name, None, &state)
    })
    .await
    .map_err(UiError::internal)?
}

#[tauri::command]
pub async fn open_clipboard(
    app: AppHandle,
    reduce_to: Option<u64>,
    state: State<'_, Arc<DesktopState>>,
) -> Result<OpenedImage, UiError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let image = app.clipboard().read_image().map_err(|e| {
            UiError::new(
                "E_DECODE",
                format!("Nenhuma imagem disponível na área de transferência: {e}"),
            )
        })?;
        let pixels = image.rgba().as_chunks::<4>().0.to_vec();
        let image = RgbaImage::new(image.width(), image.height(), pixels)?;
        if reduce_to.is_none() && image.px.len() > 16_000_000 {
            return Err(CoreError::ImageTooLarge {
                w: image.w,
                h: image.h,
                limit: 16_000_000,
            }
            .into());
        }
        let image = if let Some(limit) = reduce_to {
            av_session::jobs::preview_raster(
                &image,
                limit.min(64_000_000) as usize,
                &Control::default(),
            )?
        } else {
            image
        };
        register(image, "Imagem colada.png".into(), None, &state)
    })
    .await
    .map_err(UiError::internal)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_seed_transport_preserves_all_u64_values() {
        let params = Params {
            seed: u64::MAX,
            ..Params::default()
        };
        let json = params_json(&params).unwrap();
        assert_eq!(json["seed"], u64::MAX.to_string());
        assert_eq!(parse_ui_params(json).unwrap().seed, u64::MAX);
        let mut invalid = params_json(&params).unwrap();
        invalid["seed"] = "18446744073709551616".into();
        assert_eq!(parse_ui_params(invalid).unwrap_err().code, "E_PARAMS");
    }
    #[test]
    fn decoded_session_and_compact_paths_match_the_svg_writer() {
        let state = DesktopState::default();
        let image = RgbaImage::new(3, 3, vec![[40, 80, 120, 255]; 9]).unwrap();
        let opened = register(image, "fixture.png".into(), None, &state).unwrap();
        assert_eq!((opened.w, opened.h), (3, 3));
        let session = state.session(opened.session_id).unwrap();
        let doc = session
            .vectorize(&opened.params, &Control::default())
            .unwrap();
        let display = frame(&doc).unwrap();
        let svg = av_export::svg::to_string(&doc).unwrap();
        assert_eq!(display.shapes.len(), 1);
        assert!(svg.contains(&format!("d=\"{}\"", display.shapes[0].d)));
        assert!(display.shapes[0].nodes.len() <= 50_000);
        let decoded = av_session::decode::load_bytes(
            &STANDARD.decode(&opened.original_png_b64).unwrap(),
            64_000_000,
        )
        .unwrap();
        assert_eq!(*session.source(), decoded);
        let json = serde_json::to_value(&display).unwrap();
        assert_eq!(json["stats"]["colors"], 1);
    }
    #[test]
    fn large_image_errors_include_dimensions_for_the_reduction_choice() {
        let error: UiError = CoreError::ImageTooLarge {
            w: 20_000,
            h: 10_000,
            limit: 16_000_000,
        }
        .into();
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json["code"], "E_TOO_LARGE");
        assert_eq!(json["width"], 20_000);
        assert_eq!(json["height"], 10_000);
    }
}

/// Optional, local-only QA recording; absent environment setting means no IO.
#[tauri::command]
pub fn record_ui_metrics(mut report: serde_json::Value) -> Result<(), UiError> {
    let Some(path) = std::env::var_os("AV_UI_METRICS_PATH") else {
        return Ok(());
    };
    report["rust_profile"] = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
    .into();
    let mut bytes = serde_json::to_vec(&report).map_err(UiError::internal)?;
    if bytes.len() > 8192 {
        return Err(UiError::new("E_PARAMS", "Relatório muito grande"));
    }
    bytes.push(b'\n');
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|e| UiError::new("E_EXPORT_IO", e.to_string()))
}

#[derive(Clone, Serialize)]
pub struct Frame {
    width: f64,
    height: f64,
    shapes: Vec<Shape>,
    palette: Vec<av_core::color::PaletteColor>,
    stats: Stats,
}
#[derive(Clone, Serialize)]
struct Shape {
    region: Option<u32>,
    color: String,
    d: String,
    nodes: Vec<Anchor>,
}
#[derive(Clone, Serialize)]
struct Anchor {
    point: [f64; 2],
    corner: bool,
}
#[derive(Clone, Serialize)]
struct Stats {
    paths: usize,
    nodes: usize,
    colors: usize,
}

fn frame(doc: &VectorDoc) -> Result<Frame, UiError> {
    let options = av_export::svg::SvgOptions::default();
    let paths = av_export::svg::to_paths(doc, &options)?;
    let shapes = paths
        .into_iter()
        .map(|p| Shape {
            region: p.region,
            color: format!("#{:02x}{:02x}{:02x}", p.color[0], p.color[1], p.color[2]),
            d: p.data,
            nodes: p
                .anchors
                .into_iter()
                .map(|(point, corner)| Anchor { point, corner })
                .collect(),
        })
        .collect();
    let stats = av_export::statistics(doc, av_export::Format::Svg, &options)?;
    Ok(Frame {
        width: doc.width,
        height: doc.height,
        shapes,
        palette: doc.palette.colors.clone(),
        stats: Stats {
            paths: stats.paths,
            nodes: stats.nodes,
            colors: stats.colors,
        },
    })
}

#[tauri::command]
pub fn vectorize(
    session_id: u64,
    params: serde_json::Value,
    request_id: u64,
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<u64, UiError> {
    let session = state.session(session_id)?;
    session
        .start(
            parse_ui_params(params)?,
            notifications(session.clone(), session_id, request_id, app),
        )
        .map_err(Into::into)
}
fn notifications(
    session: Arc<Session>,
    session_id: u64,
    request_id: u64,
    app: AppHandle,
) -> Arc<dyn Fn(JobEvent) + Send + Sync> {
    Arc::new(move |event| {
        let (name, payload) = match event {
            JobEvent::Progress {
                job_id,
                stage,
                fraction,
            } => (
                "progress",
                serde_json::json!({"job_id":job_id,"stage":stage,"frac":fraction}),
            ),
            JobEvent::Preview { job_id, doc } => match frame(&doc) {
                Ok(frame) => ("preview", serde_json::json!({"job_id":job_id,"doc":frame})),
                Err(error) => ("error", serde_json::json!({"job_id":job_id,"error":error})),
            },
            JobEvent::Done { job_id, doc } => match completed_state(&session, &doc) {
                Ok(result) => (
                    "done",
                    serde_json::json!({"job_id":job_id,"doc":result.doc,"params":params_json(&result.params).ok(),"history":result.history}),
                ),
                Err(error) => ("error", serde_json::json!({"job_id":job_id,"error":error})),
            },
            JobEvent::Error { job_id, error } => (
                "error",
                serde_json::json!({"job_id":job_id,"error":UiError::from(error)}),
            ),
        };
        let mut payload = payload;
        payload["session_id"] = session_id.into();
        payload["request_id"] = request_id.into();
        let _ = app.emit(name, payload);
    })
}

#[derive(Serialize)]
pub struct DocumentState {
    doc: Frame,
    #[serde(serialize_with = "serialize_ui_params")]
    params: Params,
    history: av_session::editor::HistoryStatus,
}
fn document_state(
    session: &Session,
    revision: &av_session::editor::Revision,
) -> Result<DocumentState, UiError> {
    Ok(DocumentState {
        doc: frame(&revision.doc)?,
        params: revision.params.clone(),
        history: session.history()?,
    })
}
fn completed_state(session: &Session, doc: &VectorDoc) -> Result<DocumentState, UiError> {
    let revision = session
        .revision()?
        .ok_or_else(|| UiError::new("E_INTERNAL", "Revisão concluída ausente"))?;
    Ok(DocumentState {
        doc: frame(doc)?,
        params: revision.params,
        history: session.history()?,
    })
}

#[derive(Serialize)]
pub struct FidelityEstimate {
    ssim: f64,
    width: u32,
    height: u32,
    estimated: bool,
}
#[tauri::command]
pub async fn estimate_fidelity(
    session_id: u64,
    state: State<'_, Arc<DesktopState>>,
) -> Result<FidelityEstimate, UiError> {
    let session = state.session(session_id)?;
    let doc = session
        .latest()?
        .ok_or_else(|| UiError::new("E_PARAMS", "Nenhum vetor concluído"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let original =
            av_session::jobs::preview_raster(&session.source(), 65_536, &Control::default())?;
        let svg = av_export::svg::to_string(&doc)?
            .replacen(
                &format!("width=\"{}\"", doc.width),
                &format!("width=\"{}\"", original.w),
                1,
            )
            .replacen(
                &format!("height=\"{}\"", doc.height),
                &format!("height=\"{}\"", original.h),
                1,
            );
        let rendered = av_metrics::render::svg(&svg, true)?;
        Ok(FidelityEstimate {
            ssim: av_metrics::ssim::ssim(&original, &rendered)?,
            width: original.w,
            height: original.h,
            estimated: true,
        })
    })
    .await
    .map_err(UiError::internal)?
}
#[tauri::command]
pub fn edit(
    session_id: u64,
    op: av_core::edit::EditOp,
    request_id: u64,
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<u64, UiError> {
    let session = state.session(session_id)?;
    session
        .start_edit(
            op,
            notifications(session.clone(), session_id, request_id, app),
        )
        .map_err(Into::into)
}
#[tauri::command]
pub async fn undo(
    session_id: u64,
    state: State<'_, Arc<DesktopState>>,
) -> Result<DocumentState, UiError> {
    let session = state.session(session_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let revision = session.undo()?;
        document_state(&session, &revision)
    })
    .await
    .map_err(UiError::internal)?
}
#[tauri::command]
pub async fn redo(
    session_id: u64,
    state: State<'_, Arc<DesktopState>>,
) -> Result<DocumentState, UiError> {
    let session = state.session(session_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let revision = session.redo()?;
        document_state(&session, &revision)
    })
    .await
    .map_err(UiError::internal)?
}
#[tauri::command]
pub async fn current_document(
    session_id: u64,
    state: State<'_, Arc<DesktopState>>,
) -> Result<DocumentState, UiError> {
    let session = state.session(session_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let revision = session
            .revision()?
            .ok_or_else(|| UiError::new("E_PARAMS", "Nenhum vetor concluído"))?;
        document_state(&session, &revision)
    })
    .await
    .map_err(UiError::internal)?
}
#[tauri::command]
pub async fn save_project(
    session_id: u64,
    path: String,
    name: String,
    state: State<'_, Arc<DesktopState>>,
) -> Result<(), UiError> {
    let session = state.session(session_id)?;
    let source = state
        .entries
        .lock()
        .map_err(UiError::internal)?
        .get(&session_id)
        .filter(|e| !e.project)
        .and_then(|e| e.path.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        if let Some(source) = source {
            if let (Ok(a), Ok(b)) = (source.canonicalize(), path.canonicalize()) {
                if a == b {
                    return Err(UiError::new(
                        "E_PARAMS",
                        "O projeto deve ser diferente da imagem original",
                    ));
                }
            }
        }
        let revision = session
            .revision()?
            .ok_or_else(|| UiError::new("E_PARAMS", "Nenhum vetor concluído"))?;
        av_session::project::save(&path, &name, &session.source(), &revision)
            .map_err(|e| UiError::new("E_EXPORT_IO", e.to_string()))
    })
    .await
    .map_err(UiError::internal)?
}
#[tauri::command]
pub async fn open_project(
    path: String,
    state: State<'_, Arc<DesktopState>>,
) -> Result<OpenedImage, UiError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        let project = av_session::project::open(&path)
            .map_err(|e| UiError::new("E_DECODE", e.to_string()))?;
        let session = Arc::new(Session::new(project.image)?);
        session.restore(&project.params, project.edits, &Control::default())?;
        register_session(session, project.name, Some(path), true, &state)
    })
    .await
    .map_err(UiError::internal)?
}
#[tauri::command]
pub async fn open_project_bytes(
    bytes: Vec<u8>,
    state: State<'_, Arc<DesktopState>>,
) -> Result<OpenedImage, UiError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = av_session::project::open_bytes(&bytes)
            .map_err(|e| UiError::new("E_DECODE", e.to_string()))?;
        let session = Arc::new(Session::new(project.image)?);
        session.restore(&project.params, project.edits, &Control::default())?;
        register_session(session, project.name, None, true, &state)
    })
    .await
    .map_err(UiError::internal)?
}

#[tauri::command]
pub fn cancel(session_id: u64, state: State<'_, Arc<DesktopState>>) -> Result<(), UiError> {
    state.session(session_id)?.cancel().map_err(Into::into)
}

#[tauri::command]
pub fn close_session(session_id: u64, state: State<'_, Arc<DesktopState>>) -> Result<(), UiError> {
    let entry = state
        .entries
        .lock()
        .map_err(UiError::internal)?
        .remove(&session_id);
    if let Some(entry) = entry {
        entry.session.cancel()?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct ExportOptions {
    format: String,
    precision: u8,
    overlap: bool,
    group_by_color: bool,
}
#[tauri::command]
pub async fn export(
    session_id: u64,
    path: String,
    options: ExportOptions,
    state: State<'_, Arc<DesktopState>>,
) -> Result<(), UiError> {
    let session = state.session(session_id)?;
    let source = state
        .entries
        .lock()
        .map_err(UiError::internal)?
        .get(&session_id)
        .and_then(|e| e.path.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        if let Some(source) = source {
            if let (Ok(a), Ok(b)) = (source.canonicalize(), path.canonicalize()) {
                if a == b {
                    return Err(UiError::new(
                        "E_PARAMS",
                        "A saída deve ser diferente da imagem original",
                    ));
                }
            }
        }
        let doc = session
            .latest()?
            .ok_or_else(|| UiError::new("E_PARAMS", "Nenhum vetor concluído"))?;
        let bytes = av_export::export(
            &doc,
            av_export::Format::parse(&options.format)?,
            &av_export::svg::SvgOptions {
                precision: options.precision,
                overlap: options.overlap,
                group_by_color: options.group_by_color,
            },
        )?;
        std::fs::write(path, bytes).map_err(|e| UiError::new("E_EXPORT_IO", e.to_string()))
    })
    .await
    .map_err(UiError::internal)?
}
