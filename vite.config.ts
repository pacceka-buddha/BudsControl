import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import { resolve } from 'node:path'

// Tauri expects a fixed dev-server port and serves the built pages from ./dist.
// One HTML entry per window: the tray flyout, the settings window and the shortcut HUD.
const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**', '**/target/**', '**/reference/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    // WebView2 (Chromium), WKWebView (Safari 16+) and WebKitGTK all handle ES2022.
    target: 'es2022',
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rolldownOptions: {
      input: {
        flyout: resolve(import.meta.dirname, 'flyout.html'),
        settings: resolve(import.meta.dirname, 'settings.html'),
        hud: resolve(import.meta.dirname, 'hud.html'),
      },
    },
  },
})
