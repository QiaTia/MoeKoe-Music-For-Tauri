// tests/unit/invokeBridge.test.js
// Tauri invoke 路由判定与错误形状：这是前端 ↔ Rust 的接缝，错误形状必须与 axios 对齐，
// 否则 request.js 里所有 `error.response?.data` 分支会静默失效。
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { isTauriEnv, shouldUseInvoke, invokeApi } from '@/utils/invokeBridge';

function setTauriEnv(on) {
  if (on) window.__TAURI_INTERNALS__ = { invoke: vi.fn() };
  else delete window.__TAURI_INTERNALS__;
}

describe('isTauriEnv', () => {
  afterEach(() => setTauriEnv(false));

  it('无 __TAURI_INTERNALS__ 时为 false（纯 Web / 测试环境）', () => {
    setTauriEnv(false);
    expect(isTauriEnv()).toBe(false);
  });

  it('注入 __TAURI_INTERNALS__ 后为 true', () => {
    setTauriEnv(true);
    expect(isTauriEnv()).toBe(true);
  });
});

describe('shouldUseInvoke', () => {
  afterEach(() => setTauriEnv(false));

  it('非 Tauri 环境一律走 HTTP', () => {
    setTauriEnv(false);
    expect(shouldUseInvoke()).toBe(false);
  });

  it('Tauri 环境且未配置自定义地址时走 invoke', () => {
    setTauriEnv(true);
    expect(shouldUseInvoke()).toBe(true);
  });

  it('用户配置了自定义远程地址时尊重用户，走 HTTP', () => {
    setTauriEnv(true);
    localStorage.setItem('settings', JSON.stringify({ apiBaseUrl: 'https://remote.example' }));
    expect(shouldUseInvoke()).toBe(false);
  });

  it('自定义地址为空串或纯空白时仍走 invoke', () => {
    setTauriEnv(true);
    localStorage.setItem('settings', JSON.stringify({ apiBaseUrl: '   ' }));
    expect(shouldUseInvoke()).toBe(true);
  });

  it('settings JSON 损坏时不阻断 invoke', () => {
    setTauriEnv(true);
    localStorage.setItem('settings', '{not json');
    expect(shouldUseInvoke()).toBe(true);
  });
});

describe('invokeApi', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('2xx 返回响应 body', async () => {
    invoke.mockResolvedValue({ status: 200, body: { status: 1, data: [1, 2] } });
    await expect(invokeApi('GET', '/search', { params: { keywords: 'a' } })).resolves.toEqual({
      status: 1,
      data: [1, 2],
    });
    expect(invoke).toHaveBeenCalledWith('api_request', {
      method: 'GET',
      path: '/search',
      params: { keywords: 'a' },
      body: null,
      auth: null,
    });
  });

  it('把 auth 透传为字符串，空值归一为 null', async () => {
    invoke.mockResolvedValue({ status: 200, body: {} });
    await invokeApi('GET', '/x', { auth: 'token=1' });
    expect(invoke.mock.calls[0][1].auth).toBe('token=1');

    await invokeApi('GET', '/x', { auth: '' });
    expect(invoke.mock.calls[1][1].auth).toBeNull();
  });

  it('非 2xx 抛出带 response 的 Error（对齐 axios 形状）', async () => {
    invoke.mockResolvedValue({ status: 403, body: { error_code: 20028 } });
    await expect(invokeApi('POST', '/like', { data: { id: 1 } })).rejects.toMatchObject({
      message: 'Request failed with status code 403',
      response: { status: 403, data: { error_code: 20028 } },
    });
  });

  it('响应缺少 status 时按 500 处理', async () => {
    invoke.mockResolvedValue({ body: { ok: true } });
    await expect(invokeApi('GET', '/x')).rejects.toMatchObject({
      response: { status: 500 },
    });
  });

  it('invoke 自身失败时抛带 request 的 Error', async () => {
    invoke.mockRejectedValue('command not found');
    await expect(invokeApi('GET', '/x')).rejects.toMatchObject({
      message: 'command not found',
      request: { method: 'GET', url: '/x' },
    });
  });

  it('invoke 抛非字符串错误时使用兜底文案', async () => {
    invoke.mockRejectedValue({ code: 1 });
    await expect(invokeApi('GET', '/x')).rejects.toMatchObject({ message: 'invoke failed' });
  });
});
