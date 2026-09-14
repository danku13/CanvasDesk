import { defineConfig } from "vite";

export default defineConfig({
  base: "./",
  build: { outDir: "dist-widget", emptyOutDir: true },
});
