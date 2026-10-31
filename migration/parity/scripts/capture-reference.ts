/**
 * Drive the SolidJS Vite UI with Playwright and save reference PNGs.
 * Prefers Edge (WebView2). Falls back to Chrome/Chromium CDP when Edge is missing.
 *
 * Builds the v1.1.8 web UI (web-ref.ts) before serving (stale-reference fix).
 * After each state's actions, settles animations then screenshots.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "playwright-core";
import { launchParityBrowser } from "./browser.ts";
import { sleep } from "./node-utils.ts";
import {
  AUTO_STATES,
  REFERENCE_DIR,
  type AutoState,
} from "./paths.ts";
import { windowsDisplayScale } from "./scale.ts";
import { settleForScreenshot, INFINITE_ANIM_FREEZE_MS } from "./settle.ts";
import { startStaticServer } from "./static-server.ts";
import { WEB_REF, buildWebReference, webRefCommit } from "./web-ref.ts";

const DEV_URL = "http://127.0.0.1:1420";
const VIEWPORT = { width: 960, height: 640 };

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

type BoxRect = { x: number; y: number; width: number; height: number };

type ElementBox = {
  selector: string;
  tag: string;
  id: string;
  classList: string[];
  text: string;
  rect: BoxRect;
  style: {
    fontFamily: string;
    fontSize: string;
    fontWeight: string;
    lineHeight: string;
    letterSpacing: string;
    paddingTop: string;
    paddingRight: string;
    paddingBottom: string;
    paddingLeft: string;
    borderTopWidth: string;
    borderRightWidth: string;
    borderBottomWidth: string;
    borderLeftWidth: string;
    borderTopColor: string;
    borderRightColor: string;
    borderBottomColor: string;
    borderLeftColor: string;
    backgroundColor: string;
    color: string;
    borderRadius: string;
    opacity: string;
    boxShadow: string;
  };
};

type TextNodeBox = {
  parentSelector: string;
  text: string;
  rects: BoxRect[];
};

type BoxesFile = {
  state: string;
  viewport: { width: number; height: number };
  elements: ElementBox[];
  textNodes: TextNodeBox[];
};

/** Collect exact element and text-line geometry from the live page. */
async function collectBoxes(page: Page, state: string): Promise<BoxesFile> {
  return page.evaluate((stateName: string) => {
    const rectOf = (r: DOMRectReadOnly): BoxRect => ({
      x: r.x,
      y: r.y,
      width: r.width,
      height: r.height,
    });

    const cssPath = (el: Element): string => {
      const parts: string[] = [];
      let node: Element | null = el;
      while (node && node.nodeType === 1) {
        const tag = node.tagName.toLowerCase();
        if (tag === "html") {
          parts.unshift("html");
          break;
        }
        let part = tag;
        const id = (node as HTMLElement).id;
        if (id) {
          part += `#${CSS.escape(id)}`;
        }
        const classes = [...node.classList]
          .filter((c) => c.length > 0)
          .map((c) => CSS.escape(c));
        if (classes.length > 0) {
          part += `.${classes.join(".")}`;
        }
        const parent = node.parentElement;
        if (parent) {
          const sameTag = [...parent.children].filter(
            (c) => c.tagName === node!.tagName,
          );
          if (sameTag.length > 1) {
            part += `:nth-of-type(${sameTag.indexOf(node) + 1})`;
          }
        }
        parts.unshift(part);
        if (id) break;
        node = parent;
      }
      return parts.join(" > ");
    };

    const ownText = (el: Element): string => {
      let out = "";
      for (const child of el.childNodes) {
        if (child.nodeType === Node.TEXT_NODE) {
          out += child.textContent ?? "";
        }
      }
      return out.slice(0, 60);
    };

    const isVisible = (el: Element, cs: CSSStyleDeclaration): boolean => {
      if (cs.display === "none" || cs.visibility === "hidden") return false;
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0;
    };

    const elements: ElementBox[] = [];
    const all = document.querySelectorAll("*");
    for (const el of all) {
      const cs = getComputedStyle(el);
      if (!isVisible(el, cs)) continue;
      const r = el.getBoundingClientRect();
      elements.push({
        selector: cssPath(el),
        tag: el.tagName.toLowerCase(),
        id: (el as HTMLElement).id || "",
        classList: [...el.classList],
        text: ownText(el),
        rect: rectOf(r),
        style: {
          fontFamily: cs.fontFamily,
          fontSize: cs.fontSize,
          fontWeight: cs.fontWeight,
          lineHeight: cs.lineHeight,
          letterSpacing: cs.letterSpacing,
          paddingTop: cs.paddingTop,
          paddingRight: cs.paddingRight,
          paddingBottom: cs.paddingBottom,
          paddingLeft: cs.paddingLeft,
          borderTopWidth: cs.borderTopWidth,
          borderRightWidth: cs.borderRightWidth,
          borderBottomWidth: cs.borderBottomWidth,
          borderLeftWidth: cs.borderLeftWidth,
          borderTopColor: cs.borderTopColor,
          borderRightColor: cs.borderRightColor,
          borderBottomColor: cs.borderBottomColor,
          borderLeftColor: cs.borderLeftColor,
          backgroundColor: cs.backgroundColor,
          color: cs.color,
          borderRadius: cs.borderRadius,
          opacity: cs.opacity,
          boxShadow: cs.boxShadow,
        },
      });
    }

    const textNodes: TextNodeBox[] = [];
    const walker = document.createTreeWalker(
      document.body ?? document.documentElement,
      NodeFilter.SHOW_TEXT,
    );
    let textNode = walker.nextNode();
    while (textNode) {
      const raw = textNode.textContent ?? "";
      if (raw.trim().length > 0) {
        const parent = textNode.parentElement;
        if (parent) {
          const pcs = getComputedStyle(parent);
          if (pcs.display !== "none" && pcs.visibility !== "hidden") {
            const range = document.createRange();
            range.selectNodeContents(textNode);
            const clientRects = [...range.getClientRects()].map(rectOf);
            const nonZero = clientRects.filter(
              (r) => r.width > 0 && r.height > 0,
            );
            if (nonZero.length > 0) {
              textNodes.push({
                parentSelector: cssPath(parent),
                text: raw.slice(0, 60),
                rects: nonZero,
              });
            }
          }
        }
      }
      textNode = walker.nextNode();
    }

    return {
      state: stateName,
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight,
      },
      elements,
      textNodes,
    };
  }, state);
}

async function captureState(page: Page, state: AutoState): Promise<void> {
  console.log(`Capturing reference: ${state}`);
  await driveState(page, state);
  await settleForScreenshot(page);
  const boxes = await collectBoxes(page, state);
  const boxesOut = join(REFERENCE_DIR, `${state}.boxes.json`);
  writeFileSync(boxesOut, JSON.stringify(boxes, null, 2));
  console.log(
    `  -> ${boxesOut} (${boxes.elements.length} elements, ${boxes.textNodes.length} text nodes)`,
  );
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

  buildWebReference();
  const refCommit = webRefCommit();
  console.log(`Reference ${WEB_REF} at ${refCommit}`);

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
      gitCommit: refCommit,
      gitDescribe: WEB_REF,
      srcDirty: false,
      infiniteAnimFreezeMs: INFINITE_ANIM_FREEZE_MS,
      note: launched.channel.includes("edge")
        ? "Edge/WebView2 channel; built the v1.1.8 web UI before capture; animations settled"
        : "Edge missing; Chrome/Chromium CDP fallback; built the v1.1.8 web UI before capture; animations settled",
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
