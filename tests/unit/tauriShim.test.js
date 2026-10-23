// tests/unit/tauriShim.test.js
// Electron → Tauri 兼容垫片。这是迁移接缝上最脆的一层：
// 通道映射错一个字母、once 语义退化成 on、启动事件丢失，都会表现为「功能静默失效」。
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { installTauriShim } from '@/utils/tauriShim';

const UA = {
  win: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
  mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
  linux: 'Mozilla/5.0 (X11; Linux x86_64)',
};

const originalUA = navigator.userAgent;
/** channel -> [cb, ...]，模拟 Tauri event 总线 */
let bus;

function setUA(value) {
  Object.defineProperty(navigator, 'userAgent', { configurable: true, value });
}

beforeEach(() => {
  bus = new Map();
  invoke.mockReset().mockResolvedValue(undefined);
  listen.mockReset().mockImplementation((channel, cb) => {
    const list = bus.get(channel) || [];
    list.push(cb);
    bus.set(channel, list);
    return Promise.resolve(() => {
      bus.set(channel, (bus.get(channel) || []).filter((fn) => fn !== cb));
    });
  });
  delete window.electron;
  delete window.electronAPI;
  delete window.__TAURI_INTERNALS__;
  setUA(UA.win);
});

afterAll(() => setUA(originalUA));

/** 在模拟的 Rust 总线上发一条事件 */
function rustEmit(channel, payload) {
  (bus.get(channel) || []).forEach((cb) => cb({ payload }));
}

function enableTauri(bootInfo) {
  window.__TAURI_INTERNALS__ = { invoke: vi.fn() };
  if (bootInfo !== undefined) {
    invoke.mockImplementation((cmd) =>
      cmd === 'app_boot_info' ? Promise.resolve(bootInfo) : Promise.resolve(undefined)
    );
  }
  installTauriShim();
}

describe('注入条件', () => {
  it('非 Tauri 环境不注入兼容层（纯 Web 预览不受影响）', () => {
    installTauriShim();
    expect(window.electron).toBeUndefined();
    expect(window.electronAPI).toBeUndefined();
  });

  it('Tauri 环境注入 electron 与 electronAPI', () => {
    enableTauri();
    expect(window.electron).toBeDefined();
    expect(window.electronAPI).toBeDefined();
    expect(typeof window.electron.ipcRenderer.send).toBe('function');
  });

  it('已注入过则跳过（幂等，避免重复注册监听）', () => {
    window.electron = { marker: true };
    enableTauri();
    expect(window.electron.marker).toBe(true);
  });
});

describe('platform 探测', () => {
  it('Windows → win32', () => {
    setUA(UA.win);
    enableTauri();
    expect(window.electron.platform).toBe('win32');
  });

  it('macOS → darwin', () => {
    setUA(UA.mac);
    enableTauri();
    expect(window.electron.platform).toBe('darwin');
  });

  it('其他 → linux', () => {
    setUA(UA.linux);
    enableTauri();
    expect(window.electron.platform).toBe('linux');
  });
});

describe('ipcRenderer.send → invite ipc_send', () => {
  it('把 channel 与可变参数原样透传给 Rust', () => {
    enableTauri();
    window.electron.ipcRenderer.send('set-thumbnail', { a: 1 }, 'x');
    expect(invoke).toHaveBeenCalledWith('ipc_send', {
      channel: 'set-thumbnail',
      args: [{ a: 1 }, 'x'],
    });
  });

  it('invoke 失败不向上抛（保持 Electron 的 fire-and-forget 语义）', () => {
    enableTauri();
    invoke.mockRejectedValueOnce(new Error('ACL denied'));
    expect(() => window.electron.ipcRenderer.send('x')).not.toThrow();
  });
});

describe('事件桥：on / once / removeListener', () => {
  it('on 把 Rust payload 作为第二参回调，第一参是兼容用的 fakeEvent', () => {
    enableTauri();
    const cb = vi.fn();
    window.electron.ipcRenderer.on('update-progress', cb);
    rustEmit('update-progress', 42);
    expect(cb).toHaveBeenCalledWith(expect.objectContaining({ senderId: 0 }), 42);
  });

  it('on 可重复触发', () => {
    enableTauri();
    const cb = vi.fn();
    window.electron.ipcRenderer.on('tick', cb);
    rustEmit('tick', 1);
    rustEmit('tick', 2);
    expect(cb).toHaveBeenCalledTimes(2);
  });

  it('once 只触发一次', () => {
    enableTauri();
    const cb = vi.fn();
    window.electron.ipcRenderer.once('ready', cb);
    rustEmit('ready', 'first');
    rustEmit('ready', 'second');
    expect(cb).toHaveBeenCalledTimes(1);
    expect(cb).toHaveBeenCalledWith(expect.anything(), 'first');
  });

  it('removeListener 后不再收到事件', () => {
    enableTauri();
    const cb = vi.fn();
    window.electron.ipcRenderer.on('ch', cb);
    rustEmit('ch', 1);
    window.electron.ipcRenderer.removeListener('ch', cb);
    rustEmit('ch', 2);
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it('removeAllListeners 清空该 channel', () => {
    enableTauri();
    const a = vi.fn();
    const b = vi.fn();
    window.electron.ipcRenderer.on('ch', a);
    window.electron.ipcRenderer.on('ch', b);
    window.electron.ipcRenderer.removeAllListeners('ch');
    rustEmit('ch', 1);
    expect(a).not.toHaveBeenCalled();
    expect(b).not.toHaveBeenCalled();
  });

  it('监听器内部抛错不会中断其余监听器', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    enableTauri();
    const bad = vi.fn(() => { throw new Error('boom'); });
    const good = vi.fn();
    window.electron.ipcRenderer.on('ch', bad);
    window.electron.ipcRenderer.on('ch', good);
    rustEmit('ch', 1);
    expect(good).toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalled();
    errSpy.mockRestore();
    void errSpy;
  });
});

describe('启动事件补发（解决 Rust 早于 Vue mounted 发事件的时序问题）', () => {
  it('监听器晚注册时仍能收到 version', async () => {
    enableTauri({ version: '1.7.1' });
    await new Promise((r) => setTimeout(r, 0)); // 等 app_boot_info resolve 并缓存

    const cb = vi.fn();
    window.electron.ipcRenderer.on('version', cb);
    await new Promise((r) => setTimeout(r, 0));

    expect(cb).toHaveBeenCalledWith(expect.anything(), '1.7.1');
  });

  it('url-params 与 show-disclaimer 同样补发', async () => {
    enableTauri({ urlParams: 'moekoe://share?hash=abc', showDisclaimer: true });
    await new Promise((r) => setTimeout(r, 0));

    const params = vi.fn();
    const disclaimer = vi.fn();
    window.electron.ipcRenderer.on('url-params', params);
    window.electron.ipcRenderer.on('show-disclaimer', disclaimer);
    await new Promise((r) => setTimeout(r, 0));

    expect(params).toHaveBeenCalledWith(expect.anything(), 'moekoe://share?hash=abc');
    expect(disclaimer).toHaveBeenCalled();
  });

  it('boot 信息缺失时不产生任何事件', async () => {
    enableTauri({});
    await new Promise((r) => setTimeout(r, 0));
    const cb = vi.fn();
    window.electron.ipcRenderer.on('version', cb);
    await new Promise((r) => setTimeout(r, 0));
    expect(cb).not.toHaveBeenCalled();
  });

  it('app_boot_info 失败时不抛错', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    window.__TAURI_INTERNALS__ = {};
    invoke.mockImplementation((cmd) =>
      cmd === 'app_boot_info' ? Promise.reject(new Error('no acl')) : Promise.resolve()
    );
    expect(() => installTauriShim()).not.toThrow();
    await new Promise((r) => setTimeout(r, 0));
    expect(errSpy).toHaveBeenCalled();
    errSpy.mockRestore();
  });
});

describe('ipcRenderer.invoke 请求/响应', () => {
  it('映射到 ipc_invoke 命令', () => {
    enableTauri();
    window.electron.ipcRenderer.invoke('lyrics-window-pointer-state', 1);
    expect(invoke).toHaveBeenCalledWith('ipc_invoke', {
      channel: 'lyrics-window-pointer-state',
      args: [1],
    });
  });
});

describe('electronAPI 命令映射', () => {
  it('窗口与日志类命令一一对应', () => {
    enableTauri();
    window.electronAPI.startUpdateDownload();
    window.electronAPI.openMvWindow('https://v.qq.com/x');
    window.electronAPI.openLogPath();
    window.electronAPI.exportLog();
    const cmds = invoke.mock.calls.map((c) => c[0]);
    expect(cmds).toEqual([
      'app_boot_info',
      'start_update_download',
      'open_mv_window',
      'open_log_path',
      'export_log',
    ]);
    expect(invoke).toHaveBeenCalledWith('open_mv_window', { url: 'https://v.qq.com/x' });
  });

  it('扩展体系在 Tauri 下安全降级为空实现', async () => {
    enableTauri();
    await expect(window.electronAPI.getExtensions()).resolves.toEqual([]);
    await expect(window.electronAPI.getExtensionsDetailed()).resolves.toEqual([]);
    await expect(window.electronAPI.validateExtension('x')).resolves.toMatchObject({ valid: false });
    await expect(window.electronAPI.installPluginFromZip('p')).resolves.toMatchObject({ success: false });
    await expect(window.electronAPI.getExtensionsDirectory()).resolves.toBe('');
  });

  it('原生对话框返回取消态，避免调用方 await 挂死', async () => {
    enableTauri();
    await expect(window.electronAPI.showOpenDialog()).resolves.toEqual({
      canceled: true,
      filePaths: [],
    });
  });
});

describe('WebView 原生行为拦截', () => {
  it('阻止默认右键菜单（不阻断 Vue 自绘菜单所需的事件传播）', () => {
    enableTauri();
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });

  it('阻止资源拖拽', () => {
    enableTauri();
    const event = new Event('dragstart', { bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });
});

void listen;
