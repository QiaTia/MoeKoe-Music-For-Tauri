# macOS 标题栏展示歌词 — 可行性报告

> 范围：评估在 macOS 平台把「当前播放歌词」展示在标题栏区域的可行性，给出可选方案、对比与落地路线。
> 日期：2026-08-01

---

## 1. 背景与现状

本项目已有三套歌词展示位面：

| 位面 | 实现 | 状态 |
|---|---|---|
| 主界面歌词面板 | `LyricsHandler.js` + 歌词视图 | 可用 |
| 独立桌面歌词窗 | `create_lyrics_window`（透明/置顶/鼠标穿透） | 可用 |
| 状态栏歌词（菜单栏） | `StatusBarLyrics.vue` + 设置项 `statusBarLyrics` | **前端已就绪，Tauri 原生桥接缺失** |

### 关键发现（决定可行性）

1. **前端渲染器已完整实现**。
   `src/components/StatusBarLyrics.vue` 已经写好一个离屏 Canvas 渲染器：把当前歌词行（含 Logo）画成 200×22pt 的 PNG，并通过
   `window.electron.ipcRenderer.send('update-statusbar-image', dataUrl)` 推给原生侧。它自带 marquee 滚动、长文本裁剪、占位符逻辑——**完全是 macOS 菜单栏歌词的成品前端**。

2. **Tauri 原生侧是空 stub**。
   `src-tauri/src/ipc.rs:211` 显式静默忽略该通道：
   ```rust
   // mac 状态栏歌词（Tauri 版暂不支持，静默忽略）
   "update-statusbar-image" | "update-current-lyrics" => {}
   ```
   注释说明这是从 Electron 迁移时预留、尚未实现的桥接。

3. **数据来源现成**。
   `LyricsHandler.js` 的 `getCurrentLineText(currentTime)` 直接返回当前行文本；播放进度在 `PlayerControl.vue` 中持续驱动。无需新增数据链路。

4. **窗口/环境已具备**。
   - macOS 主窗口当前固定用原生标题栏：`decorations(true)`（`src-tauri/src/windows.rs:31`）。
   - `tauri.conf.json` 已设 `macOSPrivateApi: true`——这是 macOS 透明/ Vibrancy 窗口的**前置要求**，已经满足。
   - `Header.vue` 已用 `data-tauri-drag-region` 实现拖拽区，透明标题栏的拖拽模式可直接复用。

> 结论：所谓「可行性」已被大幅前置——前端 80% 完成，缺口只是把 Rust 空 stub 补成真正的 macOS 原生桥接。

---

## 2. 可选方案

### 方案 A：系统菜单栏（Menu Bar / 状态栏）歌词 — 最贴合「标题栏」语义

- **描述**：在屏幕顶部系统菜单栏右侧显示一个状态项（Status Item），内含滚动歌词 + Logo。这是 macOS 用户对「标题栏歌词」最直觉的理解（LyricsX、网易云等均有此模式）。
- **复用**：`StatusBarLyrics.vue` 前端**零改动或极少改动**；只需把 `ipc.rs:211` 的空 stub 实现为真正的 `NSStatusItem`。
- **技术**：
  - Rust 侧用 `objc2-app-kit` 创建 `NSStatusItem`（长度 `NSVariableStatusItemLength`），将前端 dataURL 解码为 `NSImage` 设置到 `button`。
  - 命令：`update_statusbar_image(data_url)` / `update_current_lyrics(text)` / `set_statusbar_visible(bool)`。
  - 可选：`update-current-lyrics` 走 `NSStatusBarButton.title`（纯文本，免去图片），与图片通道二选一或并用。
- **优点**：最原生、最贴合需求语义；不侵入 App 窗口布局；窗口最小化/非激活时仍可见；前端成本极低。
- **缺点/风险**：仅 macOS（正好契合需求）；需新增 `objc2-app-kit` / `objc2-foundation` 依赖；菜单栏空间有限（约 200pt），长文本依赖前端 marquee（已处理）；App 未常驻时菜单项随窗口退出消失（可接受，或做登录项）；需适配深色模式与点击弹出菜单。
- **工作量**：中（Rust 桥接约 150–250 行 + 编译验证，约 2–3 天）。

### 方案 B：主窗口统一标题栏（Unified / Transparent Title Bar）歌词条

- **描述**：将主窗口标题栏设为透明（`TitleBarStyle::Transparent`），把当前歌词行渲染到**窗口自身标题栏区域**（红绿灯右侧、Header 顶部融合成一条居中歌词带）。
- **技术**：
  - `WebviewWindowBuilder.title_bar_style(TitleBarStyle::Transparent)`（仅 macOS）+ `objc2-app-kit` 设置窗口背景色 / Vibrancy。
  - 前端在 `Header.vue` 顶部加一条歌词带，沿用 `data-tauri-drag-region` 保证拖拽不被破坏。
- **优点**：全在 webview 内，无原生 API 风险；视觉最统一（类 macOS Music 的 unified toolbar）；可跨平台降级（Windows 自定义 `TitleBar.vue` 已支持）。
- **缺点**：仅窗口可见/激活时显示；需重排 Header（红绿灯 | 歌词 | 搜索/头像），顶部约 28–36pt 区域重做；透明标题栏会失去系统部分窗口对齐/分屏手势（Tauri 文档明确提示）；需处理「点击歌词跳转」与拖拽区的冲突。
- **工作量**：中低（前端为主，Rust 仅改窗口样式，约 1–2 天）。

### 方案 C：吸附主窗口标题栏的迷你桌面歌词窗

- **描述**：复用现有 `create_lyrics_window`（透明/置顶/鼠标穿透 + 光标转发线程），缩小并钉在主窗口标题栏位置，实时跟踪主窗口几何。
- **优点**：复用已有的透明窗 + 鼠标穿透转发机制，无需原生菜单栏 API。
- **缺点**：overlay 会覆盖红绿灯（须右移避让）；须持续监听主窗口 move/resize 同步几何（比方案 B 复杂）；多屏/全屏边角情况多；视觉上是「浮窗」而非原生。
- **工作量**：中（前端改小 + 几何同步逻辑，约 2 天）。

### 方案 D：Touch Bar 歌词 — 不推荐

- 2019 16" 及所有 Apple Silicon MacBook 已移除 Touch Bar，硬件覆盖极低；项目虽有 `touchBar` 设置项但属遗留。仅记录。

### （附）Now Playing / MPNowPlayingInfoCenter

- 仅显示元数据 + 封面于控制中心/锁屏，**不渲染歌词**，macOS 无歌词通道，不适用。

---

## 3. 方案对比

| 维度 | A 菜单栏 | B 统一标题栏 | C 迷你浮窗 | D Touch Bar |
|---|---|---|---|---|
| 平台覆盖 | 仅 macOS ✅ | macOS + 可降级 Win | 仅 macOS | 基本无 |
| 原生度 | 最高 | 高 | 中（浮窗感） | — |
| 侵入窗口布局 | 否 | 是（重排 Header） | 否 | 否 |
| 复用现状 | **前端已就绪** | 需新建 | 复用透明窗 | 无 |
| 新增原生代码 | Rust 桥接 | 窗口样式 | 几何同步 | objc |
| 工作量 | 中（2–3 天） | 中低（1–2 天） | 中（2 天） | 低但无意义 |
| 风险 | 低 | 中（手势/拖拽） | 中（几何跟踪） | 高（无硬件） |

---

## 4. 推荐

- 若「标题栏」指**系统菜单栏（屏幕顶栏）** → **选方案 A**。ROI 最高：前端成品已就绪，仅补 Rust 桥接即可让既有 `statusBarLyrics` 设置项生效；也是最贴合「标题栏歌词」用户心智的做法。
- 若「标题栏」指 **App 自身窗口标题栏** → **选方案 B**。风险最低、可跨平台，视觉最统一。
- **建议路径**：先落地方案 A（填补既有 stub，盘活已有前端与设置项），方案 B 作为可选增强后续迭代。

---

## 5. 落地路线（以方案 A 为主）

1. **依赖**：`src-tauri/Cargo.toml` 增加
   ```toml
   [target.'cfg(target_os = "macos")'.dependencies]
   objc2-app-kit = { version = "0.3", features = ["NSStatusItem", "NSImage", "NSColor", "NSWindow"] }
   objc2-foundation = "0.3"
   ```
2. **新建 `src-tauri/src/macos_statusbar.rs`**：
   - `create_status_item()`：`NSStatusItem`（Variable 长度），button 设 image（前端 dataURL → `NSImage`）。
   - 命令：`update_statusbar_image(data_url)` / `update_current_lyrics(text)` / `set_statusbar_visible(bool)`。
   - 深色模式适配、点击弹出菜单（暂停/下一首等，可选）。
3. **`ipc.rs`**：将第 211 行空 stub 改为调用 `macos_statusbar`（非 macOS 编译为空实现）。
4. **前端 `StatusBarLyrics.vue`**：把 `window.electron.ipcRenderer.send('update-statusbar-image', dataUrl)` 改为经 `tauriShim` 兼容的 `invoke('update_statusbar_image', { dataUrl })`，并补充 `update-current-lyrics` 文本通道。
5. **设置项**：`settings.js` 的 `statusBarLyrics` 已存在，直接复用开关，无需新建。
6. **验证**：`tauri dev` 下开启「状态栏歌词」，菜单栏出现滚动歌词；切歌/进度驱动实时更新；深色模式切换正常。

---

## 6. 风险与注意事项

- App 未常驻时菜单栏项随窗口退出消失（可接受；如需常驻可加登录项）。
- 菜单栏空间有限，超长文本已由前端 marquee 处理。
- `objc2` 版本须与 Tauri 2 对齐（文档示例用 `objc2-app-kit 0.3.x`）。
- 方案 B 的透明标题栏会失去系统分屏/对齐部分手势——需产品权衡。
- 本方案不影响已有的 Mac ATS 修复（`Info.plist` / `bundle.macOS.infoPlist`）与播放 `crossOrigin` 修复。

---

## 7. 一句话总结

「macOS 标题栏歌词」在技术上**高度可行且几乎是现成的**：前端 `StatusBarLyrics.vue` 已完整实现菜单栏歌词渲染，唯一缺口是 `src-tauri/src/ipc.rs:211` 那个被静默忽略的空 stub——补上一段 `objc2-app-kit` 的 `NSStatusItem` 桥接即可让 `statusBarLyrics` 设置项真正生效。
