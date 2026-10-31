/**
 * Region-crop DETAIL/FILL diff for sidebar (0,42,340,598) + 5x zooms.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";

const PARITY = "C:/Users/hi/Desktop/Work/HyperType/HyperType/migration/parity";
const REF = join(PARITY, "reference");
const GPUI = join(PARITY, "gpui");
const OUT = join(PARITY, "diff", "s2-sidebar");
const REGION = { x: 0, y: 42, w: 340, h: 598 };
const STATES = [
  "default-selected",
  "row-hover",
  "search-filter",
  "search-no-match",
  "empty-library",
];
const DETAIL_THRESHOLD = 0.1;
const DETAIL_PASS = 1.0;
const FILL_BLOCK = 4;
const FILL_TOL = 3;
const FILL_PASS = 3.0;

function load(path) {
  return PNG.sync.read(readFileSync(path));
}

function crop(png, { x, y, w, h }) {
  const out = new PNG({ width: w, height: h });
  for (let row = 0; row < h; row++) {
    for (let col = 0; col < w; col++) {
      const sx = x + col;
      const sy = y + row;
      if (sx < 0 || sy < 0 || sx >= png.width || sy >= png.height) continue;
      const si = (sy * png.width + sx) * 4;
      const di = (row * w + col) * 4;
      out.data[di] = png.data[si];
      out.data[di + 1] = png.data[si + 1];
      out.data[di + 2] = png.data[si + 2];
      out.data[di + 3] = png.data[si + 3];
    }
  }
  return out;
}

function blockAvg(data, width, height, bx, by, block) {
  let r = 0, g = 0, b = 0, n = 0;
  const x0 = bx * block;
  const y0 = by * block;
  const x1 = Math.min(x0 + block, width);
  const y1 = Math.min(y0 + block, height);
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const i = (y * width + x) * 4;
      r += data[i];
      g += data[i + 1];
      b += data[i + 2];
      n++;
    }
  }
  if (!n) return [0, 0, 0];
  return [r / n, g / n, b / n];
}

function fillDiff(ref, cmp) {
  const cols = Math.ceil(ref.width / FILL_BLOCK);
  const rows = Math.ceil(ref.height / FILL_BLOCK);
  let mismatched = 0;
  const total = cols * rows;
  const failing = [];
  for (let by = 0; by < rows; by++) {
    for (let bx = 0; bx < cols; bx++) {
      const [r1, g1, b1] = blockAvg(ref.data, ref.width, ref.height, bx, by, FILL_BLOCK);
      const [r2, g2, b2] = blockAvg(cmp.data, cmp.width, cmp.height, bx, by, FILL_BLOCK);
      const max = Math.max(Math.abs(r1 - r2), Math.abs(g1 - g2), Math.abs(b1 - b2));
      if (max > FILL_TOL) {
        mismatched++;
        if (failing.length < 80) {
          failing.push({
            bx,
            by,
            x: bx * FILL_BLOCK + REGION.x,
            y: by * FILL_BLOCK + REGION.y,
            max: Math.round(max),
          });
        }
      }
    }
  }
  return { mismatched, total, percent: total ? (mismatched / total) * 100 : 0, failing };
}

function zoom5(png, rect, outPath) {
  const src = crop(png, rect);
  const scale = 5;
  const out = new PNG({ width: src.width * scale, height: src.height * scale });
  for (let y = 0; y < src.height; y++) {
    for (let x = 0; x < src.width; x++) {
      const si = (y * src.width + x) * 4;
      for (let dy = 0; dy < scale; dy++) {
        for (let dx = 0; dx < scale; dx++) {
          const di = ((y * scale + dy) * out.width + (x * scale + dx)) * 4;
          out.data[di] = src.data[si];
          out.data[di + 1] = src.data[si + 1];
          out.data[di + 2] = src.data[si + 2];
          out.data[di + 3] = 255;
        }
      }
    }
  }
  writeFileSync(outPath, PNG.sync.write(out));
}

mkdirSync(OUT, { recursive: true });
const results = [];

for (const state of STATES) {
  const refPath = join(REF, `${state}.png`);
  const cmpPath = join(GPUI, `${state}.png`);
  if (!existsSync(refPath) || !existsSync(cmpPath)) {
    results.push({ state, error: "missing png" });
    console.log(`${state}: MISSING`);
    continue;
  }
  const refFull = load(refPath);
  const cmpFull = load(cmpPath);
  const ref = crop(refFull, REGION);
  const cmp = crop(cmpFull, REGION);
  writeFileSync(join(OUT, `${state}-ref-region.png`), PNG.sync.write(ref));
  writeFileSync(join(OUT, `${state}-gpui-region.png`), PNG.sync.write(cmp));

  const detailPng = new PNG({ width: ref.width, height: ref.height });
  const mismatched = pixelmatch(ref.data, cmp.data, detailPng.data, ref.width, ref.height, {
    threshold: DETAIL_THRESHOLD,
    includeAA: true,
  });
  const total = ref.width * ref.height;
  const detailPct = (mismatched / total) * 100;
  writeFileSync(join(OUT, `${state}-detail.png`), PNG.sync.write(detailPng));
  const fill = fillDiff(ref, cmp);
  const pass = detailPct <= DETAIL_PASS && fill.percent <= FILL_PASS;
  const row = {
    state,
    detailPct,
    mismatched,
    total,
    fillPct: fill.percent,
    fillMismatched: fill.mismatched,
    fillTotal: fill.total,
    pass,
    failing: fill.failing,
  };
  results.push(row);
  console.log(
    `${state}: detail ${detailPct.toFixed(3)}% (${mismatched}/${total}) fill ${fill.percent.toFixed(3)}% (${fill.mismatched}/${fill.total}) ${pass ? "PASS" : "FAIL"}`,
  );
}

// 5x zooms from default-selected / empty / search-no-match
const zoomSpecs = [
  { state: "default-selected", name: "top-section", rect: { x: 0, y: 42, w: 340, h: 99 } },
  { state: "default-selected", name: "list-header", rect: { x: 0, y: 141, w: 340, h: 31 } },
  { state: "default-selected", name: "row1", rect: { x: 10, y: 177, w: 320, h: 60 } },
  { state: "default-selected", name: "row2", rect: { x: 10, y: 241, w: 320, h: 60 } },
  { state: "search-no-match", name: "empty-state", rect: { x: 10, y: 177, w: 320, h: 175 } },
  { state: "empty-library", name: "empty-library", rect: { x: 10, y: 177, w: 320, h: 175 } },
];

for (const z of zoomSpecs) {
  const refP = join(REF, `${z.state}.png`);
  const gpuiP = join(GPUI, `${z.state}.png`);
  if (!existsSync(refP) || !existsSync(gpuiP)) continue;
  zoom5(load(refP), z.rect, join(OUT, `zoom5-ref-${z.name}.png`));
  zoom5(load(gpuiP), z.rect, join(OUT, `zoom5-gpui-${z.name}.png`));
}

writeFileSync(join(OUT, "summary.json"), JSON.stringify(results, null, 2));
console.log(`Wrote ${OUT}`);
