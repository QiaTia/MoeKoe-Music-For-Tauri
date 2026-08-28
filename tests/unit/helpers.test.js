// tests/unit/helpers.test.js
// 通用交互辅助逻辑：节流、音量滚轮、键盘快捷键。不涉及 DOM 重渲染。
// Helpers 间接依赖 utils(i18n) 与 stores/store(pinia)，统一打桩避免加载框架层。
vi.mock('@/utils/i18n', () => ({ default: { global: { t: (k) => k } } }));
vi.mock('../../stores/store', () => ({
  MoeAuthStore: vi.fn(() => ({ isAuthenticated: false })),
}));

import { ref } from 'vue';
import useHelpers from '@/components/player/Helpers';

const identity = (k) => k;

describe('useHelpers', () => {
  it('节流：delay 内只执行一次', () => {
    vi.useFakeTimers();
    const { throttle } = useHelpers(identity);
    const fn = vi.fn();
    const throttled = throttle(fn, 100);
    throttled();
    throttled();
    throttled();
    expect(fn).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(100);
    throttled();
    expect(fn).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });

  it('handleVolumeScroll 滚轮上调音量并夹紧 0-100', () => {
    const { handleVolumeScroll } = useHelpers(identity);
    const volume = ref(50);
    const changeVolume = vi.fn();
    const up = { preventDefault: vi.fn(), deltaY: -100 };
    handleVolumeScroll(up, volume, changeVolume);
    expect(up.preventDefault).toHaveBeenCalled();
    expect(volume.value).toBe(60); // +10
    expect(changeVolume).toHaveBeenCalled();

    const down = { preventDefault: vi.fn(), deltaY: 100 };
    handleVolumeScroll(down, volume, changeVolume);
    expect(volume.value).toBe(50); // -10
  });

  it('handleVolumeScroll 夹紧到边界', () => {
    const { handleVolumeScroll } = useHelpers(identity);
    const volume = ref(98);
    const changeVolume = vi.fn();
    handleVolumeScroll({ preventDefault() {}, deltaY: -100 }, volume, changeVolume);
    expect(volume.value).toBe(100); // 不超过 100
    volume.value = 5;
    handleVolumeScroll({ preventDefault() {}, deltaY: 100 }, volume, changeVolume);
    expect(volume.value).toBe(0); // 不低于 0
  });

  it('handleKeyDown：输入框聚焦时不响应', () => {
    const { handleKeyDown } = useHelpers(identity);
    const handlers = { togglePlayPause: vi.fn(), playSongFromQueue: vi.fn() };
    handleKeyDown({ code: 'Space', preventDefault() {} }, handlers, true);
    expect(handlers.togglePlayPause).not.toHaveBeenCalled();
  });

  it('handleKeyDown：Space 切换播放/暂停', () => {
    const { handleKeyDown } = useHelpers(identity);
    const toggle = vi.fn();
    handleKeyDown(
      { code: 'Space', preventDefault: vi.fn() },
      { togglePlayPause: toggle, playSongFromQueue: vi.fn(), showLyrics: ref(false), toggleLyrics: vi.fn() },
      false,
    );
    expect(toggle).toHaveBeenCalled();
  });

  it('handleKeyDown：左右方向键切歌，Esc 关闭歌词', () => {
    const { handleKeyDown } = useHelpers(identity);
    const prev = vi.fn();
    const next = vi.fn();
    const toggleLyrics = vi.fn();
    const handlers = {
      togglePlayPause: vi.fn(),
      playSongFromQueue: (dir) => (dir === 'previous' ? prev() : next()),
      showLyrics: ref(true),
      toggleLyrics,
    };
    handleKeyDown({ code: 'ArrowLeft', preventDefault: vi.fn() }, handlers, false);
    expect(prev).toHaveBeenCalled();
    handleKeyDown({ code: 'ArrowRight', preventDefault: vi.fn() }, handlers, false);
    expect(next).toHaveBeenCalled();
    handleKeyDown({ code: 'Escape', preventDefault: vi.fn() }, handlers, false);
    expect(toggleLyrics).toHaveBeenCalled();
  });
});
