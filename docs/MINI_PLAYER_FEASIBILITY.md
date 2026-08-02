# MoeKoe Music「小屏/迷你播放器模式」可行性分析报告

> 范围：评估在 Tauri v2 桌面端实现类似网易云音乐「小屏模式」的可行性，给出可选方案、对比与落地路线。  
> 日期：2026-08-01

---

## 1. 需求定义（基于截图）

用户给出的三张截图展示了网易云音乐经典的两种「小屏模式」：

| 形态          | 尺寸估算                   | 界面元素                                        | 使用场景           |
| ----------- | ---------------------- | ------------------------------------------- | -------------- |
| **小型完整播放器** | 约 280–340 × 320–400 px | 大封面、歌曲名/歌手、播放/暂停、上一首/下一首、进度条、喜欢、歌词、播放列表、音量等 | 需要完整控制但占用桌面空间小 |
| **迷你横条播放器** | 约 280–340 × 50–70 px   | 小封面、歌曲名/歌手、播放/暂停、上一首/下一首、进度条（可选）            | 常驻桌面角落，最省空间    |
| **两种形态切换**  | 同一窗口内通过按钮切换            | 小屏模式 ↔ 迷你模式一键切换                             | 用户按需切换         |

核心诉求可归纳为：

1. **独立于主窗口**：小屏模式是一个单独的、可拖拽的小窗口，不依赖主窗口是否显示。
2. **两种尺寸形态**：可在大一点的「小屏播放器」和更小的「迷你横条」之间切换。
3. **完整播放控制**：播放/暂停、切歌、进度拖拽、音量、喜欢等关键操作可直接在小窗完成。
4. **与主播放器状态实时同步**：当前歌曲、播放进度、播放状态、歌词等数据与主窗口共享。
5. **可置顶/可移动**：小窗口通常需要置顶，并允许用户自由拖拽位置。

---

## 2. 现状评估

### 2.1 项目已有能力

本项目（MoeKoe-Music-For-Tauri）基于 **Tauri v2 + Vue 3 + WebView2/WebKit**，已经具备实现小屏模式所需的大部分基础设施：

| 能力       | 现状    | 说明                                                           |
| -------- | ----- | ------------------------------------------------------------ |
| 多窗口管理    | ✅ 已成熟 | `src-tauri/src/windows.rs` 已实现 `main` / `lyrics` / `mv` 三类窗口 |
| 独立歌词窗    | ✅ 已成熟 | `create_lyrics_window`：透明、置顶、跳过任务栏、持久化位置尺寸                   |
| 播放状态     | ✅ 已成熟 | `PlayerControl.vue` + `AudioController.js` 集中管理播放状态          |
| IPC 通信   | ✅ 已成熟 | `src-tauri/src/ipc.rs` 已封装 15+ 通道，前端通过 `tauriShim` 调用        |
| 路由系统     | ✅ 已成熟 | `src/router/router.js` 使用 hash 路由，可新增 `/mini` 路由             |
| 窗口状态持久化  | ✅ 已成熟 | `windowState` / `lyricsWindowPosition` 已有存储机制                |
| 全局快捷键/托盘 | ✅ 已成熟 | 已有托盘菜单与全局快捷键                                                 |

### 2.2 关键发现

1. **多窗口机制完全可行**。Tauri 2 的 `WebviewWindowBuilder` 支持创建任意数量的独立窗口，项目已有 `main` / `lyrics` / `mv` 三个标签窗口，新增一个 `mini` 窗口毫无技术障碍。
2. **状态同步有现成链路**。前端可以通过已有的 `ipc_send` 通道把播放状态广播到所有窗口；也可以在小窗路由内复用同一套 Pinia/Vuex store（如果项目使用全局状态管理）。
3. **UI 复用度高**。`PlayerControl.vue` 已经包含播放控制、进度条、音量、喜欢、播放列表等全部元素，小屏模式可基于该组件做尺寸裁剪与重新布局。
4. **平台差异可控**。Tauri 的窗口 API 在 Windows / macOS / Linux 上基本一致，透明、置顶、跳过任务栏、尺寸限制等属性跨平台可用。

> 结论：实现「小屏/迷你播放器模式」在项目当前技术栈下**高度可行**，主要工作是新增一个独立窗口 + 一个专用前端路由/组件，并补齐状态同步与窗口生命周期管理。

---

## 3. 可选技术方案

### 方案 A：独立小窗模式（推荐主方案）

**描述**：新增一个标签为 `mini` 的独立 `WebviewWindow`，加载前端路由 `/mini`。该窗口专门渲染精简版播放器，支持两种尺寸切换（小屏 ↔ 迷你）。

**技术实现**：

- **Rust 侧**（`src-tauri/src/windows.rs`）：
  - 新增 `pub const MINI: &str = "mini";`
  - 新增 `create_mini_window(app, store)`：
    - 尺寸：默认小屏模式约 `320 × 380`，迷你模式约 `320 × 70`。
    - 属性：`.decorations(false)`（自定义标题栏，节省空间）、`.transparent(true)`、`.always_on_top(true)`、`.skip_taskbar(true)`、`.resizable(false)` 或限制最小/最大尺寸。
    - 位置：从 `store.get("miniWindowState")` 恢复，默认右下角或屏幕中央。
    - Windows 额外参数：`.additional_browser_args("--allow-mixed-content")`。
  - 新增 `toggle_mini_mode(app, mode)`：切换窗口尺寸时调用 `set_size`，并持久化当前模式。
  - 新增 `close_mini_window(app)`。
- **前端侧**：
  - 新增 `src/views/MiniPlayer.vue`：
    - 复用 `PlayerControl.vue` 的播放状态与控制逻辑。
    - 提供两种布局：小屏模式显示大封面 + 完整控制；迷你模式只显示小封面 + 基础控制。
    - 顶部/边缘放置「切换形态」按钮、关闭按钮、拖拽区域（`data-tauri-drag-region`）。
  - `src/router/router.js` 新增 `{ path: '/mini', name: 'MiniPlayer', component: MiniPlayer }`。
  - 在 `App.vue` 中判断当前路由为 `/mini` 时，不渲染 `TitleBar`、`Disclaimer`、`StatusBarLyrics`。
- **状态同步**：
  - 小窗与主窗共享播放状态。若项目使用 Pinia/Vuex，直接复用同一 store 实例即可（同一进程内多窗口共享 JS 运行时）。
  - 若不共享 store，可通过 `ipc_send` 通道让 Rust 侧转发播放状态事件（`play-progress`、`play-state-changed`、`song-changed`）到小窗。
- **入口**：
  - 主窗口播放器控制栏新增「小屏模式」按钮，调用 `invoke('open_mini_window')`。
  - 托盘菜单新增「打开小屏播放器」选项。
  - 全局快捷键（可选）绑定切换小屏模式。

**优点**：

- 最贴合网易云音乐截图效果。
- 独立窗口，不干扰主窗口布局与状态。
- 两种形态在同一窗口内切换，动画/过渡自然。
- 复用现有播放控制组件与状态管理，前端成本低。
- 跨平台一致性最好（Windows / macOS / Linux 行为一致）。

**缺点/风险**：

- 需要新增一个 Webview 实例，内存占用略有增加（约 30–60 MB，现代桌面可接受）。
- 无边框窗口的拖拽、缩放、关闭需要前端自己实现。
- 多窗口间状态同步需仔细设计，避免循环更新。
- 透明窗口在某些 Linux 桌面环境下可能有渲染问题。

**工作量**：中（约 2–3 天）。

---

### 方案 B：主窗口变形模式

**描述**：不创建新窗口，而是通过改变主窗口尺寸、隐藏侧边栏/Header、切换路由到 `/mini`，让主窗口本身变成小屏播放器。

**技术实现**：

- 前端新增 `/mini` 路由与 `MiniPlayer.vue`。
- 调用 Tauri API `window.setSize(320, 380)` 将主窗口缩到小屏尺寸。
- `App.vue` 中检测到 `/mini` 路由时隐藏 `TitleBar`、`Disclaimer`、`StatusBarLyrics`。
- 提供「恢复」按钮，将窗口尺寸还原并切回主界面路由。

**优点**：

- 不需要额外 Webview 实例，内存占用最低。
- 实现简单，无需管理第二个窗口生命周期。

**缺点/风险**：

- 主窗口被缩小后，用户无法同时使用完整主界面（不符合截图中「主界面 + 小窗并存」的直觉）。
- 窗口缩放动画、位置调整体验不如独立小窗自然。
- 如果用户关闭小屏模式再开主界面，需要恢复之前的位置尺寸，状态管理复杂。
- 与网易云截图的「额外小窗口」形态不一致。

**工作量**：低（约 1 天）。

> 评价：可作为备选或快捷入口，但不建议作为主要产品形态。

---

### 方案 C：系统级画中画 / 紧凑覆盖模式

**描述**：利用操作系统原生能力实现小窗：

- **Windows**：Compact Overlay / Picture-in-Picture API（UWP 特性，传统 Win32 程序支持有限）。
- **macOS**：`NSPanel` 或自定义 `NSWindow` 配合 `NSWindowStyleMask`。

**技术实现**：

- 需要大量平台特定代码（`objc2-app-kit` 或 Windows API）。
- 小窗内容仍可用 WebView 渲染，但窗口容器需要原生实现。

**优点**：

- 原生体验最好，可集成到系统画中画体系。
- 可能获得系统级的置顶/缩放/吸附手势支持。

**缺点/风险**：

- 跨平台实现差异极大，Tauri 2 没有高层抽象，需要手写大量原生代码。
- Windows 的 Compact Overlay 对非 UWP/非 MSIX 应用支持不佳。
- 开发周期长，维护成本高。
- 超出项目现有能力边界（目前连 `statusbar.rs` 这类简单原生桥接都需要数日）。

**工作量**：高（约 1–2 周）。

> 评价：技术可行但 ROI 低，不建议作为首选。

---

### 方案 D：托盘弹出面板模式

**描述**：点击托盘图标时弹出一个迷你播放控制面板。

**技术实现**：

- Tauri 2 的 `TrayIcon` 支持点击事件，但**不支持直接嵌入复杂 UI 面板**。
- 实际实现需要在托盘位置附近创建一个临时小窗口，点击外部后自动关闭。

**优点**：

- 不常驻桌面，最轻量。

**缺点/风险**：

- 无法持续显示歌曲信息/进度。
- 与截图中「常驻小窗口」形态差距大。
- 托盘位置在不同平台/显示器上难以精确计算。

**工作量**：中（约 2 天）。

> 评价：适合作为快捷控制入口，不适合作为完整的小屏模式。



---

## 4. 方案对比

| 维度     | 方案 A：独立小窗        | 方案 B：主窗变形 | 方案 C：系统画中画 | 方案 D：托盘面板 |
| ------ | ---------------- | --------- | ---------- | --------- |
| 贴合截图效果 | ⭐⭐⭐ 高            | ⭐⭐ 中      | ⭐⭐⭐ 高      | ⭐ 低       |
| 跨平台一致性 | ⭐⭐⭐ 高            | ⭐⭐⭐ 高     | ⭐ 低        | ⭐⭐ 中      |
| 实现复杂度  | ⭐⭐ 中             | ⭐ 低       | ⭐⭐⭐ 高      | ⭐⭐ 中      |
| 内存占用   | ⭐⭐ 中（新增 Webview） | ⭐ 低       | ⭐⭐⭐ 高      | ⭐ 低       |
| 与主窗并存  | ✅ 可以             | ❌ 不行      | ✅ 可以       | ✅ 可以      |
| 状态同步难度 | ⭐⭐ 中             | ⭐ 低       | ⭐⭐⭐ 高      | ⭐⭐ 中      |
| 维护成本   | ⭐⭐ 中             | ⭐ 低       | ⭐⭐⭐ 高      | ⭐⭐ 中      |
| 推荐度    | **首选**           | 备选        | 不推荐        | 辅助入口      |

---

## 5. 推荐方案

**推荐方案 A（独立小窗模式）作为主要产品形态**，理由：

1. **最贴合用户截图**：网易云音乐的小屏模式就是独立小窗口，可常驻桌面、可拖拽、可与主窗口并存。
2. **技术最稳妥**：完全基于 Tauri 2 已有窗口能力，不需要系统原生 API，跨平台一致性好。
3. **复用度高**：可直接复用 `windows.rs` 的窗口创建模式、`PlayerControl.vue` 的播放状态与控制逻辑。
4. **扩展性好**：未来可在小窗内继续增加歌词、评论、相似推荐等模块，而不影响主窗口。

**建议同时补充方案 D 的部分能力**：在托盘菜单增加「打开小屏播放器」入口，作为快捷触发方式。

---

## 6. 落地路线（以方案 A 为例）

### 6.1 第一阶段：最小可用版本（MVP，约 2 天）

1. **新增窗口标签与创建函数**（`src-tauri/src/windows.rs`）
   - 新增 `pub const MINI: &str = "mini";`
   - 新增 `create_mini_window(app, store)`：
     - 默认尺寸 320 × 380（小屏模式）。
     - 无边框、透明、置顶、跳过任务栏。
     - 恢复上次位置（`miniWindowState`）。
   - 新增 `toggle_mini_size(app, mode)` / `close_mini_window(app)`。
2. **新增 IPC 命令**（`src-tauri/src/ipc.rs` + `lib.rs`）
   - 新增 `open_mini_window` / `close_mini_window` / `toggle_mini_mode` 命令。
   - 在 `ipc_send` 的 `window-control` 分支中处理前端主动切换/关闭小窗的请求。
3. **新增前端路由与视图**
   - `src/views/MiniPlayer.vue`：小屏模式主视图。
   - `src/router/router.js` 新增 `/mini` 路由。
   - `src/components/mini/MiniModeToggle.vue`：小屏/迷你形态切换按钮。
4. **UI 布局**
   - 复用 `PlayerControl.vue` 的播放状态（`currentSong`、`playing`、`progress` 等）。
   - 小屏模式：大封面 + 歌曲信息 + 完整控制栏。
   - 迷你模式：小封面 + 歌曲名/歌手 + 播放/暂停/切歌 + 进度条。
5. **入口**
   - 主窗口 `PlayerControl.vue` 新增「小屏模式」按钮。
   - 托盘菜单新增「打开小屏播放器」。

### 6.2 第二阶段：体验优化（约 1 天）

1. **状态同步优化**
   - 确保小窗与主窗的播放进度、播放状态、当前歌曲实时同步。
   - 如果项目使用全局 store，直接共享；否则通过 Rust 转发事件。
2. **窗口行为**
   - 小窗失去焦点时保持置顶（`always_on_top(true)`）。
   - 关闭小窗时仅隐藏，不销毁（下次打开秒开），或在隐藏一段时间后自动销毁释放资源。
   - 小窗关闭/显示状态持久化到 `settings`。
3. **拖拽与自定义标题栏**
   - 使用 `data-tauri-drag-region` 实现无边框窗口拖拽。
   - 提供关闭、最小化、切换形态按钮。
4. **开机自启/恢复**
   - 若用户设置「启动时打开小屏模式」，在 `lib.rs` 的 `setup` 中按需创建。

### 6.3 第三阶段：高级增强（可选，约 1–2 天）

1. **歌词浮层**：在小屏模式封面区域叠加当前歌词行。
2. **毛玻璃背景**：根据专辑封面取色或系统主题适配。
3. **吸附边缘**：小窗靠近屏幕边缘时自动吸附。
4. **全局快捷键**：绑定 `Ctrl/Cmd + Shift + M` 切换小屏模式。

---

## 7. 关键代码示例

### 7.1 Rust：创建小窗

```rust
pub const MINI: &str = "mini";

pub fn create_mini_window(app: &AppHandle, store: &SettingsStore) -> tauri::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(MINI) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(existing);
    }

    let (screen_w, screen_h) = primary_work_area(app);
    let state = store.get("miniWindowState");
    let mode = state.get("mode").and_then(|v| v.as_str()).unwrap_or("normal");

    let (width, height) = if mode == "compact" {
        (320.0, 70.0)
    } else {
        (320.0, 380.0)
    };

    let x = state.get("x").and_then(|v| v.as_f64()).unwrap_or(screen_w - width - 20.0);
    let y = state.get("y").and_then(|v| v.as_f64()).unwrap_or(screen_h - height - 60.0);

    let mut builder = WebviewWindowBuilder::new(app, MINI, WebviewUrl::App("index.html#/mini".into()))
        .title("MoeKoe Mini")
        .inner_size(width, height)
        .min_inner_size(280.0, 60.0)
        .max_inner_size(400.0, 450.0)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .resizable(false)
        .visible(true);

    #[cfg(target_os = "windows")]
    {
        builder = builder.additional_browser_args("--allow-mixed-content");
    }

    builder.build()
}
```

### 7.2 前端：MiniPlayer.vue 结构

```vue
<template>
  <div class="mini-player" :class="{ 'compact': isCompact }">
    <div class="drag-bar" data-tauri-drag-region>
      <button @click="toggleMode">{{ isCompact ? '▣' : '▭' }}</button>
      <button @click="close">✕</button>
    </div>

    
    <template v-if="!isCompact">
      <img class="cover" :src="currentSong.img" />
      <div class="song-info">{{ currentSong.name }} — {{ currentSong.author }}</div>
      <MiniProgressBar />
      <MiniControls />
    </template>

    
    <template v-else>
      <img class="cover-small" :src="currentSong.img" />
      <div class="song-info">
        <div class="name">{{ currentSong.name }}</div>
        <div class="artist">{{ currentSong.author }}</div>
      </div>
      <MiniControls :compact="true" />
    </template>
  </div>
</template>
```

### 7.3 状态同步

如果项目使用 Pinia：

```js
// 小窗与主窗共享同一 store 实例
import { usePlayerStore } from '@/stores/player';
const player = usePlayerStore();
// currentSong、playing、progress 自动同步
```

如果不共享 store，通过 IPC 广播：

```rust
// Rust 侧在播放状态变化时广播到 mini 窗口
app.emit_to(MINI, "player-state", state)?;
```

---

## 8. 风险与注意事项

| 风险               | 影响 | 缓解措施                                        |
| ---------------- | -- | ------------------------------------------- |
| 多窗口状态不同步         | 高  | 使用全局 store 或 Rust 中转事件；避免双向循环更新             |
| 透明窗口在 Linux 渲染异常 | 中  | 提供 fallback：Linux 下关闭透明，使用纯色背景              |
| 无边框窗口拖拽体验差       | 中  | 明确拖拽区域（`data-tauri-drag-region`），保留足够的可拖拽空间 |
| 小窗被其他窗口遮挡        | 低  | `always_on_top(true)` 保持置顶，失去焦点仍可见          |
| 内存占用增加           | 低  | 小窗隐藏时不销毁可秒开；长期隐藏后自动关闭释放资源                   |
| 与现有桌面歌词窗冲突       | 低  | 两个独立窗口，互不影响；可同时存在                           |
| 不同 DPI/缩放比例适配    | 中  | 使用逻辑像素（LogicalSize/LogicalPosition）持久化位置尺寸  |
| macOS ATS 网络图片   | 低  | 已修复（`Info.plist`），小窗内封面加载不受影响               |

---

## 9. 一句话总结

在 MoeKoe Music 当前 Tauri v2 架构下，实现类似网易云音乐的「小屏/迷你播放器模式」**技术成熟、风险可控、高度可行**。推荐采用**独立小窗方案（方案 A）**：新增一个无边框、透明、置顶的 `mini` 窗口加载 `/mini` 路由，复用现有播放控制组件与状态管理，约 2–3 天即可完成最小可用版本，并可在未来继续扩展歌词、取色背景、边缘吸附等增强功能。
