// tests/unit/listenTime.test.js
// 听歌时长 store：CSCC 事件载体构造 + 上报响应分级处理。
// 该 store 是 v1.7.2 新增的等级体系入口，载体字段多删少删都会导致酷狗侧拒绝，
// 因此对 generateCsccJson 的输出形状做逐字段断言（该函数未导出，经 reportCsccEvent 观测）。
vi.mock('@/utils/request', () => ({
  get: vi.fn(),
  post: vi.fn(),
}));

import { setActivePinia, createPinia } from 'pinia';
import { get, post } from '@/utils/request';
import { useListenTimeStore } from '@/stores/listenTime';

/** 屏蔽被测代码里预期内的 console 噪声，返回 spy 便于断言 */
const silenceConsole = () => ({
  log: vi.spyOn(console, 'log').mockImplementation(() => {}),
  warn: vi.spyOn(console, 'warn').mockImplementation(() => {}),
  error: vi.spyOn(console, 'error').mockImplementation(() => {}),
});

describe('useListenTimeStore', () => {
  let spy;

  beforeEach(() => {
    setActivePinia(createPinia());
    get.mockReset();
    post.mockReset();
    spy = silenceConsole();
  });

  afterEach(() => {
    spy.log.mockRestore();
    spy.warn.mockRestore();
    spy.error.mockRestore();
  });

  it('初始时长为 0', () => {
    expect(useListenTimeStore().listenTime).toBe(0);
  });

  describe('refreshListenTime', () => {
    it('取 d_sec 写入 store，并带上时间戳防缓存', async () => {
      get.mockResolvedValue({ status: 1, data: { d_sec: 3600 } });
      const store = useListenTimeStore();

      await expect(store.refreshListenTime()).resolves.toEqual({
        status: 1,
        data: { d_sec: 3600 },
      });
      expect(store.listenTime).toBe(3600);
      expect(get.mock.calls[0][0]).toMatch(/^\/user\/grade\/info\?t=\d+$/);
    });

    it('响应缺 d_sec 时回落为 0（而非 undefined）', async () => {
      get.mockResolvedValue({ status: 1, data: {} });
      const store = useListenTimeStore();

      await store.refreshListenTime();
      expect(store.listenTime).toBe(0);
    });

    it('请求失败返回 null 且不污染已有时长', async () => {
      const store = useListenTimeStore();
      get.mockResolvedValueOnce({ status: 1, data: { d_sec: 120 } });
      await store.refreshListenTime();

      get.mockRejectedValueOnce(new Error('network'));
      await expect(store.refreshListenTime()).resolves.toBeNull();
      expect(store.listenTime).toBe(120);
      expect(spy.error).toHaveBeenCalled();
    });
  });

  describe('reportCsccEvent — 事件载体形状', () => {
    it('start 只保留 event 与 mixsongid，其余字段全部剔除', async () => {
      post.mockResolvedValue({ status: 1 });
      const store = useListenTimeStore();

      await store.reportCsccEvent('start', {
        mixsongid: 77,
        duration: 215000,
        d_sec: 999,
        diff_sec: 120,
      });

      const [url, payload] = post.mock.calls[0];
      expect(url).toMatch(/^\/user\/listen\/report\?timestamp=\d+$/);
      expect(payload).toEqual({ event: 'start', mixsongid: 77 });
      // start 不需要先刷基线
      expect(get).not.toHaveBeenCalled();
      expect(store.listenTime).toBe(0);
    });

    it('end 标记「完整播放」，switch 标记「中断播放」', async () => {
      post.mockResolvedValue({ status: 1 });
      const store = useListenTimeStore();
      const data = { mixsongid: 5, duration: 200000, d_sec: 60, diff_sec: 200 };

      await store.reportCsccEvent('end', data);
      expect(post.mock.calls[0][1]).toEqual({
        event: 'end',
        mixsongid: 5,
        duration: 200000,
        state: '完整播放',
        d_sec: 60,
        diff_sec: 200,
      });

      await store.reportCsccEvent('switch', data);
      expect(post.mock.calls[1][1]).toEqual({
        event: 'end',
        mixsongid: 5,
        duration: 200000,
        state: '中断播放',
        d_sec: 60,
        diff_sec: 200,
      });
    });

    it('d_sec 未传时回落到 store 当前值', async () => {
      get.mockResolvedValue({ status: 1, data: { d_sec: 42 } });
      post.mockResolvedValue({ status: 1 });
      const store = useListenTimeStore();

      // 未显式传 d_sec → 先刷新基线拿到 42
      await store.reportCsccEvent('end', { mixsongid: 1, duration: 1000, diff_sec: 10 });
      expect(get).toHaveBeenCalledTimes(1);

      // 第二次显式不传 d_sec，此时 store 已是 42，但仍会因 !data.d_sec 触发刷新
      await store.reportCsccEvent('end', { mixsongid: 1, duration: 1000, diff_sec: 10 });
      expect(post.mock.calls[1][1].d_sec).toBe(42);
    });

    // 刻画既有行为（与上游 1:1 一致）：`!data.d_sec` 对 0 成立 → 会多刷一次基线，
    // 但载体里 `data.d_sec ?? listenTime.value` 中 0 不算 nullish → 最终仍上报 0。
    // 即「显式 0 优先于刚刷到的基线」，副作用只是一次多余的 /user/grade/info 请求。
    it('d_sec 传 0 时仍刷新基线，但载体保留显式 0（nullish 语义）', async () => {
      get.mockResolvedValue({ status: 1, data: { d_sec: 8 } });
      post.mockResolvedValue({ status: 1 });
      const store = useListenTimeStore();

      await store.reportCsccEvent('end', {
        mixsongid: 1, duration: 1000, diff_sec: 10, d_sec: 0,
      });

      expect(get).toHaveBeenCalledTimes(1);
      expect(post.mock.calls[0][1].d_sec).toBe(0);
      // 基线刷新确实改写了 store，只是没被本次载体采用
      expect(store.listenTime).toBe(8);
    });
  });

  describe('reportCsccEvent — 响应分级处理', () => {
    it('end/switch 同步成功时更新 store 时长', async () => {
      post.mockResolvedValue({
        status: 1,
        data: {
          playback_accepted: true,
          grade_synced: true,
          grade: { data: { d_sec: 7200 } },
        },
      });
      const store = useListenTimeStore();

      const result = await store.reportCsccEvent('end', {
        mixsongid: 1, duration: 1000, diff_sec: 10, d_sec: 60,
      });

      expect(result.status).toBe(1);
      expect(store.listenTime).toBe(7200);
      expect(spy.log).toHaveBeenCalled();
      expect(spy.warn).not.toHaveBeenCalled();
    });

    it('playback_accepted 为假时告警，但仍不更新时长', async () => {
      post.mockResolvedValue({
        status: 1,
        data: { playback_accepted: false, report: 'rejected', grade_synced: false },
      });
      const store = useListenTimeStore();

      await store.reportCsccEvent('end', {
        mixsongid: 1, duration: 1000, diff_sec: 10, d_sec: 60,
      });

      expect(spy.warn).toHaveBeenCalled();
      expect(store.listenTime).toBe(0);
    });

    it('grade_synced 为假时保留原时长并告警', async () => {
      post.mockResolvedValue({
        status: 1,
        data: { playback_accepted: true, grade_synced: false, grade: { err: 1 } },
      });
      const store = useListenTimeStore();

      await store.reportCsccEvent('switch', {
        mixsongid: 1, duration: 1000, diff_sec: 10, d_sec: 30,
      });

      expect(store.listenTime).toBe(0);
      expect(spy.warn).toHaveBeenCalled();
    });

    it.each([
      ['非数字字符串', 'abc'],
      ['非整数', 1.5],
      ['负数', -1],
    ])('服务端 d_sec 为异常值（%s）时不写入 store', async (_label, badSec) => {
      post.mockResolvedValue({
        status: 1,
        data: {
          playback_accepted: true,
          grade_synced: true,
          grade: { data: { d_sec: badSec } },
        },
      });
      const store = useListenTimeStore();

      await store.reportCsccEvent('end', {
        mixsongid: 1, duration: 1000, diff_sec: 10, d_sec: 60,
      });

      expect(store.listenTime).toBe(0);
    });

    it('start 走原始响应分支：status=1 即视为上报成功', async () => {
      post.mockResolvedValue({ status: 1 });
      const store = useListenTimeStore();

      await store.reportCsccEvent('start', { mixsongid: 1 });
      expect(spy.log).toHaveBeenCalled();
      expect(spy.warn).not.toHaveBeenCalled();
    });

    it('errcode=0 同样视为成功', async () => {
      post.mockResolvedValue({ errcode: 0 });
      const store = useListenTimeStore();

      await store.reportCsccEvent('start', { mixsongid: 1 });
      expect(spy.warn).not.toHaveBeenCalled();
    });

    it('非预期响应（无 success 标记）告警但不抛错', async () => {
      post.mockResolvedValue({ status: 0, errcode: 20028 });
      const store = useListenTimeStore();

      await expect(store.reportCsccEvent('start', { mixsongid: 1 })).resolves.toEqual({
        status: 0,
        errcode: 20028,
      });
      expect(spy.warn).toHaveBeenCalled();
    });

    it('上报抛错时降级为 null，不打断播放链路', async () => {
      post.mockRejectedValue(new Error('boom'));
      const store = useListenTimeStore();

      await expect(
        store.reportCsccEvent('end', { mixsongid: 1, duration: 1000, diff_sec: 10, d_sec: 5 })
      ).resolves.toBeNull();
      expect(spy.error).toHaveBeenCalled();
    });
  });
});
