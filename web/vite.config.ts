import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// The dashboard is served by `altium serve` on 127.0.0.1:7420. In dev, Vite
// proxies API calls to it so the front can be iterated on with hot reload.
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:7420',
    },
  },
})
