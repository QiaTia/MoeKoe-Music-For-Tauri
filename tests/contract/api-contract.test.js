// tests/contract/api-contract.test.js
// 前后端契约测试：纯静态扫描，不联网、不启动应用。
//
// 为什么需要它：本项目前端通过 `invoke('api_request', { path })` 直调 Rust，
// 路径或通道名写错不会编译失败、不会抛错 —— Rust 侧返回 404 或静默丢弃，
// 表现为「功能点了没反应」，线上极难定位。这类错误必须由契约测试兜住。
//
// 设计要点（防止测试本身失效）：
//  1. 先断言「扫描到的条目数」在合理区间 —— 否则正则失配会让测试变成永远通过的空壳；
//  2. 已知缺口放在白名单里并附原因，而不是直接过滤掉；
//  3. 白名单做双向校验：既不能有新增缺口，也不能有「已修复却仍挂在白名单」的僵尸条目。
import fs from 'node:fs';
import path from 'node:path';

// jsdom 环境下 import.meta.url 不是 file: scheme（会被 Vite 改写），
// 因此以 cwd 为根；下方自检会在根目录识别失败时立即报错，不会静默扫空。
const ROOT = process.cwd();

/** 已知缺口：前端调用了，但 Rust 侧未实现。修复后必须从本表移除（否则测试会提醒）。 */
const KNOWN_MISSING_ROUTES = {
  '/user/cloud/del':
    'Tauri 版未实现云盘删除（Electron 版有）。dispatch 对未知路由返回 404，云盘删除不可用。',
  '/user/cloud/upload':
    'Tauri 版未实现云盘上传。注意：即便补 Rust 实现，invoke 通道也无法序列化二进制 body，需独立 command + sign_binary。',
};

/** 已知无处理通道：前端发出但 Rust 无对应分支，功能已有等价实现。 */
const KNOWN_UNHANDLED_CHANNELS = {
  'current-song-updated':
    'Electron 遗留的无参通知。Tauri 侧歌曲信息由 player-meta → ws.update_current_song 覆盖，可直接删除该 send。',
};

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.(js|vue)$/.test(entry.name)) out.push(full);
  }
  return out;
}

function readAll(dir) {
  return walk(path.join(ROOT, dir)).map((file) => ({
    file: path.relative(ROOT, file),
    text: fs.readFileSync(file, 'utf8'),
  }));
}

/** 归一化：去掉 query、去掉尾部斜杠，保留根路径 */
function normalizeRoute(raw) {
  const noQuery = raw.split('?')[0];
  return noQuery.replace(/\/+$/, '') || '/';
}

// ── 提取器 ────────────────────────────────────────────────

function extractRustRoutes() {
  const src = fs.readFileSync(path.join(ROOT, 'rust-api/src/modules.rs'), 'utf8');
  const routes = new Map();
  for (const m of src.matchAll(/route:\s*"([^"]+)"/g)) {
    routes.set(normalizeRoute(m[1]), m[1]);
  }
  return routes;
}

function extractFrontendRoutes() {
  // 前置断言排除 `axios.get(` / `map.get(` 等方法调用
  const callRe = /(?<![.\w$])(get|post|put|del|patch)\(\s*[`'"]([^`'"]+)[`'"]/g;
  const routes = new Map();
  for (const { file, text } of readAll('src')) {
    for (const m of text.matchAll(callRe)) {
      if (!m[2].startsWith('/')) continue;
      const route = normalizeRoute(m[2]);
      if (!routes.has(route)) routes.set(route, new Set());
      routes.get(route).add(file);
    }
  }
  return routes;
}

function extractRustChannels() {
  const src = fs.readFileSync(path.join(ROOT, 'src-tauri/src/ipc.rs'), 'utf8');
  const channels = new Set();
  // 只取顶层 match 分支（8 空格缩进），嵌套 match 的内部动作（如 "close"）不算通道
  for (const m of src.matchAll(/^ {8}"([^"]+)"\s*=>/gm)) channels.add(m[1]);
  return channels;
}

function extractFrontendChannels() {
  const sendRe = /ipcRenderer\??\.(?:send|invoke)\(\s*['"]([^'"]+)['"]/g;
  const send = new Map();
  for (const { file, text } of readAll('src')) {
    for (const m of text.matchAll(sendRe)) {
      if (!send.has(m[1])) send.set(m[1], new Set());
      send.get(m[1]).add(file);
    }
  }
  return send;
}

// ── 用例 ──────────────────────────────────────────────────

const rustRoutes = extractRustRoutes();
const frontendRoutes = extractFrontendRoutes();
const rustChannels = extractRustChannels();
const frontendChannels = extractFrontendChannels();

describe('解析自检（防止正则失配导致假绿）', () => {
  it('工作目录正确，能定位到关键源文件', () => {
    expect(fs.existsSync(path.join(ROOT, 'rust-api/src/modules.rs'))).toBe(true);
    expect(fs.existsSync(path.join(ROOT, 'src-tauri/src/ipc.rs'))).toBe(true);
    expect(fs.existsSync(path.join(ROOT, 'src/main.js'))).toBe(true);
  });

  it('能解析出 Rust 路由注册表', () => {
    // 当前 160 个；低于 150 说明提取逻辑已与实际代码脱节
    expect(rustRoutes.size).toBeGreaterThanOrEqual(150);
  });

  it('能解析出前端 API 调用', () => {
    expect(frontendRoutes.size).toBeGreaterThanOrEqual(40);
  });

  it('能解析出 Rust IPC 通道', () => {
    expect(rustChannels.size).toBeGreaterThanOrEqual(20);
  });

  it('能解析出前端 IPC 发送', () => {
    expect(frontendChannels.size).toBeGreaterThanOrEqual(20);
  });
});

describe('API 路由契约', () => {
  it('Rust 路由格式规范：以 / 开头、无 query、无尾部斜杠', () => {
    const bad = [];
    for (const [route, raw] of rustRoutes) {
      if (!route.startsWith('/')) bad.push(raw);
      if (route.includes('?')) bad.push(raw);
      if (route.length > 1 && route.endsWith('/')) bad.push(raw);
    }
    expect(bad).toEqual([]);
  });

  it('前端引用的接口必须已在 Rust 注册（已知缺口除外）', () => {
    const unexpected = [...frontendRoutes.keys()]
      .filter((route) => !rustRoutes.has(route))
      .filter((route) => !(route in KNOWN_MISSING_ROUTES))
      .sort()
      .map((route) => `${route}  ← ${[...frontendRoutes.get(route)].join(', ')}`);

    expect(unexpected).toEqual([]);
  });

  it('已知缺口清单未过期：每条仍被前端引用', () => {
    const stale = Object.keys(KNOWN_MISSING_ROUTES).filter(
      (route) => !frontendRoutes.has(route)
    );
    expect(stale).toEqual([]);
  });

  it('已知缺口未被静默修复：若后端已实现需移除白名单', () => {
    const fixed = Object.keys(KNOWN_MISSING_ROUTES).filter((route) => rustRoutes.has(route));
    expect(fixed).toEqual([]);
  });
});

describe('IPC 通道契约', () => {
  it('前端发出的通道必须由 Rust 处理（已知遗留除外）', () => {
    const unexpected = [...frontendChannels.keys()]
      .filter((channel) => !rustChannels.has(channel))
      .filter((channel) => !(channel in KNOWN_UNHANDLED_CHANNELS))
      .sort()
      .map((channel) => `${channel}  ← ${[...frontendChannels.get(channel)].join(', ')}`);

    expect(unexpected).toEqual([]);
  });

  it('已知无处理通道清单未过期', () => {
    const stale = Object.keys(KNOWN_UNHANDLED_CHANNELS).filter(
      (channel) => !frontendChannels.has(channel)
    );
    expect(stale).toEqual([]);
  });

  it('已知无处理通道未被静默补上（若 Rust 已处理需移除白名单）', () => {
    const handled = Object.keys(KNOWN_UNHANDLED_CHANNELS).filter((channel) =>
      rustChannels.has(channel)
    );
    expect(handled).toEqual([]);
  });
});
