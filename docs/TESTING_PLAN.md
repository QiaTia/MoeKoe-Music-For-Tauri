# MoeKoe Music (Tauri) — 自动化测试方案 v2

> 版本：v2，基于 `main` @ v1.7.1 实测重写。
> 上一版（v1，对应 v1.6.9）保留原架构判断，但有三处「文档声称 ≠ 实际状态」的背离，已在本版修正。
> **P0 / P1 已完成，P2（契约测试 + 原生 E2E）已完成**（2026-10-23）：单测 255 例、覆盖率 95.51%、E2E Web 6 例、E2E Native 3 例、Rust 29 例。详见 §0.2–§0.6。

---

## 0. 实测审计（2026-10-23）

### 0.1 修复前基线

| 检查项 | 文档/配置声称 | 实测结果 | 结论 |
|---|---|---|---|
| Vitest 单测 | 4 个测试文件 | **4 文件 / 27 例 / 1.06s，全绿** | 健康，但覆盖面极窄 |
| 覆盖率门槛 | 语句 60% / 分支 50% / 函数 60% / 行 60% | 实际 **lines 3.55% / statements 3.55% / functions 51.11% / branches 78.84%**，阈值直接报 ERROR | **`frontend-unit` job 必红** |
| E2E 技术栈 | 「Playwright + tauri-driver」 | `tauri-driver` 是 **WebDriver 协议**服务端，Playwright 不支持连接它；配置从未生效 | **选型矛盾** |
| E2E 实际行为 | 等待 `__TAURI_INTERNALS__` 就绪 | spec 内**没有任何 `goto`**，也未设 `baseURL` → page 停留在 `about:blank` → `waitForTauri` 必然 30s 超时 | **4 个用例 100% 跑不通** |
| E2E 断言质量 | — | `expect(count).toBeGreaterThanOrEqual(0)`、`expect(...).toBeTruthy()` 恒真 | **假测试**（即使跑通也不验证任何东西） |
| rust-api 单测 | — | **14 例全过，2.45s**（crypto/sign/util 向量对拍质量很高） | 优质资产，但 **CI 从未执行** |
| src-tauri 单测 | 23 例 | CI 有跑（`cargo test --all-features`） | 保留 |
| `coverage-gate` job | 「门禁」 | 仅 `run: echo "..."` | **空壳门禁** |
| PR 保护 | — | e2e 仅在 tag / 手动触发 | PR 阶段无任何 UI 回归保护 |

修复前存在的两个硬伤：

1. **流水线红灯免疫**：门槛定在 60% 而实际 3.55%，红灯是常态，久而久之团队会忽略 CI —— 这比没有 CI 更危险。
2. **E2E 是负资产**：它给人一种「有端到端覆盖」的错觉，实际既跑不通也不断言，维护成本却真实存在。

### 0.2 P0 执行结果（已完成）

| 项 | 修复前 | 修复后（实测） |
|---|---|---|
| 前端单测 | 4 文件 / 27 例 | **20 文件 / 255 例，全绿，2.8s** |
| 覆盖率 | 3.55%（门槛 60%，必红） | **95.51%**（门槛 90/80/85/90，退出码 0） |
| 覆盖率统计范围 | `src/**` 全量（25000 行 .vue 入分母） | utils / stores / composables / plugins（有测试责任的目录） |
| E2E | 4 例停在 `about:blank`、断言恒真 | **6 例真实用例，5.3s 全绿**，PR 可跑 |
| E2E 通道 | `tauri dev`（含 Rust 编译，>3min） | `npm run serve`（秒级） |
| rust-api 测试 | 14 例存在但 CI 从未执行 | 已纳入 `rust-unit` job |
| `coverage-gate` | 仅 `echo` 的空壳 | 已移除，门禁回归真实阈值 |

覆盖明细（P1 后）：`stores` 100%、`plugins` 100%、`composables` 94.67%、`utils` 94.92%（其中 `utils.js` 27.68% → **99.46%**）。

### 0.3 E2E 首跑即抓到的 3 个真实缺陷

这三个都不是测试环境噪声，而是**用户可见的功能缺陷**，此前无任何机制能发现：

| # | 位置 | 症状 | 修复 |
|---|---|---|---|
| 1 | `index.html` 内联脚本 | 引用 `Object.keys(messages)`，而 `messages` 只存在于 `src/utils/i18n.js` 的模块作用域 → 任何浏览器都抛 `ReferenceError`，`<html lang>` 设置被阻断 | 内联脚本自带支持语言列表；`JSON.parse` 加 try/catch |
| 2 | `PlayerControl.vue` | `window.electron.ipcRenderer.on('request-player-state-sync')` 注册时未判空（回调内才判断）→ 非 Electron/Tauri 环境下 mounted 抛 TypeError | 改为可选链 `window.electron?.ipcRenderer?.on` |
| 3 | `Home.vue` | `response.data.song_list.sort()` 无防御 → 上游字段缺失/降级响应时整页崩 | `Array.isArray(songList)` 守卫 |

> 结论：一条 4.5 秒、断言到位的 E2E，价值高于十个恒真断言的「E2E」。

### 0.4 P1 执行结果（已完成）

| 项 | P0 后 | P1 后（实测） |
|---|---|---|
| 单测规模 | 160 例 | **243 例（+83）** |
| 全局行覆盖 | 79.77% | **95.51%** |
| `utils.js` | 27.68%（唯一短板） | **99.46%** |
| 组件测试 | 无 | **4 个组件 / 41 例**，四个文件合计行覆盖 96.09% |
| E2E | 4 例 | **6 例**（新增首页→歌单详情、详情页空数据兜底） |

新增覆盖的模块：

- `utils.js` 有副作用的函数：`applyCustomFont`（含 CSS 转义）、`applyColorTheme`（四套主题）、`getProfileBgColor`（canvas 采样与透明度加权）、`setTheme`（含 auto 模式监听器注册/注销）、`openRegisterUrl`、`openMvPlayer`（popup 与路由回退）、`share`（桌面深链 / Web 域名 / 登录昵称三态）、`requestMicrophonePermission`（六种权限分支）、`getAudioOutputDeviceSignature`。
- 组件：`CustomModal`（alert/confirm/prompt/loading 的 Promise 契约）、`BackToTop`（阈值、容器绑定、解绑）、`Disclaimer`（Web 与桌面 IPC 双模式）、`ContextMenu`（登录态分支、子菜单过滤、五个操作入口）。

P1 过程中又修复 1 个真实缺陷：

| 位置 | 症状 | 修复 |
|---|---|---|
| `CustomModal.vue::closePrompt` | 点「取消」只关 UI 不结算 Promise → 任何 `await $modal.prompt()` 永久挂起（Promise 泄漏，调用方后续逻辑永不执行） | 取消时 `promptResolve(null)` |

> 关于 `tests/setup/tauri-mock.js`：P1 未建设。四个组件测试各自用 `vi.mock()` 声明所需替身，其中 `ContextMenu` 需 5 个模块、`CustomModal` 只需 1 个 —— 差异过大，此时抽公共层会得到一个充满开关的伪抽象。**待第三个及以上组件共用同一套 IPC 路由时再抽**，这条记录保留为明确的技术判断而非遗漏。

### 0.5 P2 契约测试（已完成）

`tests/contract/api-contract.test.js`（12 例，8ms，纯静态扫描，不联网、不启应用）。

**为什么需要它**：前端通过 `invoke('api_request', { path })` 直调 Rust，路径或通道名写错时**不会编译失败、不会抛异常** —— Rust 返回 404 或静默丢弃，表现为「点了没反应」。这类错误只能靠契约测试兜住。

扫描结果：

| 维度 | Rust 侧 | 前端侧 | 差异 |
|---|---|---|---|
| API 路由 | 160 个 `ModuleDef { route }` | 50 个调用路径 | **2 个未注册** |
| IPC 通道 | 24 个 `match` 分支 | 23 个 `ipcRenderer.send` | **1 个无处理** |

发现的 3 处不一致：

| 缺口 | 位置 | 性质 | 处置 |
|---|---|---|---|
| `/user/cloud/del` | `CloudDrive.vue:719` | **真实功能缺失**：dispatch 返回 `404 Not Found`，Tauri 版云盘删除不可用 | 登记白名单待补 Rust 实现 |
| `/user/cloud/upload` | `CloudDrive.vue:546` | **真实功能缺失**，且更复杂：即便补 Rust 实现，`api_request` 也无法序列化二进制 body，需独立 command + `sign_binary`（与听歌识曲同一套路） | 登记白名单待补 |
| `current-song-updated` | `PlayerControl.vue:1014,1019` | 非缺陷：Electron 遗留无参通知；Tauri 侧歌曲信息已由 `player-meta` → `ws.update_current_song` 覆盖 | 登记白名单，建议直接删除该 `send` |

设计上的两道保险（避免契约测试自己烂掉）：

1. **解析自检**：先断言扫描到的条目数（路由 ≥150、前端调用 ≥40、通道 ≥20）。正则失配会让测试变成永远通过的空壳，这条断言专门防它。
2. **白名单双向校验**：既不允许出现新的未登记缺口，也不允许「已修复却仍挂在白名单」的僵尸条目 —— 后端补上实现后测试会提醒移除白名单。

有效性已验证：临时清空白名单后测试立即失败，并精确报出 `"/user/cloud/del ← srciews\CloudDrive.vue"`，恢复后 12 例全过。

> 未实现这两个 Rust 接口的原因：`rust-api/.api-doc/` 的采集语料与 `docs/API.md` 在当前工作区**并不存在**（长期记忆中的记录已失效），且无有效 token，无法验证参数与签名是否正确。按「未经验证不宣称修复」的原则，此处只登记缺口，实现需先拿到接口参数与可用的登录态。

### 0.6 P2 原生 E2E（已完成，本机实测通过）

`tests/e2e/native/run.mjs` + `npm run test:e2e:native`，3 例，约 6s（不含首次构建）。

与 Web 轨的分工是**互补而非重复**：

| 维度 | `tests/e2e/web` | `tests/e2e/native` |
|---|---|---|
| 驱动 | Playwright + Vite | tauri-driver + msedgedriver + 真实 WebView2 |
| IPC | **mock**（`page.route` + 无 Tauri 环境） | **真实**（真进程、真命令、真 cookie jar） |
| 覆盖 | UI 外壳、路由、空数据兜底 | 应用能否启动、`__TAURI_INTERNALS__` 是否真实存在、`invoke` 是否通 |
| 耗时 | 5.3s | ~6s（+ 首次构建） |
| 触发 | PR 必跑 | 手动 / 依赖 Windows 环境 |

三条用例：

1. **应用窗口渲染主界面外壳** —— 标题含「萌音」、`#app` 内容 > 500 字节（防白屏）。
2. **运行在真实 Tauri 环境** —— `window.__TAURI_INTERNALS__` 存在。这一条 Web 轨永远验不到（它是 mock 的）。
3. **真实 IPC 可用** —— `invoke('app_boot_info')` 返回含 `version` 的对象，证明 invoke 通道与 Rust 命令注册链路畅通。

环境与实现要点：

- `tauri-driver 2.1.0` + **msedgedriver 必须与 WebView2 运行时同版本**（本次为 `155.0.4283.62`，版本不匹配时会话直接建不起来）。
- **不引入 WebdriverIO**：只有 3 条冒烟断言，而 WebDriver 本身是标准 HTTP 协议，直接用 `fetch` 发 `/session`、`/execute/sync` 等 5 个端点即可，避免为冒烟测试拖入几十个依赖并改动 lock 文件。
- **产物新鲜度检查**：脚本会比较 `moekoemusic.exe` 与 `src/`、`src-tauri/src/` 的最新 mtime，产物更旧时打印醒目警告 —— 否则会出现「绿了但测的是旧包」的假通过。本机已验证：先警告「旧 208 小时」，重新 `tauri build --debug --no-bundle`（30s）后警告消失、3 例通过。
- 驱动二进制放 `tests/e2e/native/bin/` 并**加入 .gitignore**（版本需随 WebView2 更新，不入库）。
- 前置条件缺失时脚本以退出码 2 结束并逐条给出「该装什么」，不会抛裸栈。

> CI 已加 `e2e-native` job（`workflow_dispatch` 手动触发，windows-latest，自动探测 WebView2 版本下载对应 msedgedriver 后构建并运行）。**该 job 尚未在 GitHub 上实跑验证** —— 本机 Windows 环境已跑通，CI 侧首次手动触发时需留意 WebView2 目录路径与版本探测是否匹配 runner 实况。

---

## 1. 目标与原则

### 1.1 分层与投入配比

```
        /\        E2E 原生专项  (3%)   ← 窗口/托盘/吸边/多屏，tag + nightly
       /  \       E2E Web 流程  (7%)   ← 核心用户路径，PR 必跑
      /----\      组件测试      (15%)  ← 中等复杂度交互组件
     /------\     适配层测试    (25%)  ← tauriShim / request / invokeBridge
    /--------\    纯逻辑单测    (50%)  ← utils / stores / composables / Rust 纯函数
```

### 1.2 四条硬原则

1. **不测第三方，只测胶水**：酷狗上游返回什么不是我们的责任；`request.js` 怎么拼 auth、`sign.rs` 怎么算签名才是。
2. **不为测组件而测组件**：`PlayerControl.vue`（2101 行）、`PlaylistDetail.vue`（2036 行）先抽逻辑再测，不硬 mount。
3. **禁止恒真断言**：任何 `toBeGreaterThanOrEqual(0)`、`toBeTruthy()` 形式的断言视为未编写。
4. **测试必须能在离线环境跑**：默认全部 mock 网络与 IPC，真实上游调用单独打 `live` 标签。

---

## 2. 分层方案

### L1 · 纯逻辑单测（Vitest + jsdom）— 承担 50% 覆盖率

| 目标文件 | 行数 | 必测行为 |
|---|---|---|
| `src/utils/utils.js` | 371 | 已覆盖 URL/封面/时间；**待补** `formatNumber`、`share`、分页/裁剪辅助 |
| `src/utils/apiBaseUrl.js` | 76 | 环境判定矩阵（dev/prod/自定义/非法输入） |
| `src/stores/musicQueue.js` | 38 | 入队/出队/去重/清空/循环边界/切歌索引 |
| `src/stores/store.js` | 65 | 登录态写入、登出清理、持久化键名契约 |
| `src/utils/teamEvent.js` | 139 | 事件状态机、超时与失败分支 |
| `src/utils/riskVerify.js` | 150 | 风控响应识别、重试计数、终止条件 |
| `src/composables/*.js` | ~120 | `useActivatedWatch` 的 keep-alive 触发条件、`useStickyDetailHeader` 滚动阈值 |

> 目标：把 `src/utils/**` + `src/stores/**` + `src/composables/**` 拉到 **≥70%**。这三块合计约 1050 行，纯逻辑占比高，投入产出比最高。

### L2 · 适配层测试（Vitest）— 本项目最独特、最易回归的一层

这一层是 Electron → Tauri 迁移的接缝，是 bug 高发区，也是 v1 方案完全漏掉的一层。

| 目标文件 | 行数 | 必测行为 |
|---|---|---|
| `src/utils/tauriShim.js` | 216 | `send` → `invoke('ipc_send')` 参数形状；`once` 只触发一次；`removeListener` 后不再回调；**启动事件补发**（监听器晚注册仍能收到 `version`/`url-params`）；`platform` UA 探测三分支；非 Tauri 环境不注入 |
| `src/utils/invokeBridge.js` | 74 | `shouldUseInvoke` 判定矩阵（环境 × 接口 × 存在性） |
| `src/utils/request.js` | 243 | `buildAuth()` 拼接顺序与字段缺失省略；invoke / axios 双通道分流；10s 超时；401/风控响应映射为可读错误 |

**关键基建**：`tests/setup/tauri-mock.js`，让所有前端测试无需真实 Tauri 即可驱动 IPC。

```js
// tests/setup/tauri-mock.js —— 假的 Tauri 运行时 + invoke 路由表
import { vi } from 'vitest';

const handlers = new Map();   // command -> (args) => result | Promise<result>
const emitted  = [];          // 记录 Rust → 前端的事件，供断言
const listeners = new Map();  // channel -> Set<fn>

export function mockCommand(cmd, handler) { handlers.set(cmd, handler); }
export function emitFromRust(channel, payload) {
  emitted.push({ channel, payload });
  (listeners.get(channel) || []).forEach((fn) => fn({ payload }));
}
export function getEmitted(channel) {
  return emitted.filter((e) => !channel || e.channel === channel).map((e) => e.payload);
}

/** 在 beforeEach 里调用：注入最小 __TAURI_INTERNALS__ */
export function installTauriMock() {
  handlers.clear(); emitted.length = 0; listeners.clear();

  const internals = {
    invoke: vi.fn(async (cmd, args) => {
      const h = handlers.get(cmd);
      if (!h) throw new Error(`[tauri-mock] 未注册命令: ${cmd}`);
      return h(args);
    }),
    transformCallback: (cb) => {
      const id = Math.random().toString(36).slice(2);
      globalThis[`_${id}`] = cb;
      return id;
    },
  };
  globalThis.window.__TAURI_INTERNALS__ = internals;
  return internals;
}

export function uninstallTauriMock() {
  delete globalThis.window.__TAURI_INTERNALS__;
}
```

配套 `@tauri-apps/api` 的模块级替身（`tests/setup/tauri-api-mock.js`），让 `import { invoke } from '@tauri-apps/api/core'` 直接走上面的路由表，无需在执行到一半才替换全局对象：

```js
// vitest.config.js 中通过 test.alias 指向此文件
import { installTauriMock } from './tauri-mock';
export const invoke = (...a) => globalThis.window.__TAURI_INTERNALS__.invoke(...a);
export const listen = (channel, fn) => {
  const set = (globalThis.__mockListeners ??= new Map()).get(channel) ?? new Set();
  set.add(fn); globalThis.__mockListeners.set(channel, set);
  return Promise.resolve(() => set.delete(fn));
};
```

### L3 · 组件测试（@vue/test-utils）— 承担 15%

只挑「有独立交互契约、且体量可控」的组件，按优先级：

| 批次 | 组件 | 行数 | 测什么 |
|---|---|---|---|
| 一批 | `ContextMenu` / `CustomModal` / `BackToTop` / `Disclaimer` | 50–167 | 显隐、受控/非受控、遮罩关闭、`emits` 契约 |
| 二批 | 搜索建议 / 歌曲列表项 / 排行榜条目等列表型组件 | 中等 | props → 渲染映射、点击/双击 `emit`、空态与骨架 |
| 三批 | `MiniPlayer.vue` | — | `isCompact` 形态切换、事件驱动渲染（**依赖 L2 mock**） |

**明确排除**：`PlayerControl.vue`、`PlaylistDetail.vue`、`LocalMusic.vue`、`Settings.vue`、`Recognize.vue`。这五个合计逾 9000 行，mount 成本极高、断言脆弱。正确做法是先把其中的纯逻辑抽到 `composables/` 再测，而非硬测。

### L4 · Rust 测试（cargo test）

| 目标 | 现状 | 待补 |
|---|---|---|
| `rust-api`：`crypto.rs` / `sign.rs` / `util.rs` | **14 例全过**，含与 Node 原版的 parity 向量 | 保持；新增签名入参即新增向量 |
| `rust-api`：`request.rs` / `dispatch.rs` / `modules.rs` | 无 | URL 与 query 构造、路由分派、160 个接口的参数映射快照 |
| `src-tauri`：`windows.rs` / `settings.rs` / `lib.rs` | 23 例（吸附几何 9 + 持久化 6 + 深链 5） | `ipc.rs` 通道映射表完整性（防漏注册）、`ws.rs` 消息协议、`updater.rs` 版本比较 |

**P0 必做**：CI 的 `rust-unit` job 目前只跑 `src-tauri`，需并行加一步 `rust-api`。这是**零成本捡回来的 14 个用例**。

### L5 · E2E（双轨，替换现有单轨错误方案）

三种可行路线对比后取 C + A 组合：

| 方案 | 可行性 | 覆盖面 | 成本 | 用途 |
|---|---|---|---|---|
| A. WebDriverIO + `tauri-driver` | 官方推荐，Win/Linux 均可 | 原生窗 + DOM | 中 | **原生专项**（窗口/吸边/托盘） |
| B. Playwright 连 WebView2 CDP | 仅 Windows，Linux WebKitGTK 不支持 | DOM + 网络拦截 | 低 | 备选，不作主路 |
| C. Playwright 跑 Vite 页面 + mock IPC | 全平台稳定 | 仅 UI 流程 | 最低 | **PR 主力** |

**C 轨（`tests/e2e/web`，PR 必跑）**：Playwright 直接 `goto('http://localhost:8080')`，启动前注入 `__TAURI_INTERNALS__` 与命令路由（复用 L2 的 mock 思路，通过 `page.addInitScript` 注入）。覆盖：首页加载 → 搜索 → 进入歌单 → 播放条出现 → 切歌 → 打开设置 → 主题切换。

**A 轨（`tests/e2e/native`，手动 / Windows 环境）**：`tauri-driver` + msedgedriver + 自写 WebDriver HTTP 客户端（不引 WebdriverIO），验证真实 Tauri 进程下应用可启动、`__TAURI_INTERNALS__` 真实存在、`invoke` 链路可用。详见 §0.6。

> 原生窗口几何（迷你窗尺寸、吸边）**无法直接经 WebDriver 读取**（那属于原生窗口属性，不在 DOM 里）。若要覆盖，需在页面内通过 `getCurrentWindow().innerSize()` 取回并断言，或把几何逻辑留在 Rust 纯函数单测（`compute_snap_position` / `mini_size_for_mode` 已有 12 例）。

现有 `tests/e2e/mini-player.spec.js` 处置：**删除**（4 个用例无 `goto`、断言恒真）。其意图由 A 轨的 3 个真实用例承接。

### L6 · 契约测试

**已实现（静态契约，`tests/contract/api-contract.test.js`）**：扫描 Rust 路由注册表与前端调用路径、Rust IPC 通道与前端 `send` 通道，做存在性比对。零依赖、8ms、随 PR 跑。详见 §0.5。

**未实现（响应 schema 契约）**：原计划回放 `rust-api/.api-doc/responses/*.json` 校验字段结构，但该目录与 `rust-api/docs/API.md` 在当前工作区**均不存在**（早期采集产物已丢失），因此 schema 级契约暂无从建立。

补齐路径（按优先级）：

1. 先恢复采集：启动 `rust-api` 服务，用有效登录态抓取关键接口响应并落盘为 fixtures。
2. 再对 20 个关键接口做 schema 断言（必需字段存在、类型正确、嵌套结构稳定），防止上游字段漂移导致静默白屏。
3. 在线探测打 `live` 标签默认 `skip`，仅 nightly 跑，避免外网抖动污染 PR；写操作必须自带配对回滚。

---

## 3. 覆盖率策略：先修正分母，再爬坡

3.55% 的根源是 `include: ['src/**/*.{js,vue}']` 把 25,000 行 `.vue` 全计入分母，而测试只碰了 utils。**分母不管理，门槛永远是自欺或自伤。**

已实施（`vitest.config.js`）：收窄分母到「有测试责任」的四个目录，全局门槛取实测基线下浮 5 个点。

```js
// vitest.config.js（当前生效配置）
test: {
  coverage: {
    provider: 'v8',
    include: [
      'src/utils/**/*.js',
      'src/stores/**/*.js',
      'src/composables/**/*.js',
      'src/plugins/**/*.js',
    ],
    exclude: ['src/**/*.d.ts', 'src/utils/i18n.js'], // i18n 仅配置装配，无分支
    // 实测 95.51 / 86.65 / 92.48 / 95.51，门槛留 5 点缓冲
    thresholds: { statements: 90, branches: 80, functions: 85, lines: 90 },
  },
}
```

爬坡路线（P0 已达成，后续按实际余量推进）：

| 阶段 | 统计范围 | 行门槛 | 状态 |
|---|---|---|---|
| P0（止血） | utils + stores + composables + plugins | 75% | **已达成：79.77%** |
| P1 | 同上 + 补齐 `utils.js` 与运行时函数 | 90% | **已达成：95.51%** |
| P2 | 追加 `components/` 白名单（4 个组件已 96.09%） | 40% | 待做 |
| P3 | 追加已抽逻辑的重点 views | 30% | 待做 |

> `views/**` 与 `PlayerControl.vue` 在完成逻辑抽取前**不入分母**，这是显式的技术债登记，而非回避。
> 注意：门槛按「当前可达 + 5 点缓冲」设定，避免偶发抖动造成假红灯；每次抬升都应伴随真实用例补入。

---

## 4. CI 编排：测试归 Test，发版串测试门

修复前问题：PR 无 UI 保护、`coverage-gate` 是 `echo`、rust-api 漏跑、artifact 路径越级、
**release 与 test 互不依赖（Test 红灯拦不住发版）**、test 又在 tag 上重复跑一遍却毫无作用。

```
PR / push main ──▶ frontend-unit（255 例 + 覆盖率门槛）┐
               ──▶ e2e-web（6 例，约 5s）              ─┼─▶ test.yml
               ──▶ rust-unit（src-tauri 29 + rust-api 14）┘

tag v* / 手动 ──▶ release.yml
                  preflight（tag 必填 + 三处版本一致）
                    └─▶ gate（单测/覆盖率/E2E/Rust 全跑）
                          └─▶ build × 5 平台矩阵 ─▶ Release

手动 ──▶ e2e-native（Windows / 真实 WebView2 + 真实 IPC）
```

已实施：

1. `test.yml` **移除 tag 触发**：Actions 的 workflow 之间没有隐式依赖，tag 上并行跑测试拦不住 release，只是白烧 5 个 job 的分钟数。发版门禁改由 `release.yml::gate` 承担。
2. `release.yml::preflight`：`workflow_dispatch` 的 version 留空**直接失败**（旧写法 `inputs.version || github.ref_name` 在 ref_name 为分支名时会创建错误 tag）；并校验 tag 与 `package.json` / `tauri.conf.json` / `Cargo.toml` 三处版本一致 —— Cargo.toml 需手工同步，是漏改高发点。
3. `release.yml::gate`：作为 `build` 的 `needs`，测试不过则不出包。
4. 覆盖率门禁：`npm run test:coverage`，阈值 90/80/85/90 自身非零退出；`coverage-gate` 空壳 job 已删除。
5. Codecov 上传步骤已移除：仓库无 `codecov.yml`、无徽章，且该步骤配了 `continue-on-error` + `fail_ci_if_error: false`，实为空转（用的 v4 也已落后至 v7）。覆盖率报告仍以 artifact 归档。
6. action 版本统一为 `checkout@v7` / `setup-node@v7` / `upload-artifact@v7`。**升级已确认安全**：upload-artifact v7 的新增 `archive` 参数不影响现有 `name`+`path` 用法；checkout v7 的破坏性变更仅涉及 `pull_request_target` / `workflow_run` 的 fork PR 检出，本项目未使用这两种触发。
7. 修正 `tee ../../rust-test-output.log` 的越级路径为 `../`。
8. `rust-cache` 的 `workspaces` 同时声明 `src-tauri` 与 `rust-api`，两处编译缓存独立命中。

### 4.1 已删除的社区运营类 workflow

`AiIssueCheck.yml`（issues opened/edited）与 `AutoCloseIssues.yml`（每日 cron）**已于 2026-10-23 成对删除**。

判定依据：

- 项目当前 issue 量为零，两条流水线实际处于空转状态（AutoClose 的输入就是 AiIssueCheck 打的 `invalid` 标签，前者不工作则后者永远无事可做，**不能单独保留其一**）。
- 即便日后需要，issue 模板 `.github/ISSUE_TEMPLATE/*.yml` 是 issue form 且关键字段已设 `required: true`，GitHub 原生即拦住了「缺字段」这一大类，AI 校验只剩「填了但没填对」这点边际价值。
- 原先的隐性成本：依赖 `OPENAI_API_KEY` / `OPENAI_API_URL` 密钥与 `ISSUE_RULES` 变量，任一缺失即静默跳过；四处会让 step 直接失败的脆弱点（`missing` 字段缺失、`invalid` 标签不存在返回 422、模型名硬编码、非 2xx 抛裸异常）。

> 如日后 issue 量上来需要恢复：两个文件必须**一起**恢复，只恢复 AiIssueCheck 会导致 issue 被标 `invalid` 后永不关闭；只恢复 AutoCloseIssues 则永远没有 `invalid` 可关。

> 未接入项：`dorny/test-reporter` 的 PR 失败摘要、`cargo-llvm-cov` Rust 覆盖率趋势 —— 属 P2，需先有稳定绿基线。

---

## 5. 测试基建目录

```
tests/
├── setup/
│   ├── vitest.setup.js        # ✅ 已建：MediaElement/matchMedia/ResizeObserver/剪贴板替身、UI 全局 stub、用例间清理
│   ├── tauri-mock.js          # ⏸ 暂缓：见 §0.4 说明（组件各自 vi.mock，差异过大不强行抽象）
│   └── tauri-api-mock.js      # ⏸ 暂缓：同上
├── fixtures/
│   ├── song.js  playlist.js  lyric.js  user.js   # ⬜ P1：工厂函数，带默认值可局部覆盖
│   └── api/                                      # ⬜ P2：从 .api-doc/responses 摘取的契约样本
├── unit/          # ✅ L1 + L2（15 文件 / 202 例）
├── component/     # ✅ L3（4 文件 / 41 例）
├── contract/      # ✅ L6 静态契约（1 文件 / 12 例，前后端路由与 IPC 通道一致性）
└── e2e/
    ├── web/       # ✅ C 轨 Playwright（app-shell.spec.js，6 例，PR 必跑）
    └── native/    # ✅ A 轨：tauri-driver + msedgedriver + 自写 WebDriver 客户端（3 例，手动触发）
```

> 当前模块级 mock 在各自测试文件内用 `vi.mock()` 声明（如 `@tauri-apps/api/core`、`@/utils/request`），
> 尚未抽象成 `tauri-mock.js`。P1 做组件级测试时再抽公共层，避免过早建设未被使用的抽象。

`tests/setup/vitest.setup.js` 必须处理 jsdom 缺失的原生 API，否则播放相关测试会直接抛错：

```js
// jsdom 不实现 HTMLMediaElement 播放方法，AudioController 一调用就崩
Object.defineProperty(HTMLMediaElement.prototype, 'play', {
  configurable: true, value: vi.fn(() => Promise.resolve()),
});
Object.defineProperty(HTMLMediaElement.prototype, 'pause', {
  configurable: true, value: vi.fn(),
});
Object.defineProperty(HTMLMediaElement.prototype, 'currentTime', {
  configurable: true, writable: true, value: 0,
});
// 需保留真实 Date，但对依赖计时器的逻辑单独 vi.useFakeTimers()
globalThis.matchMedia ??= () => ({ matches: false, addEventListener() {}, removeEventListener() {} });
globalThis.ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} };
afterEach(() => { localStorage.clear(); vi.clearAllMocks(); });
```

---

## 6. 优先级路线图与验收标准

### P0 · 止血 —— 已完成（2026-10-23）

| # | 动作 | 验收 | 结果 |
|---|---|---|---|
| 1 | 收窄覆盖率分母，门槛按实测基线下浮 5 点设为 75/80/75/75 | `test:coverage` 退出码 0 | ✅ 79.77%，EXIT=0 |
| 2 | 删除 `tests/e2e/mini-player.spec.js`，重建 `tests/e2e/web` 真实用例 | 假用例不再出现 | ✅ 4 例真实用例，4.5s 绿 |
| 3 | `rust-api` 纳入 CI | CI 日志出现 14 passed | ✅ 已加独立 step |
| 4 | 移除 `coverage-gate` 的 `echo` 空壳 | 门禁由真实阈值承担 | ✅ 已删除该 job |
| 5 | 补 L1/L2 单测（原计划属 P1，因门槛必须真实达标而提前） | 覆盖率 ≥75% | ✅ 27 → 160 例 |
| 6 | 修正 `test.yml` 中 `tee ../../` 的越级路径 | artifact 能正确归档 | ✅ 改 `../` |

> P0 完成的标志是 **`Test` 工作流首次全绿**。这是后续一切的地基。
> 注：本机验证覆盖率时若遇 `safe-delete` 拦截清理 `coverage/`，用 `--coverage.clean=false` 复跑即可；CI（Ubuntu）无此限制。

### P1 · 建塔 —— 已完成（2026-10-23）

| # | 动作 | 验收 | 结果 |
|---|---|---|---|
| 7 | 补 `utils.js` 运行时函数用例 | 全局行覆盖 ≥85% | ✅ `utils.js` 99.46%，全局 95.51% |
| 8 | 组件测试（ContextMenu / CustomModal / BackToTop / Disclaimer） | 组件层行覆盖 ≥40% | ✅ 四个组件 96.09%，41 例 |
| 9 | `e2e/web` 扩到歌单路径 | 5–6 条稳定用例 | ✅ 6 例，5.3s |
| 10 | 修 `CustomModal` 取消不结算 Promise 的缺陷 | `await $modal.prompt()` 可正常返回 | ✅ 返回 null |
| — | `tests/setup/tauri-mock.js` | — | ⏸ 暂缓（见 §0.4，避免伪抽象） |

### P2 · 加固（2–4 周）

| # | 动作 | 验收 |
|---|---|---|
| 10 | `e2e/native` 上 tauri-driver 原生冒烟 | 3 条原生用例通过 | ✅ 已完成（§0.6，本机实测；迷你窗/吸边仍需具体用例） |
| 11 | 前后端 API/IPC 契约测试 | 前端调用与 Rust 注册表一致 | ✅ 已完成（§0.5），抓出 3 处不一致 |
| 12 | 补 `/user/cloud/del`、`/user/cloud/upload` 的 Rust 实现 | 契约白名单清空 | ⬜ 待做（需接口参数 + 有效登录态） |
| 13 | `cargo-llvm-cov` 接入 nightly | Rust 行覆盖 ≥40% 并有趋势 | ⬜ 待做 |
| 14 | `src-tauri` 补 `mv_window_title` / `nearest_safe_position` 单测 | Rust 用例 23 → 29 | ✅ 已完成（§0.6） |

---

## 7. 反模式清单（评审测试代码时逐条对照）

1. **恒真断言**：`toBeGreaterThanOrEqual(0)`、`expect(true).toBe(true)`、只断言元素「存在数组」而不校验内容。
2. **无导航的 E2E**：spec 未 `goto` 也未设 `baseURL`，测的是 `about:blank`。
3. **`waitForTimeout` 硬等待**：应改为 `expect(...).toBeVisible()` 或 `waitForFunction` 条件等待。
4. **测实现细节**：断言内部私有函数调用次数、组件内部 `ref` 名称，重构即碎。
5. **冲覆盖率的空测试**：只 `mount()` 不断言，把数字做上去却不验证行为。
6. **依赖真实网络**：默认用例打真实酷狗接口，导致 CI 随机红；在线用例必须打 `live` 标签并单独触发。
7. **跨用例状态污染**：未 `localStorage.clear()` / 未重置 mock 路由表，用例顺序敏感。
8. **用 `window.electron?.` 探测 Tauri 能力**：Tauri 下该对象由 shim 事后注入，正确探针是 `window.__TAURI_INTERNALS__`。

---

## 8. 命令清单

```bash
npm run test                 # L1 + L2 单测（一次）
npm run test:watch           # 监听模式
npm run test:coverage        # 覆盖率 + 门槛校验（含契约测试）
npx vitest run tests/contract # 仅契约测试（8ms，无需网络）
npm run test:e2e             # E2E Web 轨（自动拉起 Vite 8080）
npx playwright test tests/e2e/web            # 仅 Web 流程
npm run test:e2e:native                      # 原生冒烟（需 tauri-driver + msedgedriver + debug 产物）
npx tauri build --debug --no-bundle          # 原生测试前重建产物（30s 增量）
npx wdio run tests/e2e/native/wdio.conf.js    # A 轨（需 tauri-driver + 已构建应用）
cd rust-api   && cargo test --lib            # 14 例，2.45s
cd src-tauri  && cargo test --lib            # 23 例
```

---

## 9. 与 v1 的差异摘要

| 项 | v1 | v2 |
|---|---|---|
| 覆盖率 | 全量 `src/**` 一刀切 60%（实际 3.55%，CI 恒红） | 收窄分母至 4 个责任目录 + 全局门槛 75（实测 79.77%）+ 四阶段爬坡 |
| E2E 选型 | 「Playwright + tauri-driver」（协议不兼容） | Web 轨已落地（Playwright，PR 跑）；原生轨规划为 WebdriverIO/tauri-driver（P2） |
| E2E 用例 | 4 个无导航、断言恒真 | 删除重建，按真实用户路径与原生行为拆分 |
| 适配层 | 未覆盖 | 作为独立一层重点投入（tauriShim / request / invokeBridge） |
| rust-api | 有测试但 CI 漏跑 | 纳入 CI |
| 门禁 | `echo` 占位 | 真实阈值 + nightly Rust 覆盖 |
| 反模式 | 未定义 | 8 条评审清单 |
