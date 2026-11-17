/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port in dev and no screen clearing.
export default defineConfig({
  plugins: [solid(), tailwindcss()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", sourcemap: false },
  // Unit tests cover pure modules; component tests add jsdom when they arrive.
  test: { environment: "node" },
});
