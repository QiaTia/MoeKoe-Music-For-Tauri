# MoeKoe Music (Tauri) — 自动化测试方案

> 适用版本：仓库 `main` @ `e1b2c55`（v1.6.9 线）。覆盖范围：前端 Vue3 + 后端 Rust/Tauri 双层，以及桌面端到端 UI 验证。

---

## 1. 项目结构与模块职责

```
MoeKoe-Music-For-Tauri/
├── src/                      # 前端（Vue 3 + Vite + Pinia + vue-i18n）
│   ├── views/                # 页面级组件（Home / Search / Lyrics / MiniPlayer / Recognize …）
│   ├── components/           # 业务组件（player/ 播放内核、search/、discover/ …）
│   ├── composables/          # 可复用逻辑（useActivatedWatch / useStickyDetailHeader）
│   ├── stores/               # Pinia 状态（musicQueue / store 登录态）
│   ├── utils/                # 纯函数工具（url 升级、时间格式化、recognize、tauriShim 兼容层）
│   ├── router/               # 路由表
│   └── config/               # 静态配置（免责声明、引导、设置键）
├── src-tauri/                # 后端（Rust，Tauri v2）
│   └── src/
│       ├── lib.rs            # 应用入口：命令注册、插件装配、深链解析、主/托盘/快捷键
│       ├── windows.rs        # 窗口管理：主窗/桌面歌词窗/MV窗/**迷你窗（形态/尺寸/边缘吸附）**
│       ├── settings.rs       # 设置持久化（store.json，对齐 electron-store）
│       ├── ipc.rs            # Electron 兼容 IPC 桥（15+ 通道）
│       ├── shortcuts/tray/statusbar/updater/ws/  # 各子系统
│       └── main.rs
├── rust-api/                 # 进程内酷狗 API 移植（kugoumusicapi crate，含 crypto/sign/request）
└── tests/                    # 本方案新增：tests/unit（Vitest）+ tests/e2e（Playwright）
```

### 核心业务流
1. **播放主流程**：`AudioController` → `SongQueue`（本地/在线/云盘三种队列）→ `LyricsHandler`（KRC 解析/翻译/音译）→ `PlayerControl`（状态广播）。
2. **迷你播放器（重点风险区）**：主窗发 `open-mini-window` → Rust `create_mini_window` 按持久化 `mode` 建窗（normal 320×460 / compact 320×80）→ 拖拽走原生 `start_dragging()`，松手 `WindowEvent::Moved` 触发 `snap_mini_window_if_needed` 边缘吸附 → `mini-state` 事件回传 Vue 维持 `isCompact` 同步。
3. **设置持久化**：`SettingsStore` 以 `store.json` 落盘（RwLock + JSON），`setting_on/merge_setting` 供各开关读取。
4. **深链/分享**：`moekoe://share?hash=&listid=` → `parse_deep_link` → `url-params` 事件。
5. **听歌识曲**：前端录 PCM → `recognize_audio` 命令（复用 cookie jar + `sign_binary`）→ 标准化映射。
6. **在线更新 / WS 远程控制 / 全局快捷键 / 托盘** 等系统集成。

---

## 2. 关键功能点与风险区域

| 风险区域 | 位置 | 风险类型 | 测试策略 |
|---|---|---|---|
| 迷你窗形态 ↔ 页面状态同步 | `windows.rs` / `MiniPlayer.vue` | **状态竞态 / 错配**（曾出现"窗小页高"） | 单测 `mini_size_for_mode`、`patch_mini_window_state`；E2E 验证 normal⇄compact |
| 边缘吸附几何 | `windows.rs::snap_mini_window_if_needed` | 多屏/任务栏/夹紧边界 | 抽纯函数 `compute_snap_position` 后单测 9 例 |
| 设置持久化 | `settings.rs` | 读写竞态、JSON 损坏、键污染 | 单测 init/set/merge/clear/load 6 例 |
| 深链解析 | `lib.rs::parse_deep_link` | scheme 误判、query 遗漏 | 单测 5 例 |
| 听歌识曲映射 | `recognize.js` | 字段映射错位、置信度、排序 | 单测映射/降级/排序 5 例 |
| 播放模式状态机 | `PlaybackMode.js` | 四态循环、持久化 | 单测 5 例 |
| 通用交互 | `Helpers.js` | 节流/快捷键/音量夹紧 | 单测 7 例 |
| Tauri ACL / 事件可达性 | `capabilities` + `tauriShim` | 窗口 `listen` 被静默拒绝 | E2E + 手动验证（见 §6） |
| 加密/签名 | `rust-api/crypto.rs` | 与 Node 原版对拍 | 已有单测（md5/sha1/aes/rsa/playlist） |

---

## 3. 测试框架选型

| 层 | 框架 | 选型理由 |
|---|---|---|
| 前端 单元/集成 | **Vitest 3** + `@vue/test-utils` + `jsdom` | 与 Vite 同源、零额外配置、启动快；`globals` 风格；v8 覆盖率原生支持 |
| 前端 覆盖率 | `@vitest/coverage-v8` | 行/分支/函数维度，输出 lcov 供 Codecov |
| 后端 单元/集成 | **cargo test**（内置） | Rust 原生，无需第三方；`#[cfg(test)]` 模块内联 |
| 后端 覆盖率 | `cargo-llvm-cov`（见 §7 可选） | 与前端 lcov 一并上报 |
| 端到端 | **Playwright** + `tauri-driver` | Tauri 官方推荐 WebView 驱动；多窗口/原生控件可控 |
| CI | **GitHub Actions** | 复用既有 release 工作流习惯，三档并行 + 门禁 |

**不选用**：Jest（与 ESM/Vite 摩擦大）、Cypress（Tauri 原生窗支持弱于 Playwright+tauri-driver）。

---

## 4. 测试分层与用例设计策略

### 4.1 单元测试（快、无 IO 依赖）
- **纯函数优先**：把可测逻辑从组件中抽出（如本次 `compute_snap_position` 重构），避免为测试引入 DOM。
- **Mock 边界**：`invoke`（Tauri）用 `vi.mock('@tauri-apps/api/core')`；`stores/store`、`i18n` 等框架层打桩，只测目标函数。
- **数据驱动**：时间格式化、吸附方向等用参数表断言。

### 4.2 集成测试
- **Rust**：`SettingsStore` 用临时 `store.json` 验证"内存 ↔ 磁盘"往返；`parse_deep_link` 验证跨模块契约。
- **前端**：组件级用 `@vue/test-utils` mount 验证 `PlayerControl` 等组件在 props/store 变化下的渲染（后续迭代扩展，本方案已铺好 `tests/unit` 骨架）。

### 4.3 端到端（真实应用）
- 启动 `tauri dev` → 等待 `__TAURI_INTERNALS__` → 通过真实 DOM/命令验证迷你模式全流程。
- E2E 用 `reuseExistingServer` + `xvfb`（Linux CI 无显示）。

---

## 5. 测试数据准备

| 场景 | 方式 |
|---|---|
| 单元/集成 | 内存构造 `SettingsStore::new()` + `init(temp_path)`；JS 侧 `localStorage.clear()` 后注入 |
| 网络依赖 | 全部 mock：酷狗 API / `recognize_audio` 的 `invoke` 返回固定 fixture |
| E2E 夹具 | 固定歌曲 hash、预设 `store.json`（normal/compact 各一份）置于 `tests/e2e/fixtures/` |
| 确定性 | JS 测试 `Date.now()` 用 `vi.useFakeTimers()`；Rust 用临时目录避免时间戳依赖 |

---

## 6. 持续集成执行流程（`.github/workflows/test.yml`）

```
push/PR ─▶ frontend-unit ─┐
          rust-unit ───────┼─▶ coverage-gate（门禁）
          e2e（仅 tag/手动）┘
```

- **frontend-unit**：`npm ci` → `npm run test:coverage` → 上传 `coverage/` + Codecov。
- **rust-unit**：装 Linux 系统库 → 配 `rsproxy` 镜像 → `cargo test --all-features` → 归档 `target/debug`。
- **e2e**：`cargo install tauri-driver` + `npx playwright install` → `xvfb-run npm run test:e2e` → 上传 `playwright-report/`。
- **门禁**：Vitest 阈值（语句 60% / 分支 50% / 函数 60% / 行 60%）；Rust 测试非零即失败。

### 本地快速命令
```bash
npm run test            # 前端单测（一次）
npm run test:coverage   # 前端单测 + 覆盖率
npm run tauri dev &     # 起应用
npm run test:e2e        # E2E（需先装 tauri-driver）
cargo test --lib        # 后端单测
```

---

## 7. 覆盖率目标与报告产出

| 维度 | 目标 | 当前实现 |
|---|---|---|
| 语句/行 | ≥ 60% | Vitest `thresholds` 已设；Rust 以 `#[cfg(test)]` 覆盖核心纯逻辑 |
| 分支 | ≥ 50% | 吸附方向、模式 fallback、深链分支已单测覆盖 |
| 函数 | ≥ 60% | 纯函数为主，覆盖率集中且稳定 |

**报告产出**：
- 前端：`coverage/index.html`（可读）、`lcov.info`（Codecov）、`coverage/cobertura.xml`。
- 后端：`cargo-llvm-cov --lcov`（可选，CI 注释中开启）。
- E2E：`playwright-report/index.html`（失败自动截图 + trace）。
- CI 产物：三份 artifact 归档，失败后必出。

---

## 8. 已落地产物清单（本方案实现）

后端（Rust，`cargo test --lib` → **23 passed**）：
- `src-tauri/src/windows.rs`：抽出 `compute_snap_position`（纯函数）+ 9 个吸附/尺寸/补丁测试
- `src-tauri/src/settings.rs`：6 个持久化/合并/加载/清理测试
- `src-tauri/src/lib.rs`：5 个深链解析测试

前端（Vitest，配置 + 4 个单测文件，已语法校验）：
- `vitest.config.js`
- `tests/unit/utils.test.js`（URL 升级 / 封面 / 时间格式化）
- `tests/unit/recognize.test.js`（识曲映射 / 置信度 / 降级 / 排序）
- `tests/unit/playbackMode.test.js`（四态状态机 / 持久化）
- `tests/unit/helpers.test.js`（节流 / 音量 / 快捷键）

端到端 + CI：
- `playwright.config.js` + `tests/e2e/mini-player.spec.js`
- `.github/workflows/test.yml`（frontend-unit / rust-unit / e2e / coverage-gate）

---

## 9. 后续建议（非本次范围）
1. 为 `MiniPlayer.vue` / `PlayerControl.vue` 补组件级 `@vue/test-utils` 用例（需 mock `tauriShim` 事件）。
2. `rust-api` 增加签名/请求层的契约测试（对拍线上接口，标记为 `#[ignore]` 避免依赖外网）。
3. 在 CI 接入 `cargo-llvm-cov` 并把 Rust 覆盖率并入 Codecov。
4. E2E 增加"多显示器 / 任务栏吸附"场景（需多显示器 fixture 或窗口位置模拟）。
```
