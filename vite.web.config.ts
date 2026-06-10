import { defineConfig, mergeConfig, type UserConfig } from "vite";
import path from "node:path";

import baseConfig from "./vite.config";

// Web (browser) build configuration.
//
// This extends the shared `vite.config.ts` and layers on the browser-only
// concerns, so the desktop config stays the single source of truth for
// plugins, chunking, etc. (keeps merge conflicts with upstream minimal):
//   1. Alias the Tauri API modules to the `fetch`-based web shims in `src/web`,
//      which is what lets the unchanged frontend talk to the web server.
//   2. Proxy `/api` to the local `ofm_server` during `vite dev`.
const webShimAliases = {
  "@tauri-apps/api/core": path.resolve(__dirname, "src/web/tauriCore.ts"),
  "@tauri-apps/api/window": path.resolve(__dirname, "src/web/tauriWindow.ts"),
};

const apiTarget = process.env.OFM_WEB_API ?? "http://localhost:8080";

export default defineConfig(async (env) => {
  const resolvedBase =
    typeof baseConfig === "function" ? await baseConfig(env) : baseConfig;

  const webOverrides: UserConfig = {
    resolve: {
      alias: webShimAliases,
    },
    server: {
      port: 1430,
      strictPort: false,
      proxy: {
        "/api": {
          target: apiTarget,
          changeOrigin: true,
        },
      },
    },
  };

  return mergeConfig(resolvedBase, webOverrides);
});
