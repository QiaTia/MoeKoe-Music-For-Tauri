// tests/unit/riskVerify.test.js
// 酷狗风控（20028 / ssa-code）：事件 ID 提取、重试上限、验证取消与二次风控。
// 这些分支一旦错判，表现是「用户点了验证却仍失败」或「无限弹窗」，线上极难复现。
//
// 组件替身说明：openRiskVerifyModal 内部用 createVNode + render 挂载，
// 替身通过 __riskBehavior 在 setup 后的微任务里触发 emit('success'|'cancel')，
// 默认行为是 cancel —— 保证任何遗漏注入的用例都有确定结果，不会把测试挂死。
const behaviorRef = (globalThis.__riskBehavior = null);

vi.mock('@/components/RiskVerifyModal.vue', () => ({
  default: {
    name: 'RiskVerifyModal',
    props: { eventId: String, requestRiskApi: Function },
    setup(props, { emit }) {
      Promise.resolve().then(() => {
        const fn = globalThis.__riskBehavior || ((e) => e('cancel'));
        fn(emit, props);
      });
      return () => null;
    },
  },
}));

import { getRiskEventIdFromResponse, handleRiskResponse } from '@/utils/riskVerify';

describe('getRiskEventIdFromResponse', () => {
  it('从 payload.ssaCode 提取', () => {
    expect(getRiskEventIdFromResponse({ data: { ssaCode: 'E1' } })).toBe('E1');
  });

  it('从 data.ssa_code 提取（下划线命名）', () => {
    expect(getRiskEventIdFromResponse({ data: { data: { ssa_code: 'E2' } } })).toBe('E2');
  });

  it('从 headers.get 提取', () => {
    const headers = { get: (name) => (name === 'ssa-code' ? 'E3' : undefined) };
    expect(getRiskEventIdFromResponse({ data: { data: {} }, headers })).toBe('E3');
  });

  it('从普通 headers 对象按大小写不敏感提取', () => {
    expect(
      getRiskEventIdFromResponse({ data: { data: {} }, headers: { 'SSA-Code': 'E4' } })
    ).toBe('E4');
  });

  it('无任何来源时返回空串', () => {
    expect(getRiskEventIdFromResponse({ data: { data: {} } })).toBe('');
    expect(getRiskEventIdFromResponse({})).toBe('');
    expect(getRiskEventIdFromResponse(null)).toBe('');
  });
});

describe('handleRiskResponse', () => {
  beforeEach(() => {
    globalThis.__riskBehavior = null; // 默认 cancel
  });

  it('__skipRisk 直接跳过（验证接口自身不再触发风控）', async () => {
    await expect(
      handleRiskResponse({ data: { error_code: 20028 }, config: { __skipRisk: true } })
    ).resolves.toEqual({ handled: false });
  });

  it('非风控响应原样放行', async () => {
    await expect(
      handleRiskResponse({ data: { status: 1, error_code: 0 }, config: {} })
    ).resolves.toEqual({ handled: false });
  });

  it('命中 20028 但拿不到 eventId 时不弹窗，交回上层', async () => {
    await expect(
      handleRiskResponse({ data: { error_code: 20028 }, config: {} })
    ).resolves.toEqual({ handled: false });
  });

  it('status=0 且带 ssa-code 头时进入验证流程', async () => {
    const requestAgain = vi.fn();
    const requestRiskApi = vi.fn();
    const r = await handleRiskResponse(
      { data: { status: 0 }, headers: { get: () => 'EVT' }, config: {} },
      requestAgain,
      requestRiskApi
    );
    // 默认替身取消验证 → handled:true 且回传原始 payload
    expect(r).toEqual({ handled: true, data: { status: 0 } });
    expect(requestAgain).not.toHaveBeenCalled();
  });

  it('已重试过一次仍风控时抛错，避免无限重试', async () => {
    const res = {
      data: { error_code: 20028, ssaCode: 'EVT' },
      config: { __riskRetryCount: 1 },
    };
    await expect(handleRiskResponse(res)).rejects.toMatchObject({ message: '安全验证未通过' });
  });

  it('缺少重试/验证请求方法时抛明确错误', async () => {
    const res = { data: { error_code: 20028, ssaCode: 'EVT' }, config: {} };
    await expect(handleRiskResponse(res)).rejects.toThrow('缺少安全验证请求方法');
  });

  it('验证通过后重试原请求，返回新数据', async () => {
    globalThis.__riskBehavior = (emit) => emit('success');

    const requestAgain = vi.fn().mockResolvedValue({ data: { status: 1, list: [1] } });
    const requestRiskApi = vi.fn();
    const res = { data: { error_code: 20028, ssaCode: 'EVT' }, config: {}, headers: {} };

    const r = await handleRiskResponse(res, requestAgain, requestRiskApi);
    expect(r).toEqual({ handled: true, data: { status: 1, list: [1] } });
    // 重试时必须带上递增的重试计数
    expect(requestAgain).toHaveBeenCalledWith(expect.objectContaining({ __riskRetryCount: 1 }));
  });

  it('验证通过但重试后仍风控 → 抛错终止', async () => {
    globalThis.__riskBehavior = (emit) => emit('success');
    const requestAgain = vi.fn().mockResolvedValue({
      data: { error_code: 20028, ssaCode: 'EVT2' },
      headers: {},
    });

    await expect(
      handleRiskResponse(
        { data: { error_code: 20028, ssaCode: 'EVT' }, config: {}, headers: {} },
        requestAgain,
        vi.fn()
      )
    ).rejects.toMatchObject({ message: '安全验证未通过' });
  });

  it('重试响应无 data.data 包装时直接取返回值', async () => {
    globalThis.__riskBehavior = (emit) => emit('success');
    const requestAgain = vi.fn().mockResolvedValue({ status: 1, ok: true });

    const r = await handleRiskResponse(
      { data: { error_code: 20028, ssaCode: 'EVT' }, config: {}, headers: {} },
      requestAgain,
      vi.fn()
    );
    expect(r.data).toEqual({ status: 1, ok: true });
  });
});

void behaviorRef;
