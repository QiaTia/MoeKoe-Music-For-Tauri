# v1.7.0 移植交付总览（Electron → Tauri）

> 分支 `feat/v1.7.0-port` ｜ 基线 v1.6.9 → 目标 v1.7.0 ｜ 版本已 bump 至 **1.7.0**

## 一、做了什么

把 old 仓库（原版 Electron）**v1.6.9 → v1.7.0** 的增量，适配进当前 Tauri 架构。核心增量是 3 个新特性 + player 系列改进 + 若干视图更新。

| 类别 | 内容 | 适配方式 |
|---|---|---|
| 组队活动 | `TeamEvent.vue` + `utils/teamEvent.js` + Library 入口 | 前端干净套用；后端 **rust-api 新增 5 个 `/team/*` 端点**；深链补 `teamcode` |
| 音频输出设备 | `player/AudioOutput.js` | 前端套用；`PlayerControl` 保留 **Tauri 守护**（WebView 不支持 setSinkId 时跳过） |
| 高潮点定位 | `player/ClimaxPoints.js` | 前端干净套用 |
| player 改进 | AudioController/LyricsHandler/MediaSession/ProgressBar/index | 前端干净套用 |
| 视图 | CloudDrive/Library/Lyrics/ContextMenu/settings | 前端套用（settings 新增 backgroundThrottling 项） |

## 二、关键决策

1. **三路合并**：`main` 是 v1.6.9 直系后代，用 `git diff v1.6.9 v1.7.0 -- src/ | git apply --3way`。14 文件干净套用，仅 `PlayerControl.vue` / `PlayerControl.scss` 三处冲突（与 mini-player 重叠），已手工合并。
2. **组队端点逐字移植**：Node `module/team_*.js` → `rust-api/src/modules.rs`。复用现有 `crypto_rsa_encrypt`（原始无填充 RSA）+ `signature_web`；**新增 `sign::signature_web_with_body`**（POST 带 body 签名），含绝对值对拍测试。token = `h5`+大写 hex，前缀 `moc.uoguk.59::`，固定 lite 公钥。
3. **主进程其余变更**（Chrome 扩展、HTML 托盘面板、macOS 状态栏歌词）在 Tauri 版已裁剪，**不移植**。
4. **`backgroundThrottling`**：Tauri v2 无对应 API，设置项保留 UI 但**暂不生效**（记录为已知限制）。

## 三、验证结果

- `rust-api`：`cargo check` ✅、`cargo test`（14 测试全过，含新增签名对拍）✅
- `src-tauri`：`cargo check` ✅
- 前端：`npm run build`（vite）✅
- 版本：package.json / tauri.conf.json / Cargo.toml 统一 1.7.0 ✅

## 四、已知限制 / 后续

- `backgroundThrottling` 设置暂不生效（Tauri 无 API）。
- 组队活动链路（前端→rust-api→酷狗 `/youth/v1/ut/*`）已实现并通过编译/单测，但**未做真机联调**（需登录态 cookie + 真实活动期）。
- 变更已 `git add` 暂存于 `feat/v1.7.0-port`，**未提交**——如需我提交/推送请告知。
