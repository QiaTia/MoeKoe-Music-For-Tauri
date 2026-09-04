// vitest.config.js
// 前端单元测试配置。独立于 vite.config.js（后者挂了 PWA 插件，测试时无需）。
import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath, URL } from 'node:url';

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    include: ['tests/unit/**/*.{test,spec}.js'],
    exclude: ['tests/e2e/**', 'node_modules/**', 'dist/**'],
    setupFiles: [],
    // ── 覆盖率配置 ──
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html', 'lcov', 'clover'],
      reportsDirectory: './coverage',
      // 仅统计源码，排除入口/路由/构建产物
      include: ['src/**/*.{js,vue}'],
      exclude: [
        'src/main.js',
        'src/router/**',
        'src/**/*.d.ts',
        '**/node_modules/**',
        '**/dist/**',
      ],
      thresholds: {
        // 全局覆盖率目标（CI 未达标将非零退出）
        statements: 60,
        branches: 50,
        functions: 60,
        lines: 60,
      },
    },
  },
});
