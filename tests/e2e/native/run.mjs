#!/usr/bin/env node
// tests/e2e/native/run.mjs
// Tauri 原生窗 E2E（Windows / WebView2 + msedgedriver + tauri-driver）。
//
// 与 Web 轨的分工：
//   tests/e2e/web   —— Playwright 驱动 Vite 页面，IPC 被 mock，跑得快，PR 必跑。
//   tests/e2e/native—— 真实 Tauri 进程 + 真实 WebView2 + 真实 IPC，只验证
//                       「Web 轨永远验不到的东西」：应用能起来、__TAURI_INTERNALS__ 真实存在、
//                       invoke 命令真的能通。
//
// 为什么不用 WebdriverIO：只有 3 条冒烟断言，WebDriver 本身就是 HTTP 协议，
// 直接用 fetch 发 5 个端点即可，无需为一个冒烟测试引入整套 WDIO 依赖链。
//
// 前置条件（缺任何一个，脚本会明确报出该装什么，而不是抛一个看不懂的栈）：
//   1) cargo install tauri-driver
//   2) msedgedriver 版本必须与 WebView2 运行时一致，放在 tests/e2e/native/bin/
//   3) src-tauri/target/debug/moekoemusic.exe 存在（npm run tauri build --debug 或 tauri dev）
import { spawn } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const ROOT = process.cwd();
const PORT = Number(process.env.TAURI_DRIVER_PORT || 4444);
const BASE = `http://127.0.0.1:${PORT}`;

const DRIVER = path.join(ROOT, 'tests/e2e/native/bin/msedgedriver.exe');
const APP = path.join(ROOT, 'src-tauri/target/debug/moekoemusic.exe');
const TAURI_DRIVER = path.join(os.homedir(), '.cargo/bin/tauri-driver.exe');

// ── 前置检查 ──────────────────────────────────────────────
const problems = [];
if (!existsSync(TAURI_DRIVER)) {
  problems.push('缺少 tauri-driver → 执行：cargo install tauri-driver --version "^2.0.0"');
}
if (!existsSync(DRIVER)) {
  problems.push(
    '缺少 msedgedriver → 从 https://msedgedriver.microsoft.com/<WebView2版本>/edgedriver_win64.zip 下载并解压到 tests/e2e/native/bin/'
  );
}
if (!existsSync(APP)) {
  problems.push('缺少应用构建产物 → 先执行 npm run tauri build --debug（或 npm run tauri:dev 生成 debug 产物）');
}
if (problems.length) {
  console.error('\n[原生 E2E] 环境未就绪：');
  problems.forEach((p) => console.error('  - ' + p));
  console.error('\n当前 WebView2 版本可查：ls "C:/Program Files (x86)/Microsoft/EdgeWebView/Application"\n');
  process.exit(2);
}

/**
 * 产物新鲜度检查：exe 比源码旧意味着「测的是旧包」，
 * 此时绿了也不能说明当前代码没问题。只警告不中止 —— 开发时未必每次都重建。
 */
function newestMtime(dir) {
  let newest = 0;
  const walk = (d) => {
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      if (entry.name === 'target' || entry.name === 'node_modules' || entry.name === 'bin') continue;
      const full = path.join(d, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (/\.(js|vue|rs|json|html|scss)$/.test(entry.name)) {
        newest = Math.max(newest, statSync(full).mtimeMs);
      }
    }
  };
  walk(dir);
  return newest;
}

const exeMtime = statSync(APP).mtimeMs;
const srcMtime = Math.max(newestMtime(path.join(ROOT, 'src')), newestMtime(path.join(ROOT, 'src-tauri/src')));
if (srcMtime > exeMtime) {
  const hours = ((srcMtime - exeMtime) / 3600000).toFixed(1);
  console.warn(
    `\n[原生 E2E] ⚠️ 构建产物比源码旧 ${hours} 小时，本次验证的是旧二进制。\n` +
      `            要验证当前代码请先执行：npm run tauri build --debug\n`
  );
}

// ── WebDriver HTTP 客户端 ─────────────────────────────────
let sessionId = null;

async function wd(method, urlPath, body) {
  const res = await fetch(`${BASE}${urlPath}`, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  let json;
  try {
    json = JSON.parse(text);
  } catch {
    json = { value: text };
  }
  if (!res.ok) {
    throw new Error(`WebDriver ${method} ${urlPath} → HTTP ${res.status}: ${text.slice(0, 400)}`);
  }
  if (json?.value?.error) {
    throw new Error(`WebDriver ${method} ${urlPath} → ${json.value.error}: ${json.value.message}`);
  }
  return json?.value;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitForPort(timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${BASE}/status`, { signal: AbortSignal.timeout(1000) });
      if (res.ok) return;
    } catch {
      /* 还没起来 */
    }
    await sleep(300);
  }
  throw new Error(`tauri-driver 未在 ${timeoutMs}ms 内监听 ${PORT}`);
}

async function execute(script, isAsync = false) {
  const endpoint = isAsync ? 'async' : 'sync';
  return wd('POST', `/session/${sessionId}/execute/${endpoint}`, { script, args: [] });
}

/** 等待前端挂载完成（WebView 需要时间加载并执行 main.js） */
async function waitForAppReady(timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const state = await execute(`
        const root = document.querySelector('#app');
        return {
          ready: document.readyState,
          len: root ? root.innerHTML.length : 0,
          title: document.title,
        };
      `);
      last = state;
      if (state?.ready === 'complete' && state.len > 0) return state;
    } catch {
      /* 页面尚未可执行脚本 */
    }
    await sleep(500);
  }
  throw new Error(`应用未在 ${timeoutMs}ms 内完成渲染，最后状态：${JSON.stringify(last)}`);
}

// ── 用例 ──────────────────────────────────────────────────
const results = [];
function check(name, fn) {
  return (async () => {
    try {
      await fn();
      results.push({ name, ok: true });
      console.log(`  ok   ${name}`);
    } catch (e) {
      results.push({ name, ok: false, error: e.message });
      console.error(`  FAIL ${name}\n       ${e.message}`);
    }
  })();
}

function assert(cond, message) {
  if (!cond) throw new Error(message);
}

// ── 主流程 ────────────────────────────────────────────────
let driverProc;

async function main() {
  console.log('[原生 E2E] 启动 tauri-driver…');
  driverProc = spawn(
    TAURI_DRIVER,
    ['--native-driver', DRIVER, '--port', String(PORT)],
    { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true }
  );
  driverProc.stderr.on('data', (d) => {
    const s = String(d).trim();
    if (s) console.error('[tauri-driver] ' + s.slice(0, 300));
  });

  await waitForPort();
  console.log('[原生 E2E] tauri-driver 就绪，创建会话并启动应用…');

  const created = await wd('POST', '/session', {
    capabilities: {
      alwaysMatch: {
        'tauri:options': { application: APP },
      },
    },
  });
  sessionId = created?.sessionId;
  assert(sessionId, '未拿到 sessionId');

  const ready = await waitForAppReady();
  console.log(`[原生 E2E] 应用已渲染（title="${ready.title}"，根节点 ${ready.len} 字节）\n`);

  await check('应用窗口渲染出主界面外壳', async () => {
    assert(/萌音/.test(ready.title), `标题不含「萌音」：${ready.title}`);
    assert(ready.len > 500, `根节点内容过少（${ready.len} 字节），可能白屏`);
  });

  await check('运行在真实 Tauri 环境（__TAURI_INTERNALS__ 存在）', async () => {
    const has = await execute('return !!window.__TAURI_INTERNALS__;');
    assert(has === true, '未检测到 __TAURI_INTERNALS__，可能启动的是纯 Web 版本');
  });

  await check('真实 IPC 可用：app_boot_info 返回启动信息', async () => {
    const info = await execute(`
      const done = arguments[arguments.length - 1];
      window.__TAURI_INTERNALS__.invoke('app_boot_info')
        .then((r) => done({ ok: true, value: r }))
        .catch((e) => done({ ok: false, error: String(e) }));
    `, true);
    assert(info?.ok, `invoke 失败：${info?.error}`);
    assert(info.value && typeof info.value === 'object', 'app_boot_info 未返回对象');
    assert(typeof info.value.version === 'string', `缺少 version 字段：${JSON.stringify(info.value)}`);
  });

  console.log('\n[原生 E2E] 完成。');
}

async function cleanup() {
  if (sessionId) {
    try {
      await wd('DELETE', `/session/${sessionId}`);
    } catch {
      /* 会话可能已随窗口关闭 */
    }
  }
  if (driverProc && !driverProc.killed) {
    driverProc.kill();
  }
}

main()
  .catch((e) => {
    console.error('\n[原生 E2E] 执行中断：' + e.message);
    results.push({ name: '执行流程', ok: false, error: e.message });
  })
  .finally(async () => {
    await cleanup();
    const failed = results.filter((r) => !r.ok);
    console.log(`\n[原生 E2E] 结果：${results.length - failed.length} 通过 / ${failed.length} 失败`);
    process.exit(failed.length ? 1 : 0);
  });
