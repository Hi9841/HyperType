/**
 * Capture the client area of a HyperType GPUI window by title + PID.
 *
 * Waits until two consecutive captures are byte-identical (window stopped
 * changing), with a timeout. Avoids fixed sleeps that can catch mid-animation.
 *
 * Usage:
 *   node --experimental-strip-types scripts/capture-gpui.ts --pid 12345 --state default-selected
 */
import { mkdirSync, readFileSync, writeFileSync, unlinkSync, existsSync } from "node:fs";
import { join } from "node:path";
import { runText, sleep } from "./node-utils.ts";
import { GPUI_DIR, PARITY_ROOT } from "./paths.ts";

function argValue(flag: string): string | undefined {
  const idx = process.argv.indexOf(flag);
  if (idx < 0) return undefined;
  return process.argv[idx + 1];
}

function captureOnce(
  pid: number,
  title: string,
  outPath: string,
): { ok: boolean; stdout: string; stderr: string; exitCode: number | null } {
  const script = join(PARITY_ROOT, "scripts", "capture-window.ps1");
  const { exitCode, stdout, stderr } = runText("powershell", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    script,
    "-ProcessId",
    String(pid),
    "-Title",
    title,
    "-OutPath",
    outPath,
  ]);
  return { ok: exitCode === 0, stdout, stderr, exitCode };
}

function buffersEqual(a: Buffer, b: Buffer): boolean {
  if (a.length !== b.length) return false;
  return a.equals(b);
}

/**
 * Capture until two consecutive frames match, or timeout.
 * Returns the final path written.
 */
export async function captureStable(
  pid: number,
  title: string,
  outPath: string,
  timeoutMs = 8_000,
): Promise<void> {
  mkdirSync(join(outPath, ".."), { recursive: true });
  const tmpA = outPath + ".a.png";
  const tmpB = outPath + ".b.png";
  const deadline = Date.now() + timeoutMs;
  let lastErr = "";
  let prev: Buffer | undefined;
  let useA = true;

  while (Date.now() < deadline) {
    const path = useA ? tmpA : tmpB;
    const result = captureOnce(pid, title, path);
    if (result.stdout.trim()) console.log(result.stdout.trim());
    if (!result.ok) {
      lastErr = result.stderr.trim() || `exit ${result.exitCode}`;
      await sleep(250);
      continue;
    }
    const buf = readFileSync(path);
    if (prev && buffersEqual(prev, buf)) {
      writeFileSync(outPath, buf);
      for (const p of [tmpA, tmpB]) {
        if (existsSync(p)) unlinkSync(p);
      }
      return;
    }
    prev = buf;
    useA = !useA;
    await sleep(100);
  }

  // Timeout: keep the last frame if we have one.
  if (prev) {
    writeFileSync(outPath, prev);
    for (const p of [tmpA, tmpB]) {
      if (existsSync(p)) unlinkSync(p);
    }
    console.warn(
      `GPUI capture timed out waiting for a stable frame; kept last capture (${lastErr || "no match"})`,
    );
    return;
  }
  for (const p of [tmpA, tmpB]) {
    if (existsSync(p)) unlinkSync(p);
  }
  throw new Error(
    `capture-window.ps1 failed before any frame: ${lastErr || "unknown"}`,
  );
}

async function main() {
  const pid = Number.parseInt(argValue("--pid") ?? "", 10);
  const state = argValue("--state") ?? "default-selected";
  const title = argValue("--title") ?? "HyperType";
  const out = argValue("--out") ?? join(GPUI_DIR, `${state}.png`);

  if (!Number.isFinite(pid) || pid <= 0) {
    throw new Error(
      "Pass --pid <number> (the parity process PID, never the installed app)",
    );
  }

  await captureStable(pid, title, out);
  console.log(`GPUI capture: ${out}`);
}

const isEntry =
  process.argv[1] !== undefined &&
  (process.argv[1].endsWith("capture-gpui.ts") ||
    process.argv[1].endsWith("capture-gpui"));
if (isEntry) {
  try {
    await main();
  } catch (err) {
    console.error(err);
    process.exit(1);
  }
}
