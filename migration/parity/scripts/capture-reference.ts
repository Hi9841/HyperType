/**
 * Drive the SolidJS Vite UI with Playwright and save reference PNGs.
 * Prefers Edge (WebView2). Falls back to Chrome/Chromium CDP when Edge is missing.
 *
 * Rebuilds repo `dist/` from current `src/` before serving (stale-reference fix).
 * After each state's actions, settles animations then screenshots.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "playwright-core";
import { launchParityBrowser } from "./browser.ts";
import { run, runText, sleep } from "./node-utils.ts";
import {
  AUTO_STATES,
  REFERENCE_DIR,
  REPO_ROOT,
  type AutoState,
} from "./paths.ts";
import { windowsDisplayScale } from "./scale.ts";
import { settleForScreenshot, INFINITE_ANIM_FREEZE_MS } from "./settle.ts";
import { startStaticServer } from "./static-server.ts";

const DEV_URL = "http://127.0.0.1:1420";
const VIEWPORT = { width: 960, height: 640 };

function rebuildDist(): void {
  console.log("Rebuilding SolidJS dist from current src/ (bun run build)...");
  const proc = run("bun", ["run", "build"], { cwd: REPO_ROOT, inherit: true });
  if (proc.status !== 0) {
    throw new Error("bun run build failed; cannot capture a fresh reference");
  }
}

function gitMeta(): {
  gitCommit: string;
  srcDirty: boolean;
  gitDescribe: string;
} {
  const head = runText("git", ["rev-parse", "HEAD"], { cwd: REPO_ROOT });
  const dirty = runText("git", ["status", "--porcelain", "--", "src/"], {
    cwd: REPO_ROOT,
  });
  const describe = runText("git", ["describe", "--always", "--dirty"], {
    cwd: REPO_ROOT,
  });
  return {
    gitCommit: (head.stdout || "").trim() || "unknown",
    srcDirty: (dirty.stdout || "").trim().length > 0,
    gitDescribe: (describe.stdout || "").trim() || "unknown",
  };
}

async function resetLibrary(page: Page): Promise<void> {
  let lastErr: unknown;
  for (let attempt = 1; attempt <= 5; attempt++) {
    try {
      await page.goto(DEV_URL, {
        waitUntil: "domcontentloaded",
        timeout: 15_000,
      });
      await page.waitForSelector(".studio-item.active", { timeout: 15_000 });
      return;
    } catch (err) {
      lastErr = err;
      console.warn(`resetLibrary attempt ${attempt} failed; retrying`);
      await sleep(500);
    }
  }
  throw lastErr instanceof Error ? lastErr : new Error(String(lastErr));
}

async function deleteAllSnippets(page: Page): Promise<void> {
  page.removeAllListeners("dialog");
  page.on("dialog", (d) => d.accept());
  for (;;) {
    const btn = page.locator(".studio-item .item-delete-btn").first();
    if ((await btn.count()) === 0) break;
    await btn.click();
    await page.waitForTimeout(150);
  }
  await page.waitForSelector(".sidebar-empty .empty-text", { timeout: 10_000 });
}

async function driveState(page: Page, state: AutoState): Promise<void> {
  switch (state) {
    case "default-selected":
      await resetLibrary(page);
      break;

    case "row-hover":
      await resetLibrary(page);
      await page.locator(".studio-item").nth(1).hover();
      break;

    case "new-snippet":
      await resetLibrary(page);
      await page.click(".new-snippet-btn");
      await page.waitForSelector("text=New Snippet");
      break;

    case "shortcut-mode-empty":
      await resetLibrary(page);
      await page.click(".new-snippet-btn");
      await page.locator(".seg-kind button", { hasText: "Shortcut" }).click();
      await page.waitForSelector("text=Click to Record Shortcut Chord");
      break;

    case "shortcut-recording":
      await resetLibrary(page);
      await page.click(".new-snippet-btn");
      await page.locator(".seg-kind button", { hasText: "Shortcut" }).click();
      await page.click(".recorder-btn");
      await page.waitForSelector(".recorder-btn.listening");
      break;

    case "search-filter":
      await resetLibrary(page);
      await page.fill(".studio-search-input", "o");
      break;

    case "search-no-match":
      await resetLibrary(page);
      await page.fill(".studio-search-input", "zzzzz");
      await page.waitForSelector("text=No matching snippets found");
      break;

    case "empty-library":
      await resetLibrary(page);
      await deleteAllSnippets(page);
      await page.waitForSelector("text=No snippets in library");
      break;

    case "settings-modal":
      await resetLibrary(page);
      await page.click(".settings-icon-btn");
      await page.waitForSelector(".settings-modal-card");
      break;

    case "tokens-modal":
      await resetLibrary(page);
      await page.click(".token-btn-all");
      await page.waitForSelector(".tokens-catalog-card");
      break;

    case "save-disabled":
      await resetLibrary(page);
      await page.click(".new-snippet-btn");
      await page.waitForSelector(".btn-save-snippet[disabled]");
      break;

    default: {
      const _exhaustive: never = state;
      throw new Error(`Unhandled state: ${_exhaustive}`);
    }
  }
}

async function captureState(page: Page, state: AutoState): Promise<void> {
  console.log(`Capturing reference: ${state}`);
  await driveState(page, state);
  await settleForScreenshot(page);
  const out = join(REFERENCE_DIR, `${state}.png`);
  await page.screenshot({ path: out, type: "png" });
  console.log(`  -> ${out}`);
}

async function main() {
  const only = process.argv.slice(2).filter((a) => !a.startsWith("-"));
  const states = (only.length > 0 ? only : [...AUTO_STATES]) as AutoState[];
  for (const s of states) {
    if (!AUTO_STATES.includes(s)) {
      throw new Error(`Unknown auto state: ${s}`);
    }
  }

  rebuildDist();
  const git = gitMeta();
  console.log(
    `Git HEAD ${git.gitCommit} (src dirty: ${git.srcDirty ? "yes" : "no"})`,
  );

  const scale = windowsDisplayScale();
  console.log(`Display scale: ${scale}`);

  mkdirSync(REFERENCE_DIR, { recursive: true });
  const stopServer = await startStaticServer();

  let stopBrowser: (() => void) | undefined;
  try {
    const launched = await launchParityBrowser(scale);
    stopBrowser = launched.stop;
    const context =
      launched.browser.contexts()[0] ??
      (await launched.browser.newContext({
        viewport: VIEWPORT,
        deviceScaleFactor: scale,
      }));
    const page = context.pages()[0] ?? (await context.newPage());
    await page.setViewportSize(VIEWPORT);

    for (const state of states) {
      await captureState(page, state);
    }

    const meta = {
      scale,
      channel: launched.channel,
      browserVersion: launched.version,
      viewport: VIEWPORT,
      deviceScaleFactor: scale,
      date: new Date().toISOString(),
      states,
      gitCommit: git.gitCommit,
      gitDescribe: git.gitDescribe,
      srcDirty: git.srcDirty,
      infiniteAnimFreezeMs: INFINITE_ANIM_FREEZE_MS,
      note: launched.channel.includes("edge")
        ? "Edge/WebView2 channel; rebuilt dist before capture; animations settled"
        : "Edge missing; Chrome/Chromium CDP fallback; rebuilt dist before capture; animations settled",
    };
    writeFileSync(
      join(REFERENCE_DIR, "meta.json"),
      JSON.stringify(meta, null, 2),
    );
    console.log("Wrote reference/meta.json");
  } finally {
    try {
      stopBrowser?.();
    } catch {
      // ignore teardown errors
    }
    try {
      stopServer();
    } catch {
      // ignore teardown errors
    }
  }
}

main()
  .then(() => {
    process.exit(0);
  })
  .catch((err) => {
    console.error(err);
    process.exit(1);
  });
