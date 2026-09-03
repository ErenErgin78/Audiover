import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  base: "./",
  plugins: [react(), tailwindcss()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    // Pinned literal IPv4 endpoint matching tauri.conf.json `devUrl`.
    // `localhost` is unsafe here: on this machine it resolves to ::1 first,
    // so the dev server and the WebView can end up on different loopback
    // families (IPv6 vs IPv4) -> "Connection refused".
    host: "127.0.0.1",
    port: 5173,
    // Fail loudly on port conflicts instead of silently moving to 5174
    // while Tauri keeps polling 5173 (same "Connection refused" symptom).
    strictPort: true,
  },
});
