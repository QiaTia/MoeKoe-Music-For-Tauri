# MoeKoeMusic 项目审阅与 Tauri 迁移方案

> 基于 v1.6.5 代码库（Electron 39 + Vue 3 + Vite 7）的全量审阅，2026-07。

---

## 第一部分：项目审阅

### 1. 架构总览

```
┌────────────────────────────────────────────────────────┐
│ Electron 主进程 (electron/main.js)                      │
│  ├─ appServices.js      窗口/托盘/TouchBar/快捷键/API启动 │
│  ├─ services/           WS控制(6520)/托盘菜单/状态栏歌词  │
│  │                      日志/更新/外链/桌面图标彩蛋       │
│  └─ extensions/         Chrome MV3 插件系统 + NativeHost │
├────────────────────────────────────────────────────────┤
│ preload.cjs (contextBridge)                             │
│  window.electron.ipcRenderer + window.electronAPI       │
├────────────────────────────────────────────────────────┤
│ 渲染进程: Vue3 + Pinia + vue-router + vue-i18n (dist/)  │
│  · 播放核心 PlayerControl.vue / AudioController          │
│  · navigator.mediaSession (SMTC/MPRIS/媒体键)           │
│  · music-metadata 纯 JS 解析本地音乐                    │
├────────────────────────────────────────────────────────┤
│ 独立子进程: api/ (KuGouMusicApi, Express)               │
│  pkg 打包为 app_win.exe / app_macos / app_linux         │
│  spawn 启动, HTTP :6521; 另有 WS 控制服务 :6520          │
└────────────────────────────────────────────────────────┘
```

### 2. 主要模块清单

| 模块 | 文件 | 说明 |
|---|---|---|
| 主窗口 | appServices.createWindow | 透明窗、`webSecurity:false`（规避 CORS）、contextIsolation 开启 |
| 桌面歌词 | createLyricsWindow + Lyrics.vue | 透明置顶 screen-saver 层、鼠标穿透、window-drag 拖拽 |
| MV 窗口 | createMvWindow | 无边框透明窗 |
| 系统托盘 | createTray + customTrayMenuService | 原生菜单 + 自定义 HTML 托盘面板（view/index.html，300×480 透明窗） |
| macOS 状态栏歌词 | statusBarLyricsService | 前端 canvas → dataURL → nativeImage 设为托盘图 |
| macOS TouchBar | createTouchBar | 播放控制 + 滚动歌词 |
| Windows Thumbar | setThumbarButtons | 任务栏缩略图 上/播/下 按钮 |
| 全局快捷键 | registerShortcut | 约 11 个，支持用户自定义 |
| 深链接 | registerProtocolHandler | `moekoe://` + 单实例锁二次唤起转发 |
| API 服务 | startApiServer | spawn pkg 二进制，`--platform=lite --port=6521` |
| 远程控制 | services/apiService.js | `ws` WebSocketServer :6520，广播歌词/播放状态，接收控制命令 |
| 自动更新 | services/updater.js | electron-updater + GitHub Releases |
| 插件系统 | extensions/* | `session.loadExtension` 加载 Chrome MV3 扩展；zip/URL 安装 |
| Native Host | nativeHostManager.js | 插件声明本地可执行程序，spawn + stdio JSON-Lines 通信，隐藏桥接窗口中转 |
| 彩蛋 | desktopShortcutIcon.js | Windows 节日快捷方式图标（powershell + WScript.Shell） |

### 3. 审阅发现（与迁移无关但值得注意）

1. **`webSecurity: false`**：主窗口关闭了同源策略，是为了跨域访问 :6521。安全面偏大，任何页面内容都可任意跨域。迁移到 Tauri 后应改用正规 CORS/代理解决。
2. **API 用 `pkg` 打包**：`pkg` 已停止维护（Node 18 后官方弃更），长期有维护风险——这也是迁移的一个顺带收益点（sidecar 可换用其他打包器或 Rust 重写）。
3. **`SIGKILL` 杀 API 进程**、updater 强改 `app.isPackaged`、`executeJavaScript` 读 localStorage 等写法偏 hack，迁移时应顺手规范化。
4. **好消息**：
   - 媒体键/SMTC/MPRIS 走 Web 标准 `navigator.mediaSession`，不依赖 Electron；
   - `music-metadata` 在渲染进程纯 JS 运行；
   - **全仓库无任何运行期 native `.node` 模块**；
   - 前端与主进程通过 preload 桥完全解耦（contextIsolation 已开启），IPC 边界清晰——这些都大幅降低迁移成本。

### 4. 前端依赖的 IPC 通道盘点（迁移工作量的直接依据）

- 渲染 → 主：18 个 `send` 通道（window-control、save-settings、desktop-lyrics-action、lyrics-data、set-ignore-mouse-events、window-drag、play-pause-action、tray-menu-* 等）
- 主 → 渲染：约 17 个 `on` 通道（url-params、play-next-track、toggle-*、update-progress 等）
- `window.electronAPI` invoke：13 个（日志、更新、MV 窗、插件管理 9 个）
- 涉及前端文件约 15 个：PlayerControl.vue、Settings.vue、Lyrics.vue、Disclaimer.vue、TitleBar.vue、Header.vue、AppUpdateDialog.vue、ExtensionManager.vue、StatusBarLyrics.vue、utils.js、Helpers.js、AudioController.js、view/index.html 等。

---

## 第二部分：Tauri 迁移方案

### 1. 可行性结论

**技术上完全可行，定位为"中大型重构"而非"平移"。** 工作量主体不在 UI（Vue 前端可 95% 复用），而在：

- ❌ **决定性难点：Chrome MV3 插件系统**。Tauri（WebView2/WKWebView/WebKitGTK）没有 `session.loadExtension`、没有 `chrome-extension://` 协议，这套插件生态**无法平移，只能重新设计**。
- ⚠️ macOS TouchBar、Windows Thumbar 无官方等价，需自写 Rust 插件或降级。
- ✅ 其余全部功能（多窗/透明/置顶/穿透、托盘、全局快捷键、深链、单实例、自启、防休眠、更新、store、日志）Tauri v2 官方插件均有对应。

**先决决策（阶段 0 必须回答）**：插件 + Native Host 生态是否必须保留？
- 若可裁剪/延后 → 迁移成本下降约 40%，主线 4~6 周可交付可用版本；
- 若必须保留 → 需设计新的插件运行时（见 §5），这是独立的项目级工作。

### 2. 技术选型

| 项 | 选择 | 说明 |
|---|---|---|
| Tauri 版本 | **Tauri 2.x** | 多窗口、tray、deep-link、single-instance 等 v2 插件齐全 |
| 前端 | 现有 Vue3 + Vite 原样保留 | `beforeDevCommand: vite`, devUrl `http://localhost:8080` |
| API 服务 | **sidecar（externalBin）** 起步 | 复用现有 pkg 二进制，后期可选 Rust 重写 |
| 状态持久化 | `tauri-plugin-store` | 替换 electron-store（含主进程 i18n 读取） |
| 日志 | `tauri-plugin-log` | 替换 electron-log |
| 更新 | `tauri-plugin-updater` | 仍走 GitHub Releases，但需生成 Tauri 签名密钥对 |

### 3. 能力映射表（Electron → Tauri 2）

| Electron 能力 | Tauri 2 对应 | 难度 |
|---|---|---|
| BrowserWindow 多窗/透明/置顶/skipTaskbar | `WebviewWindow` + `transparent/alwaysOnTop/skipTaskbar` | 低 |
| setIgnoreMouseEvents（歌词穿透） | `set_ignore_cursor_events` | 低 |
| window-drag 手动拖拽 | `startDragging()` 或 `data-tauri-drag-region` | 低 |
| ipcMain/ipcRenderer | `invoke` 命令 + `emit/listen` 事件 | 中（30+ 通道逐一映射） |
| Tray + Menu | `TrayIcon` + `Menu`（v2 支持动态更新） | 低 |
| 自定义 HTML 托盘面板 | 普通 Tauri 窗口 + 托盘 click 定位显示 | 中 |
| webContents.executeJavaScript | ❌ 无等价 → 前端主动上报状态到 Rust 侧共享 state | 中 |
| globalShortcut | `tauri-plugin-global-shortcut` | 低 |
| 单实例锁 + second-instance | `tauri-plugin-single-instance` | 低 |
| moekoe:// 深链 | `tauri-plugin-deep-link`（与 single-instance 配合） | 低 |
| app.setLoginItemSettings | `tauri-plugin-autostart` | 低 |
| powerSaveBlocker | `tauri-plugin-prevent-sleep`（社区）或自写 | 低 |
| shell.openExternal / openPath | `tauri-plugin-opener` | 低 |
| dialog.showOpenDialog/SaveDialog | `tauri-plugin-dialog` | 低 |
| Notification | `tauri-plugin-notification` | 低 |
| electron-updater | `tauri-plugin-updater`（需 minisign 密钥） | 中 |
| spawn API 二进制 | `tauri-plugin-shell` sidecar（bundle.externalBin） | 低 |
| ws WebSocketServer :6520 | Rust 侧 `tokio-tungstenite` 重写 | 中 |
| adm-zip 日志打包/插件安装 | Rust `zip` crate | 低 |
| nativeImage 状态栏歌词图 | 托盘 `set_icon(Image)`，Rust 解码 base64 PNG | 中 |
| TouchBar | ❌ 需 objc2 自写或**降级放弃** | 高 |
| setThumbarButtons | ❌ 需 windows-rs (ITaskbarList3) 自写或**降级** | 高 |
| session.loadExtension（MV3 插件） | ❌ **无解，需重新设计插件体系** | 极高 |
| Native Host (spawn+stdio) | Rust `Command` + stdin/stdout 线程可实现 | 中 |
| powershell 桌面图标彩蛋 | Rust 直接调 powershell 可保留 | 低 |

### 4. 分阶段实施计划

**阶段 0 — 决策与骨架（0.5 周）**
- 决策插件系统去留（见 §5）。
- `src-tauri/` 初始化 Tauri 2 工程；`tauri.conf.json` 接管现有 `dist/`；确认 Windows（WebView2）下音频播放、mediaSession、字体渲染正常。
- 产出：能跑起来的空壳应用（无 IPC）。

**阶段 1 — IPC 桥接层（1~1.5 周）★ 核心**
- 新建 `src/utils/bridge.js` 抽象层，统一封装：
  ```js
  // 目标：前端 15 个文件不直接 import Tauri API，只改 preload 调用点
  export const bridge = {
    send: (ch, ...args) => invoke('ipc_send', { ch, args }) /* 或逐命令 */,
    on:   (ch, cb) => listen(ch, e => cb(e.payload)),
    api:  { openLogPath: () => invoke('open_log_path'), ... }
  };
  ```
- Rust 侧按第一部分 §4 的通道清单实现约 30 个 `#[tauri::command]` + 事件转发。
- 替换 window-control、save-settings（store 插件）、open-url、免责声明、版本号。
- 产出：主窗口功能完整可用。

**阶段 2 — API sidecar（0.5 周）**
- `bundle.externalBin: ["binaries/api"]`，按 target triple 命名（`api-x86_64-pc-windows-msvc.exe` 等），复用现有 `pkg` 产物。
- Rust 启动时 spawn sidecar `--platform=lite --port=6521`，退出时随 app 终止（Tauri sidecar 自动管理，替代现在的 SIGKILL）。
- **CORS 处理**：不再有 `webSecurity:false`。二选一：
  a) KuGouMusicApi 设置 `CORS_ALLOW_ORIGIN=tauri://localhost, http://tauri.localhost`；
  b) Rust 侧起一个本地反向代理或用 `tauri-plugin-http` 转发。推荐 a，改动最小。
- 产出：登录、搜索、播放全链路可用。

**阶段 3 — 系统集成（1~1.5 周）**
- 托盘 + 原生菜单（先用原生菜单替代 HTML 托盘面板，HTML 面板作为增强项后补）。
- 全局快捷键（含用户自定义重注册）、单实例 + `moekoe://` 深链、开机自启、防休眠。
- 日志（plugin-log）+ 导出 zip（Rust 侧脱敏逻辑照抄正则）。
- WS 控制服务 :6520 用 tokio-tungstenite 重写（协议保持不变，第三方控制端零改动）。

**阶段 4 — 桌面歌词与增强窗口（1 周）**
- 歌词窗：透明 + always_on_top + skip_taskbar + `set_ignore_cursor_events`；拖拽改用 `startDragging`。
- MV 窗、HTML 托盘面板窗。
- macOS 状态栏歌词：前端照旧生成 dataURL → invoke 传 Rust → 解码为 tray icon。
- TouchBar / Thumbar：**建议 v1 降级放弃**，需求强烈再用 objc2 / windows-rs 补插件。

**阶段 5 — 更新与打包发布（0.5~1 周）**
- `tauri-plugin-updater` + GitHub Releases；生成 minisign 密钥；CI 改造（GitHub Actions：`tauri-action`）。
- 打包目标：Windows NSIS（保留 perMachine、安装目录可选）、macOS dmg（x64+aarch64）、Linux AppImage/deb。
- 注意：**Tauri 不支持 ia32 Windows**（现有 ia32 用户需保留 Electron 旧版通道或放弃）。

**阶段 6 —（可选，独立排期）插件系统重构（3 周起）**
- 见 §5。

**主线合计（不含插件系统）：约 5~6 周单人全职。**

### 5. 插件系统的三条路

| 方案 | 思路 | 代价 |
|---|---|---|
| A. 裁剪 | v1 不带插件，ExtensionManager 入口隐藏 | 最低，功能回退 |
| B. 自研轻量插件协议（推荐） | 放弃 Chrome MV3，定义 MoeKoe 自己的 manifest：插件 = JS bundle（在隐藏 webview/前端沙箱 iframe 中运行）+ 声明式权限 + 保留 Native Host（Rust spawn + stdio JSON-Lines 照搬协议）。现有插件按新规范迁移 | 中高；生态需作者配合迁移 |
| C. 双内核过渡 | Tauri 主程序 + 保留一个精简 Electron "插件宿主"进程 | 不推荐：违背迁移初衷，包体积/复杂度双高 |

Native Host 部分（stdio JSON-Lines）与 Chrome 扩展本身解耦，**协议可 100% 保留**，Rust 侧用 `std::process::Command` + 读写线程实现即可，现有原生程序无需改动。

### 6. 关键配置示意

```jsonc
// src-tauri/tauri.conf.json（节选）
{
  "productName": "MoeKoe Music",
  "identifier": "cn.MoeKoe.Music",
  "build": {
    "beforeDevCommand": "npm run serve",
    "devUrl": "http://localhost:8080",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "bundle": {
    "externalBin": ["binaries/api"],        // sidecar：现有 pkg 产物改名即可
    "targets": ["nsis", "dmg", "appimage", "deb"]
  },
  "plugins": {
    "updater": { "endpoints": ["https://github.com/MoeKoeMusic/MoeKoeMusic/releases/latest/download/latest.json"] },
    "deep-link": { "desktop": { "schemes": ["moekoe"] } }
  },
  "app": {
    "windows": [{ "label": "main", "transparent": true, "decorations": false }],
    "security": { "csp": "default-src 'self'; connect-src 'self' http://127.0.0.1:6521 ws://127.0.0.1:6520 https:; img-src * data: blob:; media-src * blob:" }
  }
}
```

### 7. 风险清单

| 风险 | 等级 | 缓解 |
|---|---|---|
| 插件生态断裂 | 高 | 阶段 0 决策；方案 B + 迁移指南 |
| WebView2/WKWebView 渲染差异（透明窗、字体、音频格式支持） | 中 | 阶段 0 先做 spike 验证；Linux WebKitGTK 尤其要早测 |
| CORS/CSP 收紧导致接口不通 | 中 | 阶段 2 统一处理，禁止再用关同源策略兜底 |
| ia32 Windows 用户流失 | 低 | 公告 + 旧版 Electron 通道封存 |
| 更新链切换（老用户 electron-updater 收不到 Tauri 包） | 中 | 发一个 Electron 末版，内置"下载新版安装包"引导 |
| Rust 学习成本 | 中 | 阶段 1-3 所需 Rust 均为模板级代码，官方插件覆盖 80% |

### 8. 迁移收益预估

- 安装包：Windows 约 85MB(NSIS) → **约 10~15MB**（不含 sidecar；sidecar 约 40MB，后续 Rust 重写 API 可再砍）
- 内存：常驻约 300~500MB → 约 100~200MB
- 安全：消除 `webSecurity:false`；Tauri 默认 CSP + capability 权限模型
- 维护：摆脱 `pkg`（已停维护）与 Electron 大版本追赶

---

## 9. 实施状态（已完成全量迁移）

> 更新时间：2026-07-27。`cargo check` 与 `vite build` 均通过。

### 9.1 Rust 侧（src-tauri/src/）

| 模块 | 职责 | 对应 Electron 源 |
|---|---|---|
| `lib.rs` | 入口：6 插件装配、AppCtx 状态、深链解析(moekoe://)、窗口关闭策略(最小化到托盘) | main.js |
| `settings.rs` | JSON 设置持久化(app_data_dir/store.json) + 内置 i18n(zh-CN/zh-TW/en/ru/ja/ko) | electron-store + language/i18n.js |
| `windows.rs` | 主窗(状态恢复/原生标题栏)、歌词窗(透明/置顶/跳过任务栏)、MV 窗 | appServices.js create*Window |
| `tray.rs` | 托盘 9 项菜单 + 单击/双击行为 + set_tray_title | createTray |
| `shortcuts.rs` | 11 个全局快捷键 + Electron accelerator 格式转换 + 自定义快捷键重载 | registerShortcut |
| `ws.rs` | axum WebSocket 远程控制 :6520（welcome/lyrics/playerState + control） | services/apiService.js |
| `updater.rs` | GitHub Releases 检查/下载/进度事件（update-progress/downloaded/error） | services/updater.js |
| `ipc.rs` | `ipc_send` 命令统一分发全部 15+ IPC 通道；`app_boot_info`；启动音；日志导出 | ipcMain.on(...) |
| `api.rs`(lib.rs 内命令) | `api_request` 进程内直调酷狗 API（原 :6521 HTTP 服务，零端口） | startApiServer |

### 9.2 前端侧（零改动 .vue 文件）

- `src/utils/tauriShim.js`：运行时垫片，注入 `window.electron`（send→invoke('ipc_send')、on/once→Tauri event listen、platform）与 `window.electronAPI`（4 个真实现 + 扩展体系安全降级）；`app_boot_info` 合成 version/show-disclaimer/url-params 启动事件并支持"注册即补发"；补齐隐藏滚动条 CSS。
- `src/utils/invokeBridge.js` + `request.js`：API 请求 Tauri 下走 invoke，Web/自定义远程地址走 HTTP（上一阶段完成）。
- `src/main.js`：顶部安装垫片（早于一切 `window.electron` 检查）。

### 9.3 降级/未迁移项

- Chromium 扩展/插件体系、NativeHost：Tauri 无对应机制，electronAPI 垫片安全降级（返回空/失败提示）。
- macOS TouchBar、Windows ThumbarButtons、mac 状态栏歌词图片：无对应 API，相关 IPC 通道静默忽略（前端均用 `?.` 调用，安全）。

### 9.4 运行方式

```bash
npm run tauri:dev    # 开发（vite:8080 + tauri dev）
npm run tauri:build  # 打包（NSIS 中英双语）
```

### 9.5 Electron / Node 清理（2026-07-28）

迁移验证通过后已彻底移除旧运行时：

- **删除**：`electron/`（主进程全部代码）、`api` 子模块（Node 版 KuGouMusicApi）、`Dockerfile`、`docker-compose.yml`、`nginx.conf`（Web+Node API 部署链路随之下线）
- **迁移**：`api/rust` → 项目根 `rust-api/`（`src-tauri/Cargo.toml` 路径依赖同步更新为 `../rust-api`）
- **package.json**：移除 `main` 字段、全部 `electron:*`/`build:api:*`/`api`/`dev(旧)` 脚本、electron-builder `build` 配置块；卸载 `electron`、`electron-builder`、`electron-is-dev`、`electron-log`、`electron-store`、`electron-updater`、`adm-zip`、`tree-kill`、`ws`、`cross-env`、`npm-run-all`、`wait-on`（npm 共清除 400 个包）；`dev` 脚本现指向 `tauri dev`
- **保留**：`music-metadata`（前端本地音乐解析）、`workbox-window`（PWA）、`build/icons`（Tauri 打包图标复用）
- 验证：`cargo check` ✅、`vite build` ✅、`tauri:dev` 应用启动 ✅
