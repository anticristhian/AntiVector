import type { OpenedImage, VectorFrame } from '../ipc/types';
export const opened: OpenedImage = { session_id: 1, w: 128, h: 128, name: 'logo.png', thumb_png_b64: '', original_png_b64: '',
  detection: { mode: 'logo', colors: 3, has_alpha: false, is_jpeg_like: false, is_pixel_art: false },
  params: { version: 1, mode: 'logo', colors: 'auto', palette_lock: [], denoise: 0.2, min_region_area: 4, antialias_resolve: true,
    corner_threshold_deg: 100, smoothness: 0.5, fit_tolerance_px: 0.9, detail: 0.5, transparency_alpha_cut: 128, seed: 0,
    experimental: { quality_v5: true, denoise_kind: 'bilateral', force_pixel_art: false, pixel_art_strategy: 'preserve_grid', upscale_small: true, curve_fit: true, corner_recovery_radius: 1.5, quantizer: 'kmeans' } } };
export const frame: VectorFrame = { width: 128, height: 128, stats: { paths: 1, nodes: 4, colors: 1 },
  palette: [{ id: 0, rgb: [30, 40, 50], locked: false }], shapes: [{ region: 0, color: '#1e2832', d: 'M0 0L128 0L128 128L0 128Z', nodes: [] }] };
