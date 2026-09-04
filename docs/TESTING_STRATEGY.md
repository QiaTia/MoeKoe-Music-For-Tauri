# MoeKoeMusic 自动化测试方案

> 适用架构：Tauri 2.x 桌面端（Rust `src-tauri` + Rust `rust-api` + Vue3/Vite 前端）
> 目标：在不依赖外部 KuGou 网络、不依赖桌面 WebView 的前提下，建立可本地运行、可 CI 门禁的分层自动化测试。

---

## 一、现状盘点（基线）

| 层 | 现有测试 | 说明 |
|---|---|---|
| 前端 `src/` | **0** | 无 vitest/jest；无测试脚本 |
| `rust-api` | 14 个 `#[cfg(test)]` 单测 | `crypto.rs`(6) `sign.rs`(5) `util.rs`(3)，均为纯函数级 |
| `src-tauri` | **0** | `parse_deep_link`、窗口 edge-snap 等纯逻辑完全未覆盖 |
| CI | `release.yml` 仅构建 | 无 `cargo test`、无前端测试步骤；其余 workflow 为 issue 管理 |

**核心风险**：① 前端任何重构都无回归保护；② `v1.7.0` 移植的组队深链 `parse_deep_link` 仅靠人工核对；③ 合并 `old` 远程后无测试门禁，易引入静默破坏。

---

## 二、测试分层策略（测试金字塔）

```
        ▲ E2E（少量，关键路径）
       ╱  Playwright（Web/PWA 版冒烟）— 可选 P2
      ╱
     ▢ 组件/集成（中等）
    ╱   Vitest + @vue/test-utils（关键组件挂载）
   ╱    rust-api + wiremock（KuGou 接口契约）
  ▼
 ███ 单元测试（最多、最快）
  Rust `cargo test`（rust-api + src-tauri 纯逻辑）
  前端 Vitest（player 逻辑 / stores / utils）
```

原则：
- **外部依赖一律 mock**：KuGou 接口用 `wiremock` 本地起服务；Tauri 命令用 `vi.mock('@tauri-apps/api')` 拦截。
- **纯逻辑优先**：先把无 I/O 的函数（签名、深链解析、播放队列、高潮点）用单测锁死，性价比最高。
- **桌面 E2E 延后**：Tauri 完整集成测试需 WebView 运行时，重且易碎，放 P2 或不强制。

---

## 三、分层落地清单

### 3.1 Rust `rust-api`（增强单测 + 接口契约测试）
- **dev-dependencies 新增**：`tokio`（带 `macros`/`rt-multi-thread`，与现有异步运行时一致）、`wiremock`（mock KuGou 网关）。
- **`src/sign.rs`**：已有 5 个（含 `signature_web_with_body` 绝对值对拍），补边界用例（空 body、特殊字符）。
- **`src/crypto.rs` / `src/util.rs`**：已有 14 个中的 9 个，保持并补异常输入。
- **`src/modules.rs`（**`/team/*` 端点 **— 重点）**：用 `wiremock` 在本地模拟 `gateway.kugou.com`，断言：
  - 请求 URL / `Host: gateway.kugou.com` 头 / 预置 `signature` 是否被原样下发；
  - `team_join` 的 POST body `{team_code}` 是否从 `query.body.team_code` 正确取出；
  - 响应 JSON 字段解析（如 `period_id` / `team_info`）映射正确。
- **命令**：`cargo test -p rust-api`（建议在 workspace 根加 `cargo test` 统一跑两个 crate）。

### 3.2 Rust `src-tauri`（补纯逻辑单测）
- **`src/lib.rs::parse_deep_link`**（v1.7.0 移植特性，最高价值）：
  | 输入 | 期望 `urlPath` | 期望 `teamcode` |
  |---|---|---|
  | `moekoe://share?hash=H&listid=L` | `share` | `""` |
  | `moekoe://team/join?code=ABC` | `join` | `ABC` |
  | `moekoe://` (无 path 无 code) | `share` | `""` |
  | `moekoe://join?code=XYZ` | `join` | `XYZ` |
- **`src/windows.rs` edge-snap**：若吸附计算已抽成纯函数（如 `fn snap(pos, screen, margin) -> pos`），补边界用例（贴左/右/顶边、越界回弹）。否则先 refactoring 抽出纯函数再测。
- 注意：`src-tauri` 内避免 `#[cfg(test)]` 触发 WebView 链接；只测纯函数，集成测试放 P2。

### 3.3 前端 Vitest（从零搭建）
- **新增依赖**：`vitest`、`@vue/test-utils`、`happy-dom`（轻量 DOM，CI 快）、`@pinia/testing`。
- **`vitest.config.ts`**：`environment: 'happy-dom'`，`setupFiles` 注入 Tauri mock。
- **纯逻辑单测**（无组件，最快回本）：
  - `src/components/player/ClimaxPoints.js` — 高潮点检测阈值/排序；
  - `src/components/player/PlaybackMode.js`、`SongQueue.js` — 队列与循环模式；
  - `src/utils/teamEvent.js`、`utils.js`、`utils/apiBaseUrl.js`；
  - `src/utils/tauriShim.js` — `window.electron`/`__TAURI_INTERNALS__` 分支降级。
- **stores 单测**：`src/stores/*`（Pinia）用 `@pinia/testing` 验证播放状态、设置持久化逻辑。
- **组件挂载测试**（关键组件）：`PlayerControl.vue`（歌词居中偏移）、`AudioOutput.js` 设备选择降级（无 `setSinkId` 时跳过）。
- **Tauri 隔离**：`tests/setup.ts` 中用 `vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))` 拦截所有 `invoke`，保证测试不触真机。

### 3.4 E2E（可选 P2）
- **Web/PWA 版**：`vite preview` 起服 + Playwright 冒烟（启动→搜索框渲染→播放控件可见）。
- **桌面端**：Tauri 官方 WebDriver（`tauri.conf.json` 开 `app.windows` 测试模式）+ Playwright；需在 CI 装 WebView 依赖（见 `release.yml` 的 Linux 系统包），成本高，建议仅关键路径。

### 3.5 CI 门禁（新增 `test.yml`）
- 触发：`pull_request` 到 `main`、`push` 到 `main`/`feat/*`。
- 矩阵：`ubuntu-22.04`，Node 22 + Rust stable。
- Steps：checkout → setup-node → dtolnay/rust-toolchain → `npm ci` → `npx vitest run` → `cargo test`(workspace) → （可选）Playwright。
- 门禁：以上任一步失败则 PR 不可合并。

---

## 四、分阶段实施路线

- **P0（地基，1 天）**：引入 Vitest 并给 `ClimaxPoints`/`PlaybackMode`/`SongQueue`/`utils` 写单测；`src-tauri` 补 `parse_deep_link` 单测；新增 `test.yml` 跑 `cargo test` + `vitest run`。→ 立即获得回归保护 + CI 门禁。
- **P1（接口与状态，2–3 天）**：`rust-api` 加 `wiremock` 契约测试覆盖 5 个 `/team/*` 端点；前端补 `stores` 与关键组件挂载测试；Tauri mock 固化。
- **P2（端到端，按需）**：Playwright Web 冒烟；桌面 Tauri WebDriver E2E（评估成本后决定是否启用）。

---

## 五、关键约定
- 外部网络/KuGou 一律 mock，测试不依赖真实登录态与活动期。
- 不硬编码密钥；测试用数据走 `tests/fixtures`。
- 测试文件就近或集中 `tests/`；Rust 用 `#[cfg(test)]` 模块，前端用 `*.test.ts` / `*.spec.ts`。
- 任何合并 `old` 远程后的回归，由 P0 门禁第一时间暴露。

---

## 六、落地文件清单（P0 起步）
- 新增：`docs/TESTING_STRATEGY.md`（本文件）、`vitest.config.ts`、`tests/setup.ts`、各 `*.test.ts`
- 新增：`.github/workflows/test.yml`
- 修改：`package.json`（`scripts.test`、`devDependencies`）、`rust-api/Cargo.toml`（`[dev-dependencies]`）、`src-tauri/src/lib.rs`（补 `#[cfg(test)]`）

## 七、示例（可直接照搬）

**`src-tauri/src/lib.rs` 单元测试（节选）**
```rust
#[cfg(test)]
mod tests {
    use super::parse_deep_link;

    #[test]
    fn team_join_sets_teamcode_and_join_path() {
        let r = parse_deep_link("moekoe://team/join?code=ABC").unwrap();
        assert_eq!(r["teamcode"], "ABC");
        assert_eq!(r["urlPath"], "join");
    }

    #[test]
    fn share_fallback_when_no_path_and_no_code() {
        let r = parse_deep_link("moekoe://").unwrap();
        assert_eq!(r["urlPath"], "share");
        assert_eq!(r["teamcode"], "");
    }
}
```

**`src/components/player/ClimaxPoints.test.ts`（Vitest 骨架）**
```ts
import { describe, it, expect } from 'vitest'
import { detectClimax } from './ClimaxPoints'

describe('detectClimax', () => {
  it('returns sorted peaks above threshold', () => {
    const pts = detectClimax([0, 0.9, 0.2, 0.95, 0.1], 0.8)
    expect(pts).toEqual([1, 3])
  })
})
```
