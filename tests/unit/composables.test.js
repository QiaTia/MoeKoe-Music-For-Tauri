// tests/unit/composables.test.js
// 组合式函数：keep-alive 感知的 watch 生命周期，以及详情页滚动折叠的样式推进。
import { defineComponent, nextTick, ref, h } from 'vue';
import { mount } from '@vue/test-utils';
import { useActivatedWatch } from '@/composables/useActivatedWatch';
import { useStickyDetailHeader } from '@/composables/useStickyDetailHeader';

function setupHost(setupFn) {
  const Comp = defineComponent({ setup: setupFn, render: () => null });
  return mount(Comp);
}

describe('useActivatedWatch', () => {
  it('挂载即开始监听，依赖变化触发回调', async () => {
    const spy = vi.fn();
    let state;
    setupHost(() => {
      state = ref(0);
      useActivatedWatch(state, spy);
      return () => null;
    });

    state.value = 1;
    await nextTick();
    expect(spy).toHaveBeenCalled();
  });

  it('返回的 cleanup 能主动停止监听', async () => {
    const spy = vi.fn();
    let state;
    let stop;
    setupHost(() => {
      state = ref(0);
      stop = useActivatedWatch(state, spy);
      return () => null;
    });

    stop();
    state.value = 1;
    await nextTick();
    expect(spy).not.toHaveBeenCalled();
  });

  it('重复 cleanup 不抛错（幂等）', () => {
    let stop;
    setupHost(() => {
      stop = useActivatedWatch(ref(0), vi.fn());
      return () => null;
    });
    expect(() => { stop(); stop(); }).not.toThrow();
  });

  it('组件卸载后停止监听', async () => {
    const spy = vi.fn();
    let state;
    const wrapper = setupHost(() => {
      state = ref(0);
      useActivatedWatch(state, spy);
      return () => null;
    });

    wrapper.unmount();
    state.value = 1;
    await nextTick();
    expect(spy).not.toHaveBeenCalled();
  });
});

describe('useStickyDetailHeader', () => {
  let scrollEl;

  beforeEach(() => {
    scrollEl = document.createElement('div');
    scrollEl.className = 'app-main-scroll';
    const detail = document.createElement('div');
    detail.className = 'detail-page';
    const listHeader = document.createElement('div');
    listHeader.className = 'track-list-header';
    listHeader.getBoundingClientRect = () => ({ height: 40, top: 0, bottom: 40 });
    detail.appendChild(listHeader);
    document.body.append(scrollEl, detail);
  });

  async function mountHeader() {
    let api;
    const wrapper = setupHost(() => {
      api = useStickyDetailHeader();
      return () => null;
    });
    await nextTick();
    await new Promise((r) => setTimeout(r, 20)); // 等 onMounted 的 rAF
    return { api, wrapper };
  }

  function setScroll(top) {
    Object.defineProperty(scrollEl, 'scrollTop', {
      configurable: true,
      writable: true,
      value: top,
    });
    scrollEl.dispatchEvent(new Event('scroll'));
  }

  it('初始为展开态：封面 200px、详情可见、折叠操作隐藏', async () => {
    const { api } = await mountHeader();
    expect(api.coverStyle.value.width).toBe('200px');
    expect(api.coverStyle.value.height).toBe('200px');
    expect(api.detailsStyle.value.opacity).toBe(1);
    expect(api.detailsStyle.value.pointerEvents).toBe('auto');
    expect(api.collapsedActionsStyle.value.opacity).toBe(0);
    expect(api.collapsedActionsStyle.value.pointerEvents).toBe('none');
  });

  it('滚动到中点时封面与文字尺寸线性收缩', async () => {
    const { api } = await mountHeader();
    setScroll(82); // collapseDistance = 220 - 56 = 164，进度 0.5
    await nextTick();

    expect(api.coverStyle.value.width).toBe('118px');
    expect(api.titleStyle.value.fontSize).toBe('28px');
    expect(api.detailsStyle.value.opacity).toBeCloseTo(0.4, 5);
    expect(api.infoStyle.value.overflow).toBe('hidden');
  });

  it('滚动到底部进入完全折叠态', async () => {
    const { api } = await mountHeader();
    setScroll(200); // 超过 164，被 clamp 到 1
    await nextTick();

    expect(api.coverStyle.value.width).toBe('36px');
    expect(api.titleStyle.value.fontSize).toBe('20px');
    expect(api.detailsStyle.value.opacity).toBe(0);
    expect(api.collapsedActionsStyle.value.opacity).toBe(1);
    expect(api.collapsedActionsStyle.value.pointerEvents).toBe('auto');
    expect(api.spacerStyle.value.height).toBe(`${164 - 20}px`);
  });

  it('折叠操作按钮在阈值附近渐显', async () => {
    const { api } = await mountHeader();
    setScroll(164 * 0.9);
    await nextTick();
    // 0.9 刚过阈值 0.86，透明度很小但不为 0
    expect(api.collapsedActionsStyle.value.opacity).toBeGreaterThan(0);
    expect(api.collapsedActionsStyle.value.opacity).toBeLessThan(0.5);
  });

  it('列表表头与轨道表头位置随折叠高度推进', async () => {
    const { api } = await mountHeader();
    expect(api.listHeaderStyle.value.top).toBe('56px');
    expect(api.trackHeaderStyle.value.top).toBe(`${56 + 40 - 3}px`);
    void h;
  });
});
