import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// Fixed dev port so Tauri's devUrl (http://localhost:1420) lines up.
export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "esnext",
    outDir: "dist",
    emptyOutDir: true,
  },
});
