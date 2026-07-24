import path from "path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// The Axum API server. Override with ZYNC_API_URL when it runs elsewhere.
const API_TARGET = process.env.ZYNC_API_URL ?? "http://127.0.0.1:58271"

// Backend route prefixes proxied to the Axum server in dev. Everything else is
// served by Vite (the React app + its assets). Keep in sync with the route
// modules merged in crates/server/src/main.rs. Note: `files::routes()` and
// `collaboration::routes()` only register nested `/workspace/:id/...` paths
// (no top-level `/files` or `/collaboration` route exists), so those two
// modules ride on the `/workspace` prefix below rather than needing their own.
const API_PREFIXES = [
  "/repositories",
  "/workspace",
  "/auth",
  "/credentials",
  "/directories",
  "/health",
]

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      // WebSocket workspace stream.
      "/ws": { target: API_TARGET, ws: true, changeOrigin: true },
      ...Object.fromEntries(
        API_PREFIXES.map((prefix) => [
          prefix,
          { target: API_TARGET, changeOrigin: true },
        ]),
      ),
    },
  },
})
