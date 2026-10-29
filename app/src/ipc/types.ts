export type Mode = 'logo' | 'illustration' | 'photo' | 'line_art';
export type Params = {
  version: number; mode: Mode; colors: 'auto' | { fixed: number }; palette_lock: number[][];
  denoise: number; min_region_area: number; antialias_resolve: boolean;
  corner_threshold_deg: number; smoothness: number; fit_tolerance_px: number; detail: number;
  transparency_alpha_cut: number; seed: string | number;
  experimental: { opaque_canvas_overlap?: boolean; quality_v5: boolean; denoise_kind: 'bilateral' | 'mean_shift' | 'guided';
    force_pixel_art: boolean; pixel_art_strategy: 'preserve_grid' | 'scale_corners';
    upscale_small: boolean; curve_fit: boolean; corner_recovery_radius: number; quantizer: string };
};
export type Detection = { mode: Mode; colors: number; has_alpha: boolean; is_jpeg_like: boolean; is_pixel_art: boolean };
export type OpenedImage = { session_id: number; w: number; h: number; name: string; detection: Detection;
  params: Params; thumb_png_b64: string; original_png_b64: string; restored?: DocumentState | null };
export type History = { undo_depth: number; redo_depth: number };
export type DocumentState = { doc: VectorFrame; params: Params; history: History };
export type EditOp = { kind: 'merge_palette'; from: number; into: number }
  | { kind: 'recolor_palette'; color: number; rgb: number[] }
  | { kind: 'delete_palette'; color: number }
  | { kind: 'lock_color'; color: number; locked: boolean }
  | { kind: 'merge_region'; region: number; into: number | null }
  | { kind: 'remove_region'; region: number };
export type FidelityEstimate = { ssim: number; width: number; height: number; estimated: boolean };
export type Tool = 'move' | 'erase' | 'eyedropper' | 'merge';
export type VectorFrame = { width: number; height: number;
  shapes: { region: number | null; color: string; d: string; nodes: { point: [number, number]; corner: boolean }[] }[];
  palette: { id: number; rgb: number[]; locked: boolean }[];
  stats: { paths: number; nodes: number; colors: number } };
export type UiError = { code: string; msg: string; width?: number; height?: number };
export type JobMessage = { kind: 'progress' | 'preview' | 'done' | 'error'; session_id: number; request_id: number;
  job_id: number; stage?: string; frac?: number; doc?: VectorFrame; error?: UiError; params?: Params; history?: History };
export type ExportOptions = { format: 'svg' | 'pdf' | 'eps' | 'dxf' | 'png'; precision: number; overlap: boolean; group_by_color: boolean };
export type View = 'side' | 'slider' | 'vector' | 'outline' | 'difference';
