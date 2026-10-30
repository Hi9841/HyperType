import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

export const PARITY_ROOT = join(HERE, "..");
export const REPO_ROOT = join(PARITY_ROOT, "..", "..");
export const REFERENCE_DIR = join(PARITY_ROOT, "reference");
export const GPUI_DIR = join(PARITY_ROOT, "gpui");
export const DIFF_DIR = join(PARITY_ROOT, "diff");
export const SEED_APPDATA = join(PARITY_ROOT, "seed");

/** States captured automatically from the browser mock. */
export const AUTO_STATES = [
  "default-selected",
  "row-hover",
  "new-snippet",
  "shortcut-mode-empty",
  "shortcut-recording",
  "search-filter",
  "search-no-match",
  "empty-library",
  "settings-modal",
  "tokens-modal",
  "save-disabled",
] as const;

export type AutoState = (typeof AUTO_STATES)[number];

/**
 * DETAIL metric: pixelmatch per-pixel YIQ threshold (0-1 channel scale).
 * Catches glyph and edge differences. Small ClearType / AA noise stays under
 * this; layout and color token changes go over it when they hit edges.
 */
export const DETAIL_PIXEL_THRESHOLD = 0.1;

/** @deprecated Use DETAIL_PIXEL_THRESHOLD. Kept for older call sites. */
export const PIXEL_THRESHOLD = DETAIL_PIXEL_THRESHOLD;

/**
 * FILL metric: box-filter block size in pixels (N x N average RGB).
 * Ignores glyph raster noise; catches wrong surface / border colors.
 */
export const FILL_BLOCK = 4;

/**
 * FILL metric: max per-channel abs difference (0-255) after box average.
 * Start at 3/255. A 1-step border off-by-4 (#1c1c22 -> #202026) must fail.
 */
export const FILL_TOLERANCE = 3;

/**
 * Pass bars (percent). A state PASSES only when BOTH metrics are under these.
 * Tuned from VALIDATION.md cases (a)-(g):
 *   noise ~0/0, raster-noise ~0.66/2.56, surface fill ~38.6, broken ~3.1/11.7
 * DETAIL must sit above LCD-text AA noise and below a deliberate layout break.
 * FILL must sit above glyph-bleed into 4x4 blocks and below a surface swap.
 */
export const DETAIL_PASS_PERCENT = 1.0;
export const FILL_PASS_PERCENT = 3.0;
