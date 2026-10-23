// tests/e2e/web/app-shell.spec.js
// 应用外壳冒烟：启动渲染、路由导航、无代码级异常。
//
// 设计约束：
//  - 不依赖真实酷狗后端：拦截 127.0.0.1:6521 的请求并返回成功信封，
//    否则每个请求 ERR_CONNECTION_REFUSED 会污染 pageerror 断言。
//  - 只断言「外壳 + 路由 + 无 JS 崩溃」，不涉及在线数据内容，
//    这样用例不受上游接口波动影响（那是契约测试的职责）。
//  - 定位一律用 href / role，不绑死界面文案：i18n 语言切换不应让 E2E 变红。
//  - 页面内存在两个 id="app"（根节点与 Home 视图），必须用 .first() 消除 strict 冲突。
import { test, expect } from '@playwright/test';

const API_HOST = '127.0.0.1:6521';
const APP_ROOT = '#app';

/** 收集页面未捕获异常，并过滤掉纯网络类噪声（后端/外网不可达） */
function collectCodeErrors(page) {
  const errors = [];
  page.on('pageerror', (err) => errors.push({ message: err.message, stack: err.stack }));
  return {
    codeErrors: () =>
      errors.filter(
        (e) => !/network|failed to fetch|err_connection|aborted/i.test(e.message)
      ),
  };
}

test.beforeEach(async ({ page }) => {
  // 进程内 API 的 HTTP 等价出口（Web 模式走 axios）→ 统一返回成功信封
  await page.route('**/*', async (route) => {
    const url = route.request().url();
    if (url.includes(API_HOST)) {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ status: 1, data: {} }),
      });
    }
    return route.continue();
  });
});

/**
 * 首次启动会弹出《用户条款》遮罩，未处理会拦截所有点击。
 * 按钮顺序固定：第一个是「同意」（第二个是「不同意」），以此定位可避免绑死 i18n 文案。
 */
async function dismissDisclaimer(page) {
  const overlay = page.locator('.modal-overlay');
  if (await overlay.count()) {
    await overlay.first().locator('.button-group button').first().click();
    await expect(overlay).toHaveCount(0);
  }
}

/** 打开应用并进入可交互状态 */
async function openApp(page, hash = '/') {
  await page.goto(hash);
  await page.locator(APP_ROOT).first().waitFor();
  await dismissDisclaimer(page);
}

test('应用启动后渲染主界面外壳与主导航', async ({ page }) => {
  await openApp(page);

  await expect(page).toHaveTitle(/萌音/);
  const root = page.locator(APP_ROOT).first();
  await expect(root).toBeVisible();
  await expect(root).not.toBeEmpty();

  // 主导航按 hash 路由渲染（不依赖语言文案）
  for (const href of ['#/', '#/discover', '#/library']) {
    await expect(page.locator(`a[href="${href}"]`).first()).toBeVisible();
  }
});

test('点击主导航可切换到发现页', async ({ page }) => {
  await openApp(page);
  await page.locator('a[href="#/discover"]').first().click();

  await expect(page).toHaveURL(/#\/discover/);
  await expect(page.locator(APP_ROOT).first()).not.toBeEmpty();
});

test('直接访问 hash 路由可渲染对应页面', async ({ page }) => {
  await openApp(page, '/#/ranking');

  await expect(page).toHaveURL(/#\/ranking/);
  await expect(page.locator(APP_ROOT).first()).not.toBeEmpty();
});

test('启动过程无代码级异常（TypeError / ReferenceError 等）', async ({ page }) => {
  const { codeErrors } = collectCodeErrors(page);

  await openApp(page);
  await page.waitForTimeout(2500); // 给 mounted 钩子与首个渲染周期留出时间

  expect(codeErrors().map((e) => e.stack)).toEqual([]);
});

test('从首页推荐卡片进入歌单详情', async ({ page }) => {
  await openApp(page);

  const card = page.locator('a[href*="PlaylistDetail"]').first();
  await expect(card).toBeVisible();
  // 推荐卡片带持续浮动动画，Playwright 的「元素稳定」检查永远不会满足
  await card.click({ force: true });

  await expect(page).toHaveURL(/#\/PlaylistDetail\?global_collection_id=/);
  await expect(page.locator(APP_ROOT).first()).not.toBeEmpty();
});

test('歌单详情在空数据下不白屏且无代码级异常', async ({ page }) => {
  const { codeErrors } = collectCodeErrors(page);

  // 上游无数据（route mock 返回空 data）时，详情页应渲染空列表而不是崩溃
  await openApp(page, '/#/PlaylistDetail?global_collection_id=collection_3_25230245_24_0');

  await expect(page.locator('.detail-page')).toBeVisible();
  await expect(page.locator(APP_ROOT).first()).not.toBeEmpty();
  expect(codeErrors().map((e) => e.stack)).toEqual([]);
});
