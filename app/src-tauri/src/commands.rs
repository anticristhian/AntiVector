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
    params: Params,
    thumb_png_b64: String,
    original_png_b64: String,
}

fn register(
    image: RgbaImage,
    name: String,
    path: Option<PathBuf>,
    state: &DesktopState,
) -> Result<OpenedImage, UiError> {
    let detection = av_core::detect(&image);
    let params = av_core::params::resolve_defaults(detection.mode, &detection);
    let thumb = av_session::jobs::preview_raster(&image, 65_536, &Control::default())?;
    let thumb_png_b64 = STANDARD.encode(av_session::decode::png_bytes(&thumb)?);
    let original_png_b64 = if thumb.w == image.w && thumb.h == image.h {
        thumb_png_b64.clone()
    } else {
        STANDARD.encode(av_session::decode::png_bytes(&image)?)
    };
    let (w, h) = (image.w, image.h);
    let session = Arc::new(Session::new(image)?);
    let id = state.next.fetch_add(1, Ordering::Relaxed) + 1;
    state
        .entries
        .lock()
        .map_err(UiError::internal)?
        .insert(id, Entry { session, path });
    Ok(OpenedImage {
        session_id: id,
        w,
        h,
        name,
        detection,
        params,
        thumb_png_b64,
        original_png_b64,
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
    params: Params,
    request_id: u64,
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<u64, UiError> {
    let session = state.session(session_id)?;
    session
        .start(
            params,
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
                    JobEvent::Done { job_id, doc } => match frame(&doc) {
                        Ok(frame) => ("done", serde_json::json!({"job_id":job_id,"doc":frame})),
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
            }),
        )
        .map_err(Into::into)
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
