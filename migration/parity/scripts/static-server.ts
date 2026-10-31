/** Tiny static file server for the reference web UI `dist/` on :1420. */
import { createServer } from "node:http";
import { readFileSync, existsSync, statSync } from "node:fs";
import { join, extname } from "node:path";
import { WEB_DIST } from "./web-ref.ts";

const ROOT = WEB_DIST;
const PORT = 1420;

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".json": "application/json",
  ".ico": "image/x-icon",
};

function contentType(file: string): string {
  return TYPES[extname(file)] ?? "application/octet-stream";
}

async function portAlreadyServing(): Promise<boolean> {
  try {
    const res = await fetch(`http://127.0.0.1:${PORT}/`);
    return res.ok || res.status === 404;
  } catch {
    return false;
  }
}

export async function startStaticServer(): Promise<() => void> {
  if (await portAlreadyServing()) {
    console.log(`Reusing server on :${PORT}`);
    return () => {};
  }

  if (!existsSync(join(ROOT, "index.html"))) {
    throw new Error(`Missing ${join(ROOT, "index.html")}; run bun run build in the repo root`);
  }

  const server = createServer((req, res) => {
    try {
      const url = new URL(req.url ?? "/", `http://127.0.0.1:${PORT}`);
      let rel = decodeURIComponent(url.pathname);
      if (rel === "/") rel = "/index.html";
      const file = join(ROOT, rel.replace(/^\//, ""));
      if (!file.startsWith(ROOT) || !existsSync(file) || statSync(file).isDirectory()) {
        res.writeHead(404);
        res.end("not found");
        return;
      }
      const body = readFileSync(file);
      res.writeHead(200, { "Content-Type": contentType(file) });
      res.end(body);
    } catch (err) {
      res.writeHead(500);
      res.end(String(err));
    }
  });

  try {
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(PORT, "127.0.0.1", () => resolve());
    });
  } catch (err) {
    const code =
      err && typeof err === "object" && "code" in err
        ? String((err as { code: unknown }).code)
        : "";
    if (code === "EADDRINUSE" && (await portAlreadyServing())) {
      console.log(`Reusing server on :${PORT} (listen raced)`);
      return () => {};
    }
    throw err;
  }
  console.log(`Static dist server on http://127.0.0.1:${PORT}`);
  return () => {
    server.close();
  };
}

if (
  process.argv[1] !== undefined &&
  (process.argv[1].endsWith("static-server.ts") ||
    process.argv[1].endsWith("static-server"))
) {
  const stop = await startStaticServer();
  console.log("Press Ctrl+C to stop");
  process.on("SIGINT", () => {
    stop();
    process.exit(0);
  });
}
