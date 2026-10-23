// playwright.config.js
// E2E Web 轨（PR 必跑）：驱动 Vite 开发服务，验证「浏览器可运行的 UI 层」。
//
// 为什么不是 Tauri 原生窗：
//   Playwright 无法连接 tauri-driver（后者是 WebDriver 协议服务端），
//   原生窗行为（迷你窗尺寸/边缘吸附/托盘）需 WebdriverIO + tauri-driver，
//   属于 `tests/e2e/native`（tag/nightly 触发），见 docs/TESTING_PLAN.md §L5。
//
// 上一版配置的两处致命问题已修复：
//   1) 没有 baseURL 且 spec 内无 goto → 页面停在 about:blank，用例必然超时；
//   2) webServer 拉起 `tauri dev`（含 Rust 编译，3 分钟起步）→ 改用 `npm run serve`。
import { defineConfig, devices } from '@playwright/test';

const PORT = 8080;

export default defineConfig({
  testDir: './tests/e2e/web',
  testMatch: '**/*.spec.js',
  timeout: 45000,
  expect: { timeout: 15000 },
  fullyParallel: false, // 单实例应用，避免并发争抢同一 dev server
  workers: 1,
  reporter: process.env.CI
    ? [['github'], ['html', { outputFolder: 'playwright-report' }]]
    : 'list',
  use: {
    baseURL: `http://localhost:${PORT}`,
    actionTimeout: 10000,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'web', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'npm run serve',
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
