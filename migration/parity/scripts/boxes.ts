/**
 * Print exact reference element geometry from reference/<state>.boxes.json.
 *
 * Usage:
 *   bun run boxes -- <state> [--grep <class-or-text>] [--region x,y,w,h]
 */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { AUTO_STATES, REFERENCE_DIR, type AutoState } from "./paths.ts";

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

type Region = { x: number; y: number; w: number; h: number };

function argValue(flag: string): string | undefined {
  const idx = process.argv.indexOf(flag);
  if (idx < 0) return undefined;
  return process.argv[idx + 1];
}

function parseRegion(raw: string): Region {
  const parts = raw.split(",").map((p) => Number(p.trim()));
  if (parts.length !== 4 || parts.some((n) => !Number.isFinite(n))) {
    throw new Error(`Invalid --region (want x,y,w,h): ${raw}`);
  }
  return { x: parts[0], y: parts[1], w: parts[2], h: parts[3] };
}

function intersects(r: BoxRect, region: Region): boolean {
  return (
    r.x < region.x + region.w &&
    r.x + r.width > region.x &&
    r.y < region.y + region.h &&
    r.y + r.height > region.y
  );
}

function shortFont(family: string): string {
  const first = family.split(",")[0]?.trim() ?? family;
  return first.replace(/^["']|["']$/g, "");
}

function fmt(n: number): string {
  return Number.isInteger(n) ? String(n) : n.toFixed(2).replace(/\.?0+$/, "");
}

function pad(s: string, w: number): string {
  return s.length >= w ? s : s + " ".repeat(w - s.length);
}

function lineYsFor(
  el: ElementBox,
  textNodes: TextNodeBox[],
): string {
  const matches = textNodes.filter((t) => t.parentSelector === el.selector);
  if (matches.length === 0) return "-";
  const ys = matches
    .flatMap((t) => t.rects.map((r) => fmt(r.y)))
    .filter((v, i, a) => a.indexOf(v) === i);
  return ys.join(",");
}

function usage(): never {
  console.error(
    "Usage: bun run boxes -- <state> [--grep <class-or-text>] [--region x,y,w,h]",
  );
  process.exit(1);
}

function main(): void {
  const positional = process.argv.slice(2).filter((a) => !a.startsWith("-"));
  const state = positional[0] as AutoState | undefined;
  if (!state) usage();
  if (!AUTO_STATES.includes(state)) {
    throw new Error(
      `Unknown state: ${state}. Want one of: ${AUTO_STATES.join(", ")}`,
    );
  }

  const grep = argValue("--grep")?.toLowerCase();
  const regionRaw = argValue("--region");
  const region = regionRaw ? parseRegion(regionRaw) : undefined;

  const path = join(REFERENCE_DIR, `${state}.boxes.json`);
  if (!existsSync(path)) {
    throw new Error(
      `Missing ${path}. Run: bun run capture-reference -- ${state}`,
    );
  }

  const data = JSON.parse(readFileSync(path, "utf8")) as BoxesFile;
  let rows = data.elements;

  if (grep) {
    rows = rows.filter((el) => {
      if (el.tag.toLowerCase().includes(grep)) return true;
      if (el.id.toLowerCase().includes(grep)) return true;
      if (el.classList.some((c) => c.toLowerCase().includes(grep))) return true;
      if (el.text.toLowerCase().includes(grep)) return true;
      const leaf = el.selector.split(" > ").pop()?.toLowerCase() ?? "";
      return leaf.includes(grep);
    });
  }

  if (region) {
    rows = rows.filter((el) => intersects(el.rect, region));
  }

  const colSel = Math.min(
    72,
    Math.max(24, ...rows.map((r) => r.selector.length), 8),
  );

  console.log(
    `${pad("SELECTOR", colSel)}  ${pad("X", 8)}  ${pad("Y", 8)}  ${pad("W", 8)}  ${pad("H", 8)}  ${pad("FONT", 18)}  ${pad("LH", 8)}  LINES(y)`,
  );
  console.log("-".repeat(colSel + 8 + 8 + 8 + 8 + 18 + 8 + 16 + 14));

  for (const el of rows) {
    const font = `${shortFont(el.style.fontFamily)} ${el.style.fontSize}`;
    const sel =
      el.selector.length > colSel
        ? `…${el.selector.slice(-(colSel - 1))}`
        : el.selector;
    console.log(
      `${pad(sel, colSel)}  ${pad(fmt(el.rect.x), 8)}  ${pad(fmt(el.rect.y), 8)}  ${pad(fmt(el.rect.width), 8)}  ${pad(fmt(el.rect.height), 8)}  ${pad(font, 18)}  ${pad(el.style.lineHeight, 8)}  ${lineYsFor(el, data.textNodes)}`,
    );
  }

  console.log(`\n${rows.length} element(s) from ${state}.boxes.json`);
}

main();
