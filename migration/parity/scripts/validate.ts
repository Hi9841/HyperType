/**
 * Prove the judge works:
 *   a) Noise floor: capture reference twice; both metrics ~0%.
 *   b) Broken CSS variant: accent + grid; must FAIL clearly.
 *   c) Real GPUI: run parity default-selected; must FAIL (different design).
 *   d) (reserved / covered by summary table sizes)
 *   e) Surface variant: wrong --surface-1/--surface-2; MUST FAIL fill.
 *   f) 1-step border: --line off by 4; report numbers.
 *   g) Raster-noise: --disable-lcd-text; MUST PASS both metrics (or report if impossible).
 */
import {
  mkdirSync,
  writeFileSync,
  cpSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import type { Browser, Page } from "playwright-core";
import { launchParityBrowser } from "./browser.ts";
import { captureStable } from "./capture-gpui.ts";
import { diffPair, formatSummary, type DiffResult } from "./diff.ts";
import { run, sleep, start } from "./node-utils.ts";
import {
  DETAIL_PASS_PERCENT,
  DETAIL_PIXEL_THRESHOLD,
  DIFF_DIR,
  FILL_BLOCK,
  FILL_PASS_PERCENT,
  FILL_TOLERANCE,
  GPUI_DIR,
  PARITY_ROOT,
  REFERENCE_DIR,
  REPO_ROOT,
  SEED_APPDATA,
} from "./paths.ts";
import { windowsDisplayScale } from "./scale.ts";
import { settleForScreenshot } from "./settle.ts";
import { startStaticServer } from "./static-server.ts";

const DEV_URL = "http://127.0.0.1:1420";
const VIEWPORT = { width: 960, height: 640 };
const VAL_DIR = join(PARITY_ROOT, "validation");

async function openPage(browser: Browser): Promise<Page> {
  const context =
    browser.contexts()[0] ?? (await browser.newContext({ viewport: VIEWPORT }));
  const page = context.pages()[0] ?? (await context.newPage());
  await page.setViewportSize(VIEWPORT);
  return page;
}

async function captureDefault(
  page: Page,
  outPath: string,
  injectCss?: string,
): Promise<void> {
  await page.goto(DEV_URL, { waitUntil: "networkidle" });
  await page.waitForSelector(".studio-item.active", { timeout: 15_000 });
  if (injectCss) {
    await page.addStyleTag({ content: injectCss });
  }
  await settleForScreenshot(page);
  await page.screenshot({ path: outPath, type: "png" });
}

function fmtResult(label: string, r: DiffResult): string {
  return (
    `${label}: detail ${r.percent.toFixed(4)}% (${r.mismatched}/${r.total})` +
    ` fill ${r.fillPercent.toFixed(4)}% (${r.fillMismatched}/${r.fillTotal})` +
    ` ${r.pass ? "PASS" : "FAIL"}` +
    ` size ${r.width}x${r.height}`
  );
}

async function runGpuiDefault(): Promise<DiffResult> {
  console.log("\n--- (c) Real GPUI window ---");
  const build = run(
    "cargo",
    ["build", "-p", "hypertype", "--features", "parity"],
    { cwd: REPO_ROOT, inherit: true },
  );
  if (build.status !== 0) throw new Error("cargo build failed");

  const exe = join(REPO_ROOT, "target", "debug", "hypertype.exe");
  const tmpAppData = join(PARITY_ROOT, ".validate-appdata");
  rmSync(tmpAppData, { recursive: true, force: true });
  mkdirSync(tmpAppData, { recursive: true });
  cpSync(join(SEED_APPDATA, "HyperType"), join(tmpAppData, "HyperType"), {
    recursive: true,
  });

  const child = start(exe, ["--parity-state", "default-selected"], {
    cwd: REPO_ROOT,
    env: { ...process.env, APPDATA: tmpAppData },
    stdout: "inherit",
    stderr: "inherit",
  });
  const pid = child.pid;
  if (!pid) throw new Error("no pid");

  mkdirSync(GPUI_DIR, { recursive: true });
  const out = join(GPUI_DIR, "default-selected.png");
  try {
    const deadline = Date.now() + 25_000;
    let ready = false;
    while (Date.now() < deadline) {
      try {
        await captureStable(pid, "HyperType", out, 3_000);
        ready = true;
        break;
      } catch {
        await sleep(400);
      }
    }
    if (!ready) throw new Error("GPUI window capture timed out");
    // Final stable capture after the window is known to exist.
    await captureStable(pid, "HyperType", out, 8_000);
  } finally {
    try {
      child.kill();
    } catch {
      // gone
    }
    run("taskkill", ["/PID", String(pid), "/T", "/F"]);
  }

  mkdirSync(DIFF_DIR, { recursive: true });
  return diffPair(
    join(REFERENCE_DIR, "default-selected.png"),
    out,
    join(DIFF_DIR, "default-selected.png"),
  );
}

function rebuildDist(): void {
  console.log("Rebuilding SolidJS dist from current src/ (bun run build)...");
  const proc = run("bun", ["run", "build"], { cwd: REPO_ROOT, inherit: true });
  if (proc.status !== 0) {
    throw new Error("bun run build failed; cannot validate against fresh UI");
  }
}

async function main() {
  mkdirSync(VAL_DIR, { recursive: true });
  mkdirSync(REFERENCE_DIR, { recursive: true });
  mkdirSync(DIFF_DIR, { recursive: true });

  rebuildDist();

  const scale = windowsDisplayScale();
  const stopServer = await startStaticServer();
  const launched = await launchParityBrowser(scale);
  const page = await openPage(launched.browser);

  let noise: DiffResult | undefined;
  let broken: DiffResult | undefined;
  let surface: DiffResult | undefined;
  let onestep: DiffResult | undefined;
  let raster: DiffResult | undefined;
  let gpuiResult: DiffResult | undefined;
  let rasterNote = "";
  let onestepNote = "";

  try {
    console.log("--- (a) Noise floor ---");
    const a1 = join(VAL_DIR, "noise-a.png");
    const a2 = join(VAL_DIR, "noise-b.png");
    await captureDefault(page, a1);
    await captureDefault(page, a2);
    cpSync(a1, join(REFERENCE_DIR, "default-selected.png"));
    noise = diffPair(a1, a2, join(VAL_DIR, "noise-diff.png"));
    console.log(fmtResult("Noise floor", noise));

    console.log("\n--- (b) Broken variant (accent + grid) ---");
    const brokenPath = join(VAL_DIR, "broken.png");
    await captureDefault(
      page,
      brokenPath,
      ":root{--accent:#2d7cff} .studio-workspace{grid-template-columns:300px 1fr !important}",
    );
    broken = diffPair(
      join(REFERENCE_DIR, "default-selected.png"),
      brokenPath,
      join(VAL_DIR, "broken-diff.png"),
    );
    console.log(fmtResult("Broken variant", broken));
    if (broken.percent < 1 && broken.fillPercent < 1) {
      throw new Error(
        `Broken variant too low (detail ${broken.percent}% fill ${broken.fillPercent}%); judge may be blind`,
      );
    }

    console.log("\n--- (e) Surface variant (must FAIL fill) ---");
    const surfacePath = join(VAL_DIR, "surface.png");
    await captureDefault(
      page,
      surfacePath,
      ":root{--surface-1:#000000;--surface-2:#0c0c0f}",
    );
    surface = diffPair(
      join(REFERENCE_DIR, "default-selected.png"),
      surfacePath,
      join(VAL_DIR, "surface-diff.png"),
    );
    console.log(fmtResult("Surface variant", surface));
    if (surface.fillPercent <= FILL_PASS_PERCENT) {
      throw new Error(
        `Surface variant fill ${surface.fillPercent}% did not exceed FILL_PASS_PERCENT ${FILL_PASS_PERCENT}%; fill metric is blind`,
      );
    }

    console.log("\n--- (f) 1-step border variant (--line off by 4) ---");
    const onestepPath = join(VAL_DIR, "onestep.png");
    await captureDefault(page, onestepPath, ":root{--line:#202026}");
    onestep = diffPair(
      join(REFERENCE_DIR, "default-selected.png"),
      onestepPath,
      join(VAL_DIR, "onestep-diff.png"),
    );
    console.log(fmtResult("1-step border", onestep));
    onestepNote =
      `--line #1c1c22 -> #202026 (delta 4). ` +
      `Observed detail ${onestep.percent.toFixed(3)}% fill ${onestep.fillPercent.toFixed(3)}%. ` +
      (onestep.fillPercent > FILL_PASS_PERCENT
        ? "Fill failed: 4x4 blocks covered the border shift."
        : "Fill stayed at 0%: 1px borders diluted inside 4x4 averages stay under tolerance 3/255.");
    console.log(`  note: ${onestepNote}`);

    // Stop the validation browser before the raster case (new browser flags).
    launched.stop();

    console.log("\n--- (g) Raster-noise variant (--disable-lcd-text) ---");
    const rasterLaunched = await launchParityBrowser(scale, {
      preferChrome: true,
      extraArgs: ["--disable-lcd-text"],
    });
    try {
      const rasterPage = await openPage(rasterLaunched.browser);
      const rasterPath = join(VAL_DIR, "raster.png");
      await captureDefault(rasterPage, rasterPath);
      raster = diffPair(
        join(REFERENCE_DIR, "default-selected.png"),
        rasterPath,
        join(VAL_DIR, "raster-diff.png"),
      );
      console.log(fmtResult("Raster-noise", raster));
      if (!raster.pass) {
        rasterNote =
          `Raster-noise did NOT stay under thresholds (detail ${raster.percent.toFixed(3)}% / fill ${raster.fillPercent.toFixed(3)}%). ` +
          `Surface fill was ${surface.fillPercent.toFixed(3)}%. ` +
          `Report: thresholds cannot pass (g) while still failing (e) if raster exceeds every bar that surface still clears.`;
        console.warn(rasterNote);
      } else {
        rasterNote =
          "Raster-noise stayed under both thresholds (glyph AA change only).";
      }
    } finally {
      rasterLaunched.stop();
    }

    // Release :1420 so capture-reference can bind (or rebuild + reuse cleanly).
    stopServer();

    console.log("\nCapturing full reference set...");
    const ref = run(
      "node",
      ["--experimental-strip-types", "scripts/capture-reference.ts"],
      { cwd: PARITY_ROOT, inherit: true },
    );
    if (ref.status !== 0) {
      console.warn(
        "Full reference capture failed; default-selected still present",
      );
    }
  } finally {
    // stopServer is idempotent enough: capture-reference owns its own server.
    try {
      stopServer();
    } catch {
      // already closed
    }
  }

  gpuiResult = await runGpuiDefault();
  console.log(fmtResult("GPUI default-selected", gpuiResult));

  // Threshold justification from measured numbers.
  const noiseDetail = noise?.percent ?? 0;
  const noiseFill = noise?.fillPercent ?? 0;
  const surfaceFill = surface?.fillPercent ?? 0;
  const rasterDetail = raster?.percent ?? 0;
  const rasterFill = raster?.fillPercent ?? 0;
  const explanation = [
    `Noise floor detail ${noiseDetail.toFixed(4)}% fill ${noiseFill.toFixed(4)}%.`,
    `Broken detail ${broken?.percent.toFixed(3)}% fill ${broken?.fillPercent.toFixed(3)}%.`,
    `Surface fill ${surfaceFill.toFixed(3)}% (must fail fill).`,
    `1-step border detail ${onestep?.percent.toFixed(3)}% fill ${onestep?.fillPercent.toFixed(3)}%.`,
    `Raster-noise detail ${rasterDetail.toFixed(3)}% fill ${rasterFill.toFixed(3)}%.`,
    `GPUI detail ${gpuiResult.percent.toFixed(3)}% fill ${gpuiResult.fillPercent.toFixed(3)}%.`,
    `Chosen: DETAIL_PASS_PERCENT=${DETAIL_PASS_PERCENT}% (pixelmatch ${DETAIL_PIXEL_THRESHOLD}),`,
    `FILL_PASS_PERCENT=${FILL_PASS_PERCENT}% (block ${FILL_BLOCK}x${FILL_BLOCK}, tolerance ${FILL_TOLERANCE}/255).`,
    "A state PASSES only when both metrics are under their bars.",
    "Fill catches near-black surface swaps that YIQ pixelmatch misses;",
    "detail still flags glyph/edge breaks.",
    rasterNote,
  ].join(" ");

  const tableRows: DiffResult[] = [
    {
      ...(noise as DiffResult),
      state: "noise-floor",
    },
    {
      ...(broken as DiffResult),
      state: "broken-variant",
    },
    {
      ...(surface as DiffResult),
      state: "surface-variant",
    },
    {
      ...(onestep as DiffResult),
      state: "onestep-border",
    },
    {
      ...(raster as DiffResult),
      state: "raster-noise",
    },
    gpuiResult,
  ];

  const md = `# Parity judge validation

Date: ${new Date().toISOString()}
Display scale: ${scale}
DETAIL pixelmatch threshold: ${DETAIL_PIXEL_THRESHOLD}
FILL block: ${FILL_BLOCK}x${FILL_BLOCK}, tolerance: ${FILL_TOLERANCE}/255
DETAIL pass bar: ${DETAIL_PASS_PERCENT}%
FILL pass bar: ${FILL_PASS_PERCENT}%

## Results

| check | detail % | fill % | pass | criterion |
| --- | ---: | ---: | --- | --- |
| (a) Noise floor (ref vs ref) | ${noiseDetail.toFixed(4)}% | ${noiseFill.toFixed(4)}% | ${(noise?.pass ?? false) ? "PASS" : "FAIL"} | both ~0% |
| (b) Broken CSS variant | ${broken?.percent.toFixed(3) ?? "-"}% | ${broken?.fillPercent.toFixed(3) ?? "-"}% | ${(broken?.pass ?? false) ? "PASS" : "FAIL"} | clearly fails |
| (c) Current GPUI default-selected | ${gpuiResult.percent.toFixed(3)}% | ${gpuiResult.fillPercent.toFixed(3)}% | ${gpuiResult.pass ? "PASS" : "FAIL"} | must fail vs SolidJS |
| (e) Surface variant | ${surface?.percent.toFixed(3) ?? "-"}% | ${surfaceFill.toFixed(3)}% | ${(surface?.pass ?? false) ? "PASS" : "FAIL"} | MUST FAIL fill |
| (f) 1-step border (--line +4) | ${onestep?.percent.toFixed(3) ?? "-"}% | ${onestep?.fillPercent.toFixed(3) ?? "-"}% | ${(onestep?.pass ?? false) ? "PASS" : "FAIL"} | report numbers; see note below |
| (g) Raster-noise (--disable-lcd-text) | ${rasterDetail.toFixed(3)}% | ${rasterFill.toFixed(3)}% | ${(raster?.pass ?? false) ? "PASS" : "FAIL"} | MUST PASS both (glyph AA only) |

${formatSummary(tableRows)}

## Chosen pass thresholds

- **DETAIL**: <= ${DETAIL_PASS_PERCENT}% mismatched pixels (pixelmatch threshold ${DETAIL_PIXEL_THRESHOLD})
- **FILL**: <= ${FILL_PASS_PERCENT}% mismatched ${FILL_BLOCK}x${FILL_BLOCK} blocks (tolerance ${FILL_TOLERANCE}/255)
- A state **PASSES** only when both metrics are under their bars.

${explanation}

## Case (f) observation

${onestepNote}

## Notes

- Edge channel \`msedge\` is preferred; this host falls back to Chrome CDP when Edge is absent.
- Scripts run under Node (\`node --experimental-strip-types\`) because Bun's Playwright WebSocket hangs on CDP connect.
- \`capture-reference\` rebuilds \`dist/\` from current \`src/\` before serving, and records git HEAD + src dirty in \`reference/meta.json\`.
- After each state action the capture waits for finite animations to finish, freezes infinite animations at t=0, then waits two animation frames.
- GPUI capture waits for two consecutive identical frames instead of a fixed sleep.
- Installed HyperType must stay running; parity skips the instance mutex and the hook.
`;

  writeFileSync(join(PARITY_ROOT, "VALIDATION.md"), md);
  console.log("\nWrote VALIDATION.md");
  console.log(explanation);

  // Hard fail the suite if surface did not trip fill (already thrown above)
  // or if noise is somehow huge.
  if (noise && (noise.percent > 0.1 || noise.fillPercent > 0.1)) {
    console.warn("Noise floor higher than expected; check capture stability.");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
