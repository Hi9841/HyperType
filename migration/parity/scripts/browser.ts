/**
 * Launch a Chromium-family browser for reference captures.
 *
 * Prefer Edge (WebView2). Playwright launch() hangs on this host, so we start
 * Chrome ourselves and connect over CDP (Node runtime required for websockets).
 */
import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { chromium, type Browser } from "playwright-core";
import { sleep, start } from "./node-utils.ts";
import { PARITY_ROOT } from "./paths.ts";

export type Launched = {
  browser: Browser;
  channel: string;
  version: string;
  stop: () => void;
};

function edgePath(): string | undefined {
  const cands = [
    join(process.env.LOCALAPPDATA ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
    join(process.env["ProgramFiles"] ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
    join(process.env["ProgramFiles(x86)"] ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
  ];
  return cands.find((p) => existsSync(p));
}

function chromePath(): string | undefined {
  const cands = [
    join(process.env["ProgramFiles"] ?? "", "Google", "Chrome", "Application", "chrome.exe"),
    join(process.env["ProgramFiles(x86)"] ?? "", "Google", "Chrome", "Application", "chrome.exe"),
    join(process.env.LOCALAPPDATA ?? "", "Google", "Chrome", "Application", "chrome.exe"),
  ];
  return cands.find((p) => existsSync(p));
}

function bundledChromiumPath(): string | undefined {
  const root = join(process.env.LOCALAPPDATA ?? "", "ms-playwright");
  for (const v of ["chromium-1248", "chromium-1234"]) {
    const exe = join(root, v, "chrome-win64", "chrome.exe");
    if (existsSync(exe)) return exe;
  }
  return undefined;
}

async function waitForCdpHttp(port: number, deadlineMs: number): Promise<string> {
  const deadline = Date.now() + deadlineMs;
  let lastErr = "";
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (res.ok) {
        const json = (await res.json()) as { Browser?: string };
        console.log(`CDP ready: ${json.Browser ?? "unknown"}`);
        return `http://127.0.0.1:${port}`;
      }
      lastErr = `status ${res.status}`;
    } catch (err) {
      lastErr = err instanceof Error ? err.message : String(err);
    }
    await sleep(250);
  }
  throw new Error(`CDP on :${port} not ready (${lastErr})`);
}

async function connectViaCdp(
  exe: string,
  label: string,
  scale: number,
  extraArgs: string[] = [],
): Promise<Launched> {
  const port = 9333 + Math.floor(Math.random() * 400);
  const profile = join(PARITY_ROOT, ".browser-profile", label);
  mkdirSync(profile, { recursive: true });

  const args = [
    `--remote-debugging-port=${port}`,
    `--remote-debugging-address=127.0.0.1`,
    `--user-data-dir=${profile}`,
    "--headless=new",
    "--disable-gpu",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-background-networking",
    `--force-device-scale-factor=${scale}`,
    ...extraArgs,
    "about:blank",
  ];

  console.log(`Starting ${label} CDP on :${port}`);
  const child = start(exe, args, { stdout: "ignore", stderr: "ignore" });

  const stop = () => {
    try {
      child.kill();
    } catch {
      // already gone
    }
    if (child.pid) {
      start("taskkill", ["/PID", String(child.pid), "/T", "/F"]);
    }
  };

  try {
    const endpoint = await waitForCdpHttp(port, 20_000);
    const browser = await chromium.connectOverCDP(endpoint, { timeout: 20_000 });
    const version = browser.version();
    console.log(`Browser: ${label} ${version} (CDP http)`);
    return {
      browser,
      channel: label,
      version,
      stop: () => {
        void browser.close().catch(() => {});
        stop();
      },
    };
  } catch (err) {
    stop();
    throw err;
  }
}

export type LaunchOptions = {
  /** Extra Chromium flags (e.g. `--disable-lcd-text` for raster-noise case). */
  extraArgs?: string[];
  /** Prefer Chrome/Chromium over Edge (needed for some Chrome-only flags). */
  preferChrome?: boolean;
};

export async function launchParityBrowser(
  scale: number,
  opts: LaunchOptions = {},
): Promise<Launched> {
  const edge = edgePath();
  if (!edge) {
    console.warn(
      "Edge (msedge) is not installed. Captures use Chrome/Chromium CDP. WebView2 pixels may differ slightly.",
    );
  }

  const targets: Array<{ label: string; exe: string }> = [];
  if (opts.preferChrome) {
    const chrome = chromePath();
    if (chrome) targets.push({ label: "chrome-cdp", exe: chrome });
    const bundled = bundledChromiumPath();
    if (bundled) targets.push({ label: "chromium-cdp", exe: bundled });
    if (edge) targets.push({ label: "msedge-cdp", exe: edge });
  } else {
    if (edge) targets.push({ label: "msedge-cdp", exe: edge });
    const chrome = chromePath();
    if (chrome) targets.push({ label: "chrome-cdp", exe: chrome });
    const bundled = bundledChromiumPath();
    if (bundled) targets.push({ label: "chromium-cdp", exe: bundled });
  }

  let lastError: unknown;
  for (const target of targets) {
    try {
      return await connectViaCdp(
        target.exe,
        target.label,
        scale,
        opts.extraArgs ?? [],
      );
    } catch (err) {
      lastError = err;
      const msg = err instanceof Error ? err.message : String(err);
      console.warn(`${target.label} failed: ${msg.split("\n")[0]}`);
    }
  }

  throw lastError instanceof Error
    ? lastError
    : new Error("No browser could launch for parity captures");
}
