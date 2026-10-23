// tests/setup/vitest.setup.js
// 全局测试环境准备：补齐 jsdom 未实现的浏览器 API，并提供全局 UI 替身。
// 原则：只补「缺失的能力」，不改业务语义；任何模块级 mock 仍在各自测试文件内声明。

// ── HTMLMediaElement：jsdom 不实现播放控制，AudioController 一调用就会抛 ──
// 用 defineProperty 覆盖原型方法，避免每个测试各自造 audio 对象。
Object.defineProperty(HTMLMediaElement.prototype, 'play', {
  configurable: true,
  writable: true,
  value: vi.fn(() => Promise.resolve()),
});
Object.defineProperty(HTMLMediaElement.prototype, 'pause', {
  configurable: true,
  writable: true,
  value: vi.fn(),
});
Object.defineProperty(HTMLMediaElement.prototype, 'load', {
  configurable: true,
  writable: true,
  value: vi.fn(),
});
Object.defineProperty(HTMLMediaElement.prototype, 'currentTime', {
  configurable: true,
  writable: true,
  value: 0,
});

// ── 布局与媒体查询 ──
if (!globalThis.matchMedia) {
  globalThis.matchMedia = (query) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  });
}

if (!globalThis.ResizeObserver) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

if (!globalThis.IntersectionObserver) {
  globalThis.IntersectionObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
    takeRecords() { return []; }
  };
}

// ── 剪贴板（teamEvent.copyTeamCode 依赖） ──
if (!navigator.clipboard) {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn(() => Promise.resolve()) },
  });
}

// ── 全局 UI 方法替身 ──
// 业务代码直接调用 $message / $modal（由插件在 main.js 注入），
// 单测不装插件，因此在此提供可断言的替身。
const uiStub = () => ({
  message: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  info: vi.fn(),
  alert: vi.fn(() => Promise.resolve()),
  confirm: vi.fn(() => Promise.resolve(true)),
  prompt: vi.fn(() => Promise.resolve('')),
  showLoading: vi.fn(),
  hideLoading: vi.fn(),
});

beforeEach(() => {
  const stub = uiStub();
  globalThis.$message = stub;
  window.$message = stub;
  globalThis.$modal = stub;
  window.$modal = stub;
});

// ── 用例间状态隔离 ──
afterEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  document.body.innerHTML = '';
  document.body.className = '';
  vi.clearAllMocks();
});
