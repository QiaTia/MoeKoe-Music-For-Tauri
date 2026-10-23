// tests/component/Disclaimer.test.js
// 首次启动的用户条款：显示条件、同意后的持久化、桌面端 IPC 回执。
// 该组件决定「用户能否进入应用」，判定错会导致每次都弹或永远不弹。
vi.mock('@/config/disclaimer', () => ({
  DISCLAIMER_BANNER_SRC: '/banner.png',
  USER_AGREEMENT_KEYS: ['clause-1', 'clause-2'],
}));

import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import Disclaimer from '@/components/Disclaimer.vue';

const t = (key) => key;

function mountDisclaimer() {
  return mount(Disclaimer, { global: { mocks: { $t: t } } });
}

/** 桌面端：记录 Rust → 前端的事件回调 */
function installElectron() {
  const listeners = new Map();
  window.electron = {
    platform: 'win32',
    ipcRenderer: {
      on: vi.fn((channel, cb) => listeners.set(channel, cb)),
      send: vi.fn(),
    },
  };
  return {
    send: window.electron.ipcRenderer.send,
    emitShow: () => listeners.get('show-disclaimer')?.(),
  };
}

afterEach(() => {
  delete window.electron;
});

describe('Web 端显示条件', () => {
  it('未同意时显示条款', async () => {
    const w = mountDisclaimer();
    await nextTick();
    expect(w.find('.modal-overlay').exists()).toBe(true);
    expect(w.findAll('.modal-content p')).toHaveLength(2);
  });

  it('已同意时不再显示', async () => {
    localStorage.setItem('disclaimerAccepted', 'true');
    const w = mountDisclaimer();
    await nextTick();
    expect(w.find('.modal-overlay').exists()).toBe(false);
  });

  it('点击同意后关闭并落盘', async () => {
    const w = mountDisclaimer();
    await nextTick();

    await w.findAll('.button-group button')[0].trigger('click');
    expect(w.find('.modal-overlay').exists()).toBe(false);
    expect(localStorage.getItem('disclaimerAccepted')).toBe('true');
  });
});

describe('桌面端 IPC 模式', () => {
  it('仅在收到 show-disclaimer 事件后才显示', async () => {
    const ipc = installElectron();
    const w = mountDisclaimer();
    await nextTick();

    expect(w.find('.modal-overlay').exists()).toBe(false);
    expect(window.electron.ipcRenderer.on).toHaveBeenCalledWith(
      'show-disclaimer',
      expect.any(Function)
    );

    ipc.emitShow();
    await nextTick();
    expect(w.find('.modal-overlay').exists()).toBe(true);
  });

  it('同意时回执给主进程，且不写 localStorage', async () => {
    const ipc = installElectron();
    const w = mountDisclaimer();
    await nextTick();
    ipc.emitShow();
    await nextTick();

    await w.findAll('.button-group button')[0].trigger('click');
    expect(ipc.send).toHaveBeenCalledWith('disclaimer-response', true);
    expect(localStorage.getItem('disclaimerAccepted')).toBeNull();
  });

  it('不同意时回执 false 并尝试关闭窗口', async () => {
    const ipc = installElectron();
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => {});
    const w = mountDisclaimer();
    await nextTick();
    ipc.emitShow();
    await nextTick();

    await w.findAll('.button-group button')[1].trigger('click');
    expect(ipc.send).toHaveBeenCalledWith('disclaimer-response', false);
    expect(closeSpy).toHaveBeenCalled();
    closeSpy.mockRestore();
  });
});

describe('文案渲染', () => {
  it('标题与按钮均走 i18n key', async () => {
    const w = mountDisclaimer();
    await nextTick();
    expect(w.find('h2').text()).toBe('yong-hu-tiao-kuan');
    const buttons = w.findAll('.button-group button');
    expect(buttons[0].text()).toBe('tong-yi');
    expect(buttons[1].text()).toBe('bu-tong-yi');
  });
});
