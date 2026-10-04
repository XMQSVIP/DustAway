import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: [
        '**/target/**',
        '**/artifacts/**',
        '**/licenses/**',
        '**/src-tauri/**',
        '**/crates/**',
      ],
    },
  },
  clearScreen: false,
  test: {
    include: ['src/test/**/*.test.{ts,tsx}'],
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    css: false,
    maxWorkers: 1,
    // Windows fork workers intermittently fail their startup handshake; retain isolation
    // with a single thread worker, validated by the complete desktop UI suite.
    pool: 'threads',
    testTimeout: 15000,
  },
});
