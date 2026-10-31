/**
 * One-shot parity run: build with --features parity, launch per state,
 * capture client area (stable frame), kill the process, then dual-metric diff.
 */
import { mkdirSync, existsSync, cpSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  DIFF_DIR,
  GPUI_DIR,
  PARITY_ROOT,
  REPO_ROOT,
  SEED_APPDATA,
} from "./paths.ts";
import { diffPair, formatSummary, parseRegion } from "./diff.ts";
import { run, runText, sleep, start } from "./node-utils.ts";

function argValue(flag: string): string | undefined {
  const idx = process.argv.indexOf(flag);
  if (idx < 0) return undefined;
  return process.argv[idx + 1];
}

function statesFromArgs(): string[] {
  const dash = process.argv.indexOf("--");
  const raw = dash >= 0 ? process.argv.slice(dash + 1) : process.argv.slice(2);
  const skipNext = new Set<number>();
  for (let i = 0; i < raw.length; i++) {
    if (raw[i] === "--region" || raw[i] === "--ref-dir" || raw[i] === "--cmp-dir" || raw[i] === "--out-dir") {
      skipNext.add(i + 1);
    }
  }
  const states = raw.filter(
    (a, i) => !a.startsWith("-") && !skipNext.has(i),
  );
  return states.length > 0 ? states : ["default-selected"];
}

function buildParity(): void {
  console.log("cargo build -p hypertype --features parity");
  const proc = run(
    "cargo",
    ["build", "-p", "hypertype", "--features", "parity"],
    { cwd: REPO_ROOT, inherit: true },
  );
  if (proc.status !== 0) throw new Error("cargo build failed");
}

function exePath(): string {
  const debug = join(REPO_ROOT, "target", "debug", "hypertype.exe");
  if (!existsSync(debug)) throw new Error(`Missing binary: ${debug}`);
  return debug;
}

function prepareAppData(): string {
  const tmp = join(PARITY_ROOT, ".run-appdata");
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(tmp, { recursive: true });
  cpSync(join(SEED_APPDATA, "HyperType"), join(tmp, "HyperType"), {
    recursive: true,
  });
  return tmp;
}

async function waitForWindow(pid: number, timeoutMs = 20_000): Promise<void> {
  const script = join(PARITY_ROOT, "scripts", "capture-window.ps1");
  const probe = join(GPUI_DIR, "_probe.png");
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const { exitCode } = runText("powershell", [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      script,
      "-ProcessId",
      String(pid),
      "-Title",
      "HyperType",
      "-OutPath",
      probe,
    ]);
    if (exitCode === 0) {
      rmSync(probe, { force: true });
      return;
    }
    await sleep(300);
  }
  throw new Error(`Timed out waiting for HyperType window from PID ${pid}`);
}

async function runState(
  state: string,
  exe: string,
  appdata: string,
): Promise<void> {
  console.log(`\n=== parity state: ${state} ===`);
  const child = start(exe, ["--parity-state", state], {
    cwd: REPO_ROOT,
    env: { ...process.env, APPDATA: appdata },
    stdout: "inherit",
    stderr: "inherit",
  });

  const pid = child.pid;
  if (!pid) {
    child.kill();
    throw new Error("Failed to start parity process");
  }
  console.log(`Started PID ${pid}`);

  try {
    await waitForWindow(pid);
    // Stable capture (two consecutive equal frames) lives in capture-gpui.ts.
    const out = join(GPUI_DIR, `${state}.png`);
    mkdirSync(GPUI_DIR, { recursive: true });
    const capture = run(
      "node",
      [
        "--experimental-strip-types",
        "scripts/capture-gpui.ts",
        "--pid",
        String(pid),
        "--state",
        state,
        "--out",
        out,
      ],
      { cwd: PARITY_ROOT, inherit: true },
    );
    if (capture.status !== 0) throw new Error(`capture failed for ${state}`);
  } finally {
    try {
      child.kill();
    } catch {
      // already exited
    }
    run("taskkill", ["/PID", String(pid), "/T", "/F"]);
    await sleep(200);
  }
}

async function main() {
  const states = statesFromArgs();
  const region = parseRegion(argValue("--region"));
  mkdirSync(GPUI_DIR, { recursive: true });
  mkdirSync(DIFF_DIR, { recursive: true });

  buildParity();
  const exe = exePath();

  const results = [];
  for (const state of states) {
    // Fresh seed per state: some states mutate data (empty-library deletes
    // every snippet), which would leak into the states after it.
    const appdata = prepareAppData();
    await runState(state, exe, appdata);
    const outName = region
      ? `${state}-r${region.x}_${region.y}_${region.w}_${region.h}.png`
      : `${state}.png`;
    const result = diffPair(
      join(PARITY_ROOT, "reference", `${state}.png`),
      join(GPUI_DIR, `${state}.png`),
      join(DIFF_DIR, outName),
      undefined,
      region,
    );
    results.push(result);
    const regionLabel = region
      ? ` region ${region.x},${region.y},${region.w},${region.h}`
      : "";
    console.log(
      `diff ${state}${regionLabel}: detail ${result.percent.toFixed(3)}% fill ${result.fillPercent.toFixed(3)}% ${result.pass ? "PASS" : "FAIL"}` +
        (result.error ? ` (${result.error})` : ""),
    );
  }

  const summary = formatSummary(results);
  const summaryPath = join(DIFF_DIR, "summary.md");
  writeFileSync(summaryPath, summary + "\n");
  console.log("\n" + summary);
  console.log(`Wrote ${summaryPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
