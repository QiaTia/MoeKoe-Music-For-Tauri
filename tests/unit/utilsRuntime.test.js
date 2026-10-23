// tests/unit/utilsRuntime.test.js
// utils.js 中有副作用的部分：DOM 主题/字体注入、麦克风权限、设备枚举、分享与窗口打开。
// 这些函数直接操作 document / navigator / window，也是「换机器就出问题」的高发区。
vi.mock('@/utils/i18n', () => ({ default: { global: { t: (k) => k } } }));

import { setActivePinia, createPinia } from 'pinia';
import {
  applyCustomFont,
  applyColorTheme,
  getProfileBgColor,
  formatTimestampToAgo,
  formatMilliseconds,
  requestMicrophonePermission,
  getAudioOutputDeviceSignature,
  setTheme,
  openRegisterUrl,
  openMvPlayer,
  share,
} from '@/utils/utils';
import { MoeAuthStore } from '@/stores/store';

const originalMatchMedia = window.matchMedia;
let imageMode = 'load';
let originalImage;

/** 让 Image 的加载结果可控（load / error） */
function installFakeImage(mode = 'load') {
  imageMode = mode;
  originalImage = globalThis.Image;
  class FakeImage {
    constructor() {
      this.naturalWidth = 100;
      this.naturalHeight = 100;
    }
    set src(value) {
      this._src = value;
      queueMicrotask(() => (imageMode === 'load' ? this.onload?.() : this.onerror?.()));
    }
    get src() {
      return this._src;
    }
  }
  globalThis.Image = FakeImage;
}

/** 用假 canvas 替换 createElement('canvas')，避免依赖 node-canvas */
function mockCanvas(pixels) {
  const ctx = {
    drawImage: vi.fn(),
    getImageData: () => ({ data: new Uint8ClampedArray(pixels) }),
  };
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  const realCreate = document.createElement.bind(document);
  vi.spyOn(document, 'createElement').mockImplementation((tag) =>
    tag === 'canvas' ? canvas : realCreate(tag)
  );
  return { canvas, ctx };
}

afterEach(() => {
  vi.restoreAllMocks();
  if (originalImage) globalThis.Image = originalImage;
  originalImage = null;
  window.matchMedia = originalMatchMedia;
  delete navigator.mediaDevices;
  delete navigator.permissions;
  document.getElementById('moekoe-custom-font')?.remove();
});

describe('applyCustomFont', () => {
  it('为空时不注入任何样式', () => {
    applyCustomFont('');
    expect(document.getElementById('moekoe-custom-font')).toBeNull();
  });

  it('注入自定义字体并附带默认回退链', () => {
    applyCustomFont('My Font');
    const style = document.getElementById('moekoe-custom-font');
    expect(style).not.toBeNull();
    expect(style.textContent).toContain('font-family: "My Font"');
    expect(style.textContent).toContain('BlinkMacSystemFont');
  });

  it('字体名中的引号与反斜杠被转义，避免 CSS 注入', () => {
    applyCustomFont('Evil";color:red');
    const css = document.getElementById('moekoe-custom-font').textContent;
    expect(css).toContain('\\"');
    expect(css).not.toContain('font-family: "Evil";');
  });

  it('重复设置时替换而非叠加', () => {
    applyCustomFont('A');
    applyCustomFont('B');
    expect(document.querySelectorAll('#moekoe-custom-font')).toHaveLength(1);
    expect(document.getElementById('moekoe-custom-font').textContent).toContain('"B"');
  });

  it('传入空值会移除已有样式', () => {
    applyCustomFont('A');
    applyCustomFont(null);
    expect(document.getElementById('moekoe-custom-font')).toBeNull();
  });
});

describe('applyColorTheme', () => {
  const primary = () => document.documentElement.style.getPropertyValue('--primary-color');

  it('blue 主题', () => {
    applyColorTheme('blue');
    expect(primary()).toBe('#4A90E2');
  });

  it('green 主题', () => {
    applyColorTheme('green');
    expect(primary()).toBe('#34C759');
  });

  it('orange 主题', () => {
    applyColorTheme('orange');
    expect(primary()).toBe('#ff6b6b');
  });

  it('未知/缺省主题回退到粉色', () => {
    applyColorTheme('unknown');
    expect(primary()).toBe('#FF69B4');
    applyColorTheme(undefined);
    expect(primary()).toBe('#FF69B4');
  });

  it('同时写入配套的次级颜色变量', () => {
    applyColorTheme('blue');
    const style = document.documentElement.style;
    expect(style.getPropertyValue('--primary-color-rgb')).toBe('74, 144, 226');
    expect(style.getPropertyValue('--background-color')).toBe('#E8F4FA');
  });
});

describe('getProfileBgColor', () => {
  it('canvas 上下文不可用时 reject', async () => {
    installFakeImage('load');
    await expect(getProfileBgColor('https://x/a.png')).rejects.toThrow(
      'Canvas context unavailable'
    );
  });

  it('按亮度加权求平均并按 tone 压暗', async () => {
    installFakeImage('load');
    mockCanvas([
      200, 100, 50, 255,
      200, 100, 50, 255,
    ]);
    await expect(getProfileBgColor('https://x/a.png', 0.52)).resolves.toBe('rgb(104, 52, 26)');
  });

  it('全透明图片 reject', async () => {
    installFakeImage('load');
    mockCanvas([0, 0, 0, 0, 0, 0, 0, 0]);
    await expect(getProfileBgColor('https://x/a.png')).rejects.toThrow('No visible pixels');
  });

  it('图片加载失败 reject', async () => {
    installFakeImage('error');
    mockCanvas([0, 0, 0, 255]);
    await expect(getProfileBgColor('https://x/a.png')).rejects.toThrow('Image load failed');
  });
});

describe('formatTimestampToAgo 跨度补全', () => {
  const now = Math.floor(Date.now() / 1000);
  it('周 / 月 / 年', () => {
    expect(formatTimestampToAgo(now - 86400 * 10)).toBe('1周前');
    expect(formatTimestampToAgo(now - 86400 * 40)).toBe('1个月前');
    expect(formatTimestampToAgo(now - 86400 * 400)).toBe('1年前');
  });

  it('未来时间戳返回刚刚', () => {
    expect(formatTimestampToAgo(now + 100)).toBe('刚刚');
  });

  it('毫秒级时间戳会被当作秒解析（与调用方约定一致）', () => {
    expect(formatTimestampToAgo(0)).toBe('');
    expect(formatTimestampToAgo(null)).toBe('');
    expect(formatTimestampToAgo(undefined)).toBe('');
  });
});

describe('formatMilliseconds 边界', () => {
  it('整分钟与零秒', () => {
    expect(formatMilliseconds(120000)).toBe('2分0秒');
  });

  it('超过一小时仍按毫秒解读', () => {
    expect(formatMilliseconds(3661000)).toBe('61分1秒');
  });
});

describe('requestMicrophonePermission', () => {
  it('无 mediaDevices 时直接返回 false', async () => {
    await expect(requestMicrophonePermission()).resolves.toBe(false);
  });

  it('Tauri 环境跳过权限申请（WebView 不支持弹窗）', async () => {
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia: vi.fn() },
    });
    window.__TAURI_INTERNALS__ = {};
    await expect(requestMicrophonePermission()).resolves.toBe(true);
    expect(navigator.mediaDevices.getUserMedia).not.toHaveBeenCalled();
    delete window.__TAURI_INTERNALS__;
  });

  it('权限已授予时申请流并立即释放轨道', async () => {
    const stop = vi.fn();
    const getUserMedia = vi.fn().mockResolvedValue({ getTracks: () => [{ stop }] });
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia },
    });
    Object.defineProperty(navigator, 'permissions', {
      configurable: true,
      value: { query: vi.fn().mockResolvedValue({ state: 'granted' }) },
    });

    await expect(requestMicrophonePermission()).resolves.toBe(true);
    expect(getUserMedia).toHaveBeenCalledWith({ audio: true });
    expect(stop).toHaveBeenCalled();
  });

  it('权限被拒绝时返回 false 且不申请流', async () => {
    const getUserMedia = vi.fn();
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia },
    });
    Object.defineProperty(navigator, 'permissions', {
      configurable: true,
      value: { query: vi.fn().mockResolvedValue({ state: 'denied' }) },
    });

    await expect(requestMicrophonePermission()).resolves.toBe(false);
    expect(getUserMedia).not.toHaveBeenCalled();
  });

  it('permissions API 抛错时回退到 getUserMedia', async () => {
    const getUserMedia = vi.fn().mockResolvedValue({ getTracks: () => [] });
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia },
    });
    Object.defineProperty(navigator, 'permissions', {
      configurable: true,
      value: { query: vi.fn().mockRejectedValue(new Error('unsupported')) },
    });

    await expect(requestMicrophonePermission()).resolves.toBe(true);
  });

  it('用户拒绝授权时返回 false', async () => {
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia: vi.fn().mockRejectedValue(new Error('NotAllowed')) },
    });

    await expect(requestMicrophonePermission()).resolves.toBe(false);
  });
});

describe('getAudioOutputDeviceSignature', () => {
  it('无 enumerateDevices 时返回 null', async () => {
    await expect(getAudioOutputDeviceSignature()).resolves.toBeNull();
  });

  it('仅统计音频输出设备，签名排序稳定', async () => {
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: {
        enumerateDevices: vi.fn().mockResolvedValue([
          { kind: 'audiooutput', deviceId: 'b', groupId: 'g2' },
          { kind: 'audioinput', deviceId: 'mic', groupId: 'g1' },
          { kind: 'audiooutput', deviceId: 'a', groupId: 'g1' },
        ]),
      },
    });

    await expect(getAudioOutputDeviceSignature()).resolves.toBe('a:g1|b:g2');
  });

  it('没有输出设备时返回空串（可稳定比较）', async () => {
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { enumerateDevices: vi.fn().mockResolvedValue([]) },
    });
    await expect(getAudioOutputDeviceSignature()).resolves.toBe('');
  });
});

describe('setTheme', () => {
  function stubMatchMedia(matches) {
    const listeners = new Set();
    const mql = {
      matches,
      addEventListener: vi.fn((_, fn) => listeners.add(fn)),
      removeEventListener: vi.fn((_, fn) => listeners.delete(fn)),
      emit(next) {
        listeners.forEach((fn) => fn({ matches: next }));
      },
      listenerCount: () => listeners.size,
    };
    window.matchMedia = vi.fn(() => mql);
    return mql;
  }

  it('dark 直接挂载 dark 类并持久化', () => {
    stubMatchMedia(false);
    setTheme('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('theme')).toBe('dark');
  });

  it('light 移除 dark 类', () => {
    document.documentElement.classList.add('dark');
    stubMatchMedia(false);
    setTheme('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(localStorage.getItem('theme')).toBe('light');
  });

  it('auto 跟随系统并响应后续变化', () => {
    const mql = stubMatchMedia(true);
    setTheme('auto');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('theme')).toBe('auto');

    mql.emit(false);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('切换主题时注销旧的 auto 监听，避免监听器泄漏', () => {
    const mql = stubMatchMedia(true);
    setTheme('auto');
    expect(mql.listenerCount()).toBe(1);

    setTheme('light');
    expect(mql.removeEventListener).toHaveBeenCalled();
    expect(mql.listenerCount()).toBe(0);
  });

  it('未知主题不改变当前状态', () => {
    stubMatchMedia(false);
    setTheme('neon');
    expect(localStorage.getItem('theme')).toBeNull();
  });
});

describe('openRegisterUrl', () => {
  it('桌面端走 IPC', () => {
    window.electron = { ipcRenderer: { send: vi.fn() } };
    openRegisterUrl('https://a.com/reg');
    expect(window.electron.ipcRenderer.send).toHaveBeenCalledWith('open-url', 'https://a.com/reg');
    delete window.electron;
  });

  it('Web 端走新窗口', () => {
    const open = vi.spyOn(window, 'open').mockImplementation(() => null);
    openRegisterUrl('https://a.com/reg');
    expect(open).toHaveBeenCalledWith('https://a.com/reg', '_blank');
  });
});

describe('openMvPlayer', () => {
  const router = () => ({
    resolve: vi.fn(() => ({ href: '/video?hash=h1' })),
    push: vi.fn(),
  });

  it('桌面端交给独立 MV 窗口', async () => {
    const openMvWindow = vi.fn().mockResolvedValue(undefined);
    window.electronAPI = { openMvWindow };
    const r = router();

    await openMvPlayer(r, 'h1', '标题');
    expect(openMvWindow).toHaveBeenCalledWith(expect.stringContaining('/video?hash=h1'));
    expect(r.push).not.toHaveBeenCalled();
    delete window.electronAPI;
  });

  it('Web 端优先 popup，并聚焦', async () => {
    const focus = vi.fn();
    const open = vi.spyOn(window, 'open').mockImplementation(() => ({ focus }));
    const r = router();

    await openMvPlayer(r, 'h1');
    expect(open).toHaveBeenCalledWith(
      expect.stringContaining('/video?hash=h1'),
      'moekoe-mv',
      expect.stringContaining('width=960')
    );
    expect(focus).toHaveBeenCalled();
    expect(r.push).not.toHaveBeenCalled();
  });

  it('popup 被拦截时回退为路由跳转', async () => {
    vi.spyOn(window, 'open').mockImplementation(() => null);
    const r = router();

    await openMvPlayer(r, 'h1');
    expect(r.push).toHaveBeenCalled();
  });
});

describe('share', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    delete window.electron;
  });

  it('桌面端单曲分享使用分享页地址', () => {
    window.electron = { platform: 'win32' };
    share('晴天', 'HASH1');
    const text = navigator.clipboard.writeText.mock.calls.at(-1)[0];
    expect(text).toContain('https://music.moekoe.cn/share/?hash=HASH1');
    expect(text).toContain('《晴天》');
    expect(window.$message.success).toHaveBeenCalled();
  });

  it('桌面端歌单分享使用深链', () => {
    window.electron = { platform: 'win32' };
    share('我的歌单', 'LIST1', 1, '歌单');
    const text = navigator.clipboard.writeText.mock.calls.at(-1)[0];
    expect(text).toContain('moekoe://share?listid=LIST1');
    expect(text).toContain('歌单');
  });

  it('Web 端使用当前域名拼接', () => {
    share('晴天', 'H2');
    const text = navigator.clipboard.writeText.mock.calls.at(-1)[0];
    expect(text).toContain(`${window.location.host}/#/share/?hash=H2`);
  });

  it('已登录时使用昵称，未登录回退「萌音」', () => {
    const store = MoeAuthStore();
    store.UserInfo = { nickname: '阿珏' };
    share('A', 'H3');
    expect(navigator.clipboard.writeText.mock.calls.at(-1)[0]).toContain('@阿珏');

    store.UserInfo = null;
    share('A', 'H4');
    expect(navigator.clipboard.writeText.mock.calls.at(-1)[0]).toContain('@萌音');
  });
});
