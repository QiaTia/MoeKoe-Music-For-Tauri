// tests/component/BackToTop.test.js
// 回到顶部按钮：滚动阈值判定、容器绑定/解绑、点击滚动。
// 该组件按选择器查找滚动容器，选择器或挂载时机变化会让按钮「永远不出现」。
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (k) => k }) }));

import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import BackToTop from '@/components/BackToTop.vue';

let container;

function createContainer(className = 'app-main-scroll') {
  const el = document.createElement('div');
  el.className = className;
  el.scrollTo = vi.fn();
  document.body.appendChild(el);
  return el;
}

function setScroll(el, top) {
  Object.defineProperty(el, 'scrollTop', { configurable: true, writable: true, value: top });
  el.dispatchEvent(new Event('scroll'));
}

async function mountButton(props = {}) {
  const wrapper = mount(BackToTop, { props, attachTo: document.body });
  await nextTick();
  await nextTick();
  return wrapper;
}

beforeEach(() => {
  container = createContainer();
});

afterEach(() => {
  document.body.innerHTML = '';
});

describe('可见性', () => {
  it('初始隐藏（未超过阈值）', async () => {
    const w = await mountButton();
    expect(w.find('.back-to-top').isVisible()).toBe(false);
    w.unmount();
  });

  it('滚动超过阈值后显示', async () => {
    const w = await mountButton({ threshold: 100 });
    setScroll(container, 200);
    await nextTick();
    expect(w.find('.back-to-top').isVisible()).toBe(true);
    w.unmount();
  });

  it('回滚到阈值以下重新隐藏', async () => {
    const w = await mountButton({ threshold: 100 });
    setScroll(container, 200);
    await nextTick();
    setScroll(container, 10);
    await nextTick();
    expect(w.find('.back-to-top').isVisible()).toBe(false);
    w.unmount();
  });

  it('默认阈值为 360', async () => {
    const w = await mountButton();
    setScroll(container, 359);
    await nextTick();
    expect(w.find('.back-to-top').isVisible()).toBe(false);

    setScroll(container, 361);
    await nextTick();
    expect(w.find('.back-to-top').isVisible()).toBe(true);
    w.unmount();
  });
});

describe('点击行为', () => {
  it('平滑滚动回顶部', async () => {
    const w = await mountButton({ threshold: 0 });
    setScroll(container, 500);
    await nextTick();

    await w.find('.back-to-top').trigger('click');
    expect(container.scrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
    w.unmount();
  });
});

describe('容器绑定', () => {
  it('支持自定义 target 选择器', async () => {
    const custom = createContainer('my-scroll');
    const w = await mountButton({ target: '.my-scroll', threshold: 10 });
    setScroll(custom, 50);
    await nextTick();
    expect(w.find('.back-to-top').isVisible()).toBe(true);
    w.unmount();
  });

  it('目标容器不存在时不抛错', async () => {
    document.body.innerHTML = '';
    const w = await mountButton({ target: '.missing' });
    expect(w.find('.back-to-top').isVisible()).toBe(false);
    w.unmount();
  });

  it('卸载后不再响应滚动', async () => {
    const w = await mountButton({ threshold: 100 });
    w.unmount();
    // 容器仍存在，但监听器应已移除
    expect(() => setScroll(container, 500)).not.toThrow();
  });
});

describe('样式覆盖', () => {
  it('bottom / right 透传到内联样式', async () => {
    const w = await mountButton({ threshold: 0, bottom: '20px', right: '10px' });
    setScroll(container, 100);
    await nextTick();
    const style = w.find('.back-to-top').element.getAttribute('style');
    expect(style).toContain('bottom: 20px');
    expect(style).toContain('right: 10px');
    w.unmount();
  });
});
