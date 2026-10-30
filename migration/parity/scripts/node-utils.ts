import { spawn, spawnSync, type SpawnSyncReturns } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

export function sleep(ms: number): Promise<void> {
  return delay(ms);
}

export function run(
  command: string,
  args: string[],
  opts: { cwd?: string; env?: NodeJS.ProcessEnv; inherit?: boolean } = {},
): SpawnSyncReturns<Buffer> {
  return spawnSync(command, args, {
    cwd: opts.cwd,
    env: opts.env ?? process.env,
    encoding: undefined,
    stdio: opts.inherit ? "inherit" : ["ignore", "pipe", "pipe"],
  });
}

export function runText(
  command: string,
  args: string[],
  opts: { cwd?: string; env?: NodeJS.ProcessEnv } = {},
): { exitCode: number | null; stdout: string; stderr: string } {
  const r = run(command, args, opts);
  return {
    exitCode: r.status,
    stdout: (r.stdout ?? Buffer.alloc(0)).toString("utf8"),
    stderr: (r.stderr ?? Buffer.alloc(0)).toString("utf8"),
  };
}

export function start(
  command: string,
  args: string[],
  opts: {
    cwd?: string;
    env?: NodeJS.ProcessEnv;
    stdout?: "pipe" | "ignore" | "inherit";
    stderr?: "pipe" | "ignore" | "inherit";
    /** Default false: GPUI parity windows must stay visible for PrintWindow. */
    windowsHide?: boolean;
  } = {},
) {
  return spawn(command, args, {
    cwd: opts.cwd,
    env: opts.env ?? process.env,
    stdio: [
      "ignore",
      opts.stdout ?? "ignore",
      opts.stderr ?? "ignore",
    ],
    windowsHide: opts.windowsHide ?? false,
  });
}
