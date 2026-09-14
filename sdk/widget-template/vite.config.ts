import { defineConfig } from "vite";

// Ключевое для пакета виджета:
//  - base "./" — относительные пути к ассетам: пакет разворачивается в
//    виртуальный origin (SetVirtualHostNameToFolderMapping), никаких "/";
//  - outDir "dist-widget" — «npm run pack» выдаёт готовую к drag-установке
//    папку (widget.json из public/ попадает в корень вывода).
export default defineConfig({
  base: "./",
  build: { outDir: "dist-widget", emptyOutDir: true },
});
