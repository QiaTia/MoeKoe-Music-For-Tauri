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
    include: [
      'tests/unit/**/*.{test,spec}.js',
      'tests/component/**/*.{test,spec}.js',
      'tests/contract/**/*.{test,spec}.js',
    ],
    exclude: ['tests/e2e/**', 'node_modules/**', 'dist/**'],
    setupFiles: ['./tests/setup/vitest.setup.js'],
    // ── 覆盖率配置 ──
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html', 'lcov', 'clover'],
      reportsDirectory: './coverage',
      // 分层管理分母：只统计「已有测试责任」的目录。
      // src/views/** 与超大组件（PlayerControl 等）在完成逻辑抽取前不入分母，
      // 这是显式的技术债登记，而非回避 —— 详见 docs/TESTING_PLAN.md §3。
      include: [
        'src/utils/**/*.js',
        'src/stores/**/*.js',
        'src/composables/**/*.js',
        'src/plugins/**/*.js',
      ],
      exclude: [
        'src/**/*.d.ts',
        'src/utils/i18n.js', // 纯配置装配，无逻辑分支
        '**/node_modules/**',
        '**/dist/**',
      ],
      thresholds: {
        // 全局覆盖率目标（CI 未达标将非零退出）
        // 2026-10 实测基线：stmts/lines 95.51、branch 86.65、funcs 92.48
        // 门槛取「实测 -5」留出缓冲，避免偶发波动误伤；新增目录拉低时再评估。
        statements: 90,
        branches: 80,
        functions: 85,
        lines: 90,
      },
    },
  },
});
