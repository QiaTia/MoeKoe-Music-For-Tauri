// playwright.config.js
// 端到端测试：针对 Tauri 桌面应用（WebView2/WKWebView）的 UI 流程验证。
// 与常规 Web 测试不同，Tauri 应用不暴露 http 服务可供 goto，而是：
//  1) 由 `webServer` 拉起 `tauri dev`（同时起 Vite dev server + 原生窗口）；
//  2) spec 通过 `window.__TAURI_INTERNALS__` 探测 WebView 就绪后，直接断言真实 DOM。
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: '**/*.spec.js',
  timeout: 60000,
  expect: { timeout: 15000 },
  fullyParallel: false, // 单实例应用：并行会触发 second-instance 逻辑
  workers: 1,
  reporter: process.env.CI ? [['github'], ['html', { outputFolder: 'playwright-report' }]] : 'list',
  use: {
    actionTimeout: 10000,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    // Tauri 应用需原生窗口，CI 中通过 xvfb-run 提供虚拟显示。
    launchOptions: process.env.CI ? { args: ['--no-sandbox'] } : undefined,
  },
  // `tauri dev` 启动较慢（含 Rust 编译与前端构建），给予充裕超时。
  webServer: {
    command: 'npm run tauri dev',
    url: 'http://localhost:8080',
    reuseExistingServer: !process.env.CI,
    timeout: 300000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
