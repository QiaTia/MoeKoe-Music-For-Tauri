// src/utils/tauriShim.js
// Tauri 环境下的 Electron 兼容垫片。
//
// 目标：让现有 15+ 个直接使用 window.electron / window.electronAPI 的 .vue 文件
// 在 Tauri 下零改动运行。
//
// 映射关系：
// - window.electron.ipcRenderer.send(channel, ...args) → invoke('ipc_send', { channel, args })
// - window.electron.ipcRenderer.on/once(channel, fn)   → Tauri event listen(channel)
//   （Rust 侧统一用 app.emit_to(window, channel, payload) 发事件，payload 为单值）
// - window.electron.platform                            → win32 / darwin / linux
// - window.electronAPI.startUpdateDownload/openMvWindow/openLogPath/exportLog → 对应 Rust 命令
// - 启动事件 version / show-disclaimer / url-params      → 调 app_boot_info 命令合成，
//   并在监听器注册时补发（解决 "Rust 发事件早于 Vue mounted 注册监听" 的时序问题）

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

function isTauriEnv() {
    return typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
}

function detectPlatform() {
    const ua = navigator.userAgent;
    if (/Macintosh|Mac OS X/i.test(ua)) return 'darwin';
    if (/Windows/i.test(ua)) return 'win32';
    return 'linux';
}

export function installTauriShim() {
    if (!isTauriEnv() || window.electron) return;

    const platform = detectPlatform();

    // ---- 事件桥 ----
    // channel → Map<原始listener, { unlisten: Promise<fn>, once: boolean }>
    const listenerRegistry = new Map();
    // 启动阶段合成事件的缓存：channel → payload[]，监听器注册时补发一次
    const pendingBootEvents = new Map();

    const fakeEvent = { senderId: 0, preventDefault() { } };

    function addListener(channel, listener, once) {
        let channelMap = listenerRegistry.get(channel);
        if (!channelMap) {
            channelMap = new Map();
            listenerRegistry.set(channel, channelMap);
        }
        const entry = { once, disposed: false, unlisten: null };
        entry.unlistenPromise = listen(channel, (event) => {
            if (entry.disposed) return;
            if (once) removeListener(channel, listener);
            try {
                listener(fakeEvent, event.payload);
            } catch (e) {
                console.error(`[tauriShim] listener error on '${channel}':`, e);
            }
        }).then((un) => { entry.unlisten = un; return un; });
        channelMap.set(listener, entry);

        // 补发启动阶段缓存的事件（version / show-disclaimer / url-params）
        const pending = pendingBootEvents.get(channel);
        if (pending && pending.length) {
            pendingBootEvents.delete(channel);
            for (const payload of pending) {
                queueMicrotask(() => {
                    if (!entry.disposed) {
                        if (once) removeListener(channel, listener);
                        try { listener(fakeEvent, payload); } catch (e) { console.error(e); }
                    }
                });
            }
        }
    }

    function removeListener(channel, listener) {
        const channelMap = listenerRegistry.get(channel);
        if (!channelMap) return;
        const entry = channelMap.get(listener);
        if (!entry) return;
        entry.disposed = true;
        channelMap.delete(listener);
        if (entry.unlisten) entry.unlisten();
        else entry.unlistenPromise.then((un) => un());
    }

    function removeAllListeners(channel) {
        const channelMap = listenerRegistry.get(channel);
        if (!channelMap) return;
        for (const listener of [...channelMap.keys()]) {
            removeListener(channel, listener);
        }
        listenerRegistry.delete(channel);
    }

    /** 合成一个"主进程事件"，若尚无监听器则缓存等待补发 */
    function dispatchBootEvent(channel, payload) {
        const channelMap = listenerRegistry.get(channel);
        if (channelMap && channelMap.size) {
            for (const [listener, entry] of [...channelMap.entries()]) {
                if (entry.disposed) continue;
                if (entry.once) removeListener(channel, listener);
                try { listener(fakeEvent, payload); } catch (e) { console.error(e); }
            }
        } else {
            if (!pendingBootEvents.has(channel)) pendingBootEvents.set(channel, []);
            pendingBootEvents.get(channel).push(payload);
        }
    }

    // ---- window.electron ----
    window.electron = {
        ipcRenderer: {
            send: (channel, ...args) => {
                invoke('ipc_send', { channel, args }).catch((e) => {
                    console.error(`[tauriShim] ipc_send '${channel}' failed:`, e);
                });
            },
            on: (channel, listener) => addListener(channel, listener, false),
            once: (channel, listener) => addListener(channel, listener, true),
            removeListener,
            removeAllListeners,
            // 请求/响应型 IPC（对应 Electron ipcRenderer.invoke）。
            // 目前仅桌面歌词悬停轮询用到 `lyrics-window-pointer-state`。
            invoke: (channel, ...args) => invoke('ipc_invoke', { channel, args }),
        },
        platform,
    };

    // ---- window.electronAPI ----
    const noopAsync = async () => undefined;
    window.electronAPI = {
        startUpdateDownload: () => invoke('start_update_download'),
        openMvWindow: (url) => invoke('open_mv_window', { url }),
        openLogPath: () => invoke('open_log_path'),
        exportLog: () => invoke('export_log'),
        // 插件/扩展体系依赖 Chromium 扩展机制，Tauri 下暂不支持 → 安全降级
        getExtensions: async () => [],
        getExtensionsDetailed: async () => [],
        reloadExtensions: async () => [],
        openExtensionsDir: noopAsync,
        openExtensionPopup: noopAsync,
        installExtension: noopAsync,
        uninstallExtension: noopAsync,
        validateExtension: async () => ({ valid: false, error: 'Tauri 版本暂不支持浏览器扩展' }),
        getExtensionsDirectory: async () => '',
        ensureExtensionsDirectory: noopAsync,
        installPluginFromZip: async () => ({ success: false, error: 'Tauri 版本暂不支持浏览器扩展' }),
        installPluginFromUrl: async () => ({ success: false, error: 'Tauri 版本暂不支持浏览器扩展' }),
        setNativeHostAuthorization: noopAsync,
        nativeHost: {
            getStatus: async () => ({ running: false }),
            send: noopAsync,
            onMessage: () => () => { },
        },
        showOpenDialog: async () => ({ canceled: true, filePaths: [] }),
    };

    // ---- 启动信息：合成 version / show-disclaimer / url-params 事件 ----
    invoke('app_boot_info')
        .then((info) => {
            if (!info) return;
            if (info.version) dispatchBootEvent('version', info.version);
            if (info.showDisclaimer) dispatchBootEvent('show-disclaimer', null);
            if (info.urlParams) dispatchBootEvent('url-params', info.urlParams);
        })
        .catch((e) => console.error('[tauriShim] app_boot_info failed:', e));

    // ---- 桌面歌词窗：模拟 Electron setIgnoreMouseEvents 的 forward 行为 ----
    // 窗口处于鼠标穿透状态时收不到任何原生 mousemove，Rust 侧轮询光标位置
    // 并通过 shim-mousemove 事件送入，这里合成真实 mousemove 派发到命中元素，
    // 使 Lyrics.vue 的悬停检测 / 拖拽逻辑零改动可用。
    if (window.location.hash.startsWith('#/lyrics')) {
        listen('shim-mousemove', ({ payload }) => {
            const { x, y, sx, sy } = payload || {};
            const target =
                (x >= 0 && y >= 0 && document.elementFromPoint(x, y)) || document.body;
            if (!target) return;
            target.dispatchEvent(
                new MouseEvent('mousemove', {
                    bubbles: true,
                    cancelable: true,
                    clientX: x,
                    clientY: y,
                    screenX: sx,
                    screenY: sy,
                })
            );
        });
    }

    // Electron 注入的隐藏滚动条样式在 Tauri 下等价补齐
    const style = document.createElement('style');
    style.textContent = '::-webkit-scrollbar{display:none;}';
    document.head.appendChild(style);

    console.log('[tauriShim] Electron 兼容层已注入 (platform=' + platform + ')');
}
