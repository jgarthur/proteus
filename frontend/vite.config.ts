import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Vitest reads this file too. It is deliberately left without a `test` block:
// importing `defineConfig` from `vitest/config` would pull in vitest's bundled
// (rollup-based) vite types alongside this project's rolldown-based vite 8 and
// break `tsc -b`. The default `node` environment is what the pure `src/lib`
// suites want anyway, and component tests opt into jsdom per file with a
// `// @vitest-environment jsdom` docblock.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
  },
});
