import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../..", import.meta.url));

export default defineConfig({
  plugins: [svelte({ configFile: false })],
  build: {
    emptyOutDir: false,
    outDir: path.resolve(root, "src/web/server/static/assets"),
    rollupOptions: {
      input: {
        "passkey-login": path.resolve(root, "src/web/ui/src/passkey-login/main.js"),
      },
      output: {
        entryFileNames: "[name].js",
        chunkFileNames: "chunks/[name].js",
        assetFileNames: "assets/[name].[ext]",
      },
    },
  },
});
