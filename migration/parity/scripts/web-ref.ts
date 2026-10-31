/**
 * The SolidJS reference UI. The web stack left the repo in v1.2.0, so the
 * judge builds the last web release (tag WEB_REF) in a detached worktree at
 * migration/parity/.web-ref and serves its `dist/`.
 */
import { existsSync } from "node:fs";
import { join } from "node:path";
import { run, runText } from "./node-utils.ts";
import { PARITY_ROOT, REPO_ROOT } from "./paths.ts";

/** Last release with the SolidJS UI; its `src/` is the reference. */
export const WEB_REF = "v1.1.8";
export const WEB_REF_DIR = join(PARITY_ROOT, ".web-ref");
export const WEB_DIST = join(WEB_REF_DIR, "dist");

function mustRun(command: string, args: string[], cwd: string): void {
  const proc = run(command, args, { cwd, inherit: true });
  if (proc.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed in ${cwd}`);
  }
}

/** Check out WEB_REF (once), install its deps, and build `dist/`. */
export function buildWebReference(): void {
  if (!existsSync(join(WEB_REF_DIR, "package.json"))) {
    console.log(`Checking out the ${WEB_REF} web UI into ${WEB_REF_DIR}...`);
    mustRun("git", ["worktree", "add", "--detach", WEB_REF_DIR, WEB_REF], REPO_ROOT);
  }
  if (!existsSync(join(WEB_REF_DIR, "node_modules"))) {
    mustRun("bun", ["install", "--frozen-lockfile"], WEB_REF_DIR);
  }
  console.log(`Building the ${WEB_REF} web UI (bun run build)...`);
  mustRun("bun", ["run", "build"], WEB_REF_DIR);
}

export function webRefCommit(): string {
  const head = runText("git", ["rev-parse", "HEAD"], { cwd: WEB_REF_DIR });
  return (head.stdout || "").trim() || "unknown";
}
