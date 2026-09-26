import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri 固定用 1420 端口；开发时浏览器直接打开也能看界面（后端调用走 mock）。
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true, chunkSizeWarningLimit: 800 },
});
