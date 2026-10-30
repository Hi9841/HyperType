/**
 * Diff reference/<state>.png vs gpui/<state>.png with two metrics:
 *
 *   DETAIL - pixelmatch (glyphs / edges)
 *   FILL   - box-downsampled RGB (surfaces / layout color)
 *
 * A state PASSES only when both metrics are under their pass thresholds.
 *
 * Usage:
 *   bun run diff
 *   bun run diff -- default-selected
 *   bun run diff -- --ref-dir reference --cmp-dir gpui --out-dir diff
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, basename } from "node:path";
import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";
import {
  DETAIL_PASS_PERCENT,
  DETAIL_PIXEL_THRESHOLD,
  DIFF_DIR,
  FILL_BLOCK,
  FILL_PASS_PERCENT,
  FILL_TOLERANCE,
  GPUI_DIR,
  REFERENCE_DIR,
} from "./paths.ts";

function argValue(flag: string): string | undefined {
  const idx = process.argv.indexOf(flag);
  if (idx < 0) return undefined;
  return process.argv[idx + 1];
}

function loadPng(path: string): PNG {
  return PNG.sync.read(readFileSync(path));
}

export type Region = { x: number; y: number; w: number; h: number };

export type DiffResult = {
  state: string;
  width: number;
  height: number;
  /** DETAIL (pixelmatch) mismatched pixel count. */
  mismatched: number;
  total: number;
  /** DETAIL mismatch percent. */
  percent: number;
  /** FILL mismatched block count. */
  fillMismatched: number;
  fillTotal: number;
  /** FILL mismatch percent. */
  fillPercent: number;
  okSize: boolean;
  /** True when both metrics are under their pass thresholds. */
  pass: boolean;
  error?: string;
  /** Crop rectangle when `--region x,y,w,h` was used. */
  region?: Region;
};

/** Parse `x,y,w,h` from `--region`. */
export function parseRegion(raw: string | undefined): Region | undefined {
  if (!raw) return undefined;
  const parts = raw.split(",").map((p) => Number(p.trim()));
  if (parts.length !== 4 || parts.some((n) => !Number.isFinite(n) || n < 0)) {
    throw new Error(`invalid --region '${raw}' (expected x,y,w,h)`);
  }
  const [x, y, w, h] = parts as [number, number, number, number];
  if (w <= 0 || h <= 0) {
    throw new Error(`invalid --region '${raw}' (w and h must be > 0)`);
  }
  return { x, y, w, h };
}

/** Crop a PNG to an inclusive pixel rectangle clipped to image bounds. */
export function cropPng(src: PNG, region: Region): PNG {
  const x0 = Math.max(0, Math.floor(region.x));
  const y0 = Math.max(0, Math.floor(region.y));
  const x1 = Math.min(src.width, x0 + Math.floor(region.w));
  const y1 = Math.min(src.height, y0 + Math.floor(region.h));
  const w = Math.max(0, x1 - x0);
  const h = Math.max(0, y1 - y0);
  const out = new PNG({ width: w, height: h });
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const si = ((y0 + y) * src.width + (x0 + x)) * 4;
      const di = (y * w + x) * 4;
      out.data[di] = src.data[si]!;
      out.data[di + 1] = src.data[si + 1]!;
      out.data[di + 2] = src.data[si + 2]!;
      out.data[di + 3] = src.data[si + 3]!;
    }
  }
  return out;
}

function emptyResult(state: string, error: string, region?: Region): DiffResult {
  return {
    state,
    width: 0,
    height: 0,
    mismatched: 0,
    total: 0,
    percent: 100,
    fillMismatched: 0,
    fillTotal: 0,
    fillPercent: 100,
    okSize: false,
    pass: false,
    error,
    region,
  };
}

/** Average RGB over an NxN block. Out-of-bounds samples are skipped. */
function blockAverage(
  data: Buffer,
  width: number,
  height: number,
  bx: number,
  by: number,
  block: number,
): [number, number, number] {
  let r = 0;
  let g = 0;
  let b = 0;
  let n = 0;
  const x0 = bx * block;
  const y0 = by * block;
  const x1 = Math.min(x0 + block, width);
  const y1 = Math.min(y0 + block, height);
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const i = (y * width + x) * 4;
      r += data[i]!;
      g += data[i + 1]!;
      b += data[i + 2]!;
      n++;
    }
  }
  if (n === 0) return [0, 0, 0];
  return [r / n, g / n, b / n];
}

/**
 * FILL metric: downsample both images with an NxN box average, then count
 * blocks whose max per-channel abs difference exceeds FILL_TOLERANCE.
 * Writes a magnified fill-diff PNG (each block is one colored cell).
 */
export function fillDiff(
  ref: PNG,
  cmp: PNG,
  fillDiffPath: string,
  block = FILL_BLOCK,
  tolerance = FILL_TOLERANCE,
): { mismatched: number; total: number; percent: number } {
  const cols = Math.ceil(ref.width / block);
  const rows = Math.ceil(ref.height / block);
  const out = new PNG({ width: cols, height: rows });
  let mismatched = 0;
  const total = cols * rows;

  for (let by = 0; by < rows; by++) {
    for (let bx = 0; bx < cols; bx++) {
      const [r1, g1, b1] = blockAverage(ref.data, ref.width, ref.height, bx, by, block);
      const [r2, g2, b2] = blockAverage(cmp.data, cmp.width, cmp.height, bx, by, block);
      const dr = Math.abs(r1 - r2);
      const dg = Math.abs(g1 - g2);
      const db = Math.abs(b1 - b2);
      const max = Math.max(dr, dg, db);
      const i = (by * cols + bx) * 4;
      if (max > tolerance) {
        mismatched++;
        // Hot magenta for mismatched fill blocks.
        out.data[i] = 255;
        out.data[i + 1] = 0;
        out.data[i + 2] = 255;
        out.data[i + 3] = 255;
      } else {
        // Dim average of the two blocks for context.
        out.data[i] = Math.round((r1 + r2) / 2);
        out.data[i + 1] = Math.round((g1 + g2) / 2);
        out.data[i + 2] = Math.round((b1 + b2) / 2);
        out.data[i + 3] = 80;
      }
    }
  }

  mkdirSync(join(fillDiffPath, ".."), { recursive: true });
  writeFileSync(fillDiffPath, PNG.sync.write(out));
  const percent = total === 0 ? 0 : (mismatched / total) * 100;
  return { mismatched, total, percent };
}

export function diffPair(
  refPath: string,
  cmpPath: string,
  diffPath: string,
  detailThreshold = DETAIL_PIXEL_THRESHOLD,
  region?: Region,
): DiffResult {
  const state = basename(refPath, ".png");
  if (!existsSync(refPath)) {
    return emptyResult(state, `missing reference: ${refPath}`, region);
  }
  if (!existsSync(cmpPath)) {
    return emptyResult(state, `missing compare image: ${cmpPath}`, region);
  }

  let ref = loadPng(refPath);
  let cmp = loadPng(cmpPath);
  if (ref.width !== cmp.width || ref.height !== cmp.height) {
    const msg = `size mismatch for ${state}: reference ${ref.width}x${ref.height} vs compare ${cmp.width}x${cmp.height}`;
    console.error(msg);
    return {
      state,
      width: ref.width,
      height: ref.height,
      mismatched: ref.width * ref.height,
      total: ref.width * ref.height,
      percent: 100,
      fillMismatched: 0,
      fillTotal: 0,
      fillPercent: 100,
      okSize: false,
      pass: false,
      error: msg,
      region,
    };
  }

  if (region) {
    ref = cropPng(ref, region);
    cmp = cropPng(cmp, region);
  }

  const detailPng = new PNG({ width: ref.width, height: ref.height });
  const mismatched = pixelmatch(
    ref.data,
    cmp.data,
    detailPng.data,
    ref.width,
    ref.height,
    { threshold: detailThreshold, includeAA: true },
  );
  const total = ref.width * ref.height;
  const percent = total === 0 ? 0 : (mismatched / total) * 100;

  mkdirSync(join(diffPath, ".."), { recursive: true });
  writeFileSync(diffPath, PNG.sync.write(detailPng));

  const fillPath = diffPath.replace(/\.png$/i, "-fill.png");
  const fill = fillDiff(ref, cmp, fillPath);

  const pass =
    percent <= DETAIL_PASS_PERCENT && fill.percent <= FILL_PASS_PERCENT;

  return {
    state,
    width: ref.width,
    height: ref.height,
    mismatched,
    total,
    percent,
    fillMismatched: fill.mismatched,
    fillTotal: fill.total,
    fillPercent: fill.percent,
    okSize: true,
    pass,
    region,
  };
}

export function formatSummary(results: DiffResult[]): string {
  const lines = [
    "| state | size | detail % | detail px | fill % | fill blocks | pass | note |",
    "| --- | --- | ---: | ---: | ---: | ---: | --- | --- |",
  ];
  for (const r of results) {
    const size = r.okSize ? `${r.width}x${r.height}` : "-";
    const detailPct = r.percent.toFixed(3);
    const detailPx = r.okSize ? String(r.mismatched) : "-";
    const fillPct = r.fillPercent.toFixed(3);
    const fillBlocks = r.okSize
      ? `${r.fillMismatched}/${r.fillTotal}`
      : "-";
    const pass = r.error ? "ERROR" : r.pass ? "PASS" : "FAIL";
    const regionNote = r.region
      ? `region ${r.region.x},${r.region.y},${r.region.w},${r.region.h}`
      : "";
    const note = [r.error, regionNote].filter(Boolean).join("; ");
    lines.push(
      `| ${r.state} | ${size} | ${detailPct}% | ${detailPx} | ${fillPct}% | ${fillBlocks} | ${pass} | ${note} |`,
    );
  }
  return lines.join("\n");
}

function main() {
  const refDir = argValue("--ref-dir") ?? REFERENCE_DIR;
  const cmpDir = argValue("--cmp-dir") ?? GPUI_DIR;
  const outDir = argValue("--out-dir") ?? DIFF_DIR;
  const region = parseRegion(argValue("--region"));
  const positional = process.argv
    .slice(2)
    .filter((a) => !a.startsWith("-") && !a.includes("\\") && !a.includes("/"));
  const dash = process.argv.indexOf("--");
  const after =
    dash >= 0
      ? process.argv.slice(dash + 1).filter((a) => !a.startsWith("-"))
      : [];
  const statesArg =
    after.length > 0
      ? after
      : positional.filter((a) => !["diff.ts", "diff"].includes(a));

  mkdirSync(outDir, { recursive: true });

  let states: string[];
  if (statesArg.length > 0) {
    states = statesArg;
  } else {
    states = readdirSync(refDir)
      .filter((f) => f.endsWith(".png"))
      .map((f) => f.replace(/\.png$/, ""));
  }

  if (states.length === 0) {
    console.error("No states to diff");
    process.exit(1);
  }

  const results: DiffResult[] = [];
  let failed = false;
  for (const state of states) {
    const outName = region
      ? `${state}-r${region.x}_${region.y}_${region.w}_${region.h}.png`
      : `${state}.png`;
    const result = diffPair(
      join(refDir, `${state}.png`),
      join(cmpDir, `${state}.png`),
      join(outDir, outName),
      DETAIL_PIXEL_THRESHOLD,
      region,
    );
    results.push(result);
    if (result.error || !result.pass) failed = true;
    const regionLabel = region
      ? ` region ${region.x},${region.y},${region.w},${region.h}`
      : "";
    console.log(
      `${state}${regionLabel}: detail ${result.percent.toFixed(3)}% (${result.mismatched}/${result.total})` +
        ` fill ${result.fillPercent.toFixed(3)}% (${result.fillMismatched}/${result.fillTotal})` +
        ` ${result.pass ? "PASS" : "FAIL"}` +
        (result.error ? ` ERROR: ${result.error}` : ""),
    );
  }

  const summary = formatSummary(results);
  writeFileSync(join(outDir, "summary.md"), summary + "\n");
  console.log("\n" + summary);
  console.log(`\nWrote ${join(outDir, "summary.md")}`);
  console.log(
    `DETAIL threshold: ${DETAIL_PIXEL_THRESHOLD} (pass <= ${DETAIL_PASS_PERCENT}%)`,
  );
  console.log(
    `FILL block=${FILL_BLOCK} tolerance=${FILL_TOLERANCE}/255 (pass <= ${FILL_PASS_PERCENT}%)`,
  );

  if (failed) process.exit(2);
}

const isEntry =
  process.argv[1] !== undefined &&
  (process.argv[1].endsWith("diff.ts") || process.argv[1].endsWith("diff"));
if (isEntry) {
  main();
}
