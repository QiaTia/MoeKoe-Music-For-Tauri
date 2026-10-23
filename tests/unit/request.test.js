// tests/unit/request.test.js
// 请求层：认证串构造、invoke/HTTP 双通道分流、响应解包、错误归一。
// 关键点：invoke 通道的错误形状必须与 axios 完全一致，否则上层 `error.response?.data` 全部失效。
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@/utils/invokeBridge', () => ({ shouldUseInvoke: vi.fn() }));
vi.mock('@/utils/riskVerify', () => ({ handleRiskResponse: vi.fn() }));
vi.mock('@/stores/store', () => ({ MoeAuthStore: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { shouldUseInvoke } from '@/utils/invokeBridge';
import { handleRiskResponse } from '@/utils/riskVerify';
import { MoeAuthStore } from '@/stores/store';
import httpClient, { get, post, put, del, patch, uploadImage } from '@/utils/request';

const DEFAULT_AUTH_USER = {
  UserInfo: { token: 'TK', userid: '100', t1: 'T1' },
  Device: {
    dfid: 'DF',
    mid: 'MID',
    guid: 'GUID',
    serverDev: 'SDEV',
    mac: 'MAC',
  },
};

/** 让 axios 实例走假 adapter，从而能真实驱动拦截器 */
function setAdapter(handler) {
  httpClient.defaults.adapter = handler;
}

describe('请求层', () => {
  beforeEach(() => {
    invoke.mockReset();
    shouldUseInvoke.mockReset();
    handleRiskResponse.mockReset().mockResolvedValue({ handled: false });
    MoeAuthStore.mockReset().mockReturnValue(DEFAULT_AUTH_USER);
    // 默认：HTTP 通道，adapter 返回成功
    setAdapter(async (config) => ({
      data: { status: 1, echo: config.url },
      status: 200,
      statusText: 'OK',
      headers: {},
      config,
    }));
  });

  describe('HTTP 通道', () => {
    it('响应拦截器解包 response.data', async () => {
      shouldUseInvoke.mockReturnValue(false);
      await expect(get('/search', { keywords: 'a' })).resolves.toEqual({
        status: 1,
        echo: '/search',
      });
    });

    it('请求拦截器写入 Authorization，字段顺序与格式固定', async () => {
      shouldUseInvoke.mockReturnValue(false);
      let seenAuth;
      setAdapter(async (config) => {
        seenAuth = config.headers.Authorization;
        return { data: {}, status: 200, statusText: 'OK', headers: {}, config };
      });

      await get('/x');
      // 顺序：token → userid → dfid → t1 → KUGOU_API_MID → GUID → DEV → MAC
      expect(seenAuth).toBe(
        'token=TK;userid=100;dfid=DF;t1=T1;KUGOU_API_MID=MID;KUGOU_API_GUID=GUID;KUGOU_API_DEV=SDEV;KUGOU_API_MAC=MAC'
      );
    });

    it('缺失的认证字段被省略；全空时不设置 Authorization', async () => {
      shouldUseInvoke.mockReturnValue(false);
      MoeAuthStore.mockReturnValue({ UserInfo: null, Device: null });
      let hasAuth = 'unset';
      setAdapter(async (config) => {
        hasAuth = config.headers.Authorization;
        return { data: {}, status: 200, statusText: 'OK', headers: {}, config };
      });

      await get('/x');
      expect(hasAuth).toBeUndefined();
    });

    it('__rawResponse 时不走风控、返回完整响应', async () => {
      shouldUseInvoke.mockReturnValue(false);
      const res = await httpClient.get('/x', { __rawResponse: true });
      expect(res.status).toBe(200);
      expect(handleRiskResponse).not.toHaveBeenCalled();
    });

    it('命中风控且已处理时返回风险处理结果', async () => {
      shouldUseInvoke.mockReturnValue(false);
      handleRiskResponse.mockResolvedValue({ handled: true, data: { recovered: true } });
      await expect(get('/x')).resolves.toEqual({ recovered: true });
    });

    it('4xx 进入错误分支并向上抛', async () => {
      const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      shouldUseInvoke.mockReturnValue(false);
      setAdapter(async (config) => {
        const err = new Error('Request failed with status code 403');
        err.config = config;
        err.response = { status: 403, data: { error_code: 20028 }, headers: {}, config };
        throw err;
      });

      await expect(get('/x')).rejects.toMatchObject({
        response: { status: 403, data: { error_code: 20028 } },
      });
      errSpy.mockRestore();
    });

    it('无响应（网络中断）时提示服务器未响应', async () => {
      const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      shouldUseInvoke.mockReturnValue(false);
      setAdapter(async (config) => {
        const err = new Error('Network Error');
        err.config = config;
        err.request = {};
        throw err;
      });

      await expect(get('/x')).rejects.toThrow('Network Error');
      expect(window.$message.error).toHaveBeenCalledWith('服务器未响应,请稍后再试!');
      errSpy.mockRestore();
    });

    it('请求构建阶段异常时提示请求错误', async () => {
      const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      shouldUseInvoke.mockReturnValue(false);
      setAdapter(async (config) => {
        const err = new Error('bad config');
        err.config = config;
        throw err;
      });

      await expect(get('/x')).rejects.toThrow('bad config');
      expect(window.$message.error).toHaveBeenCalledWith('请求错误,请稍后再试!');
      errSpy.mockRestore();
    });
  });

  describe('invoke 通道', () => {
    beforeEach(() => {
      shouldUseInvoke.mockReturnValue(true);
    });

    it('2xx 直接返回 body，并携带认证串', async () => {
      invoke.mockResolvedValue({ status: 200, body: { status: 1, data: [] }, headers: {} });
      await expect(get('/search', { keywords: 'x' })).resolves.toEqual({ status: 1, data: [] });
      expect(invoke).toHaveBeenCalledWith('api_request', {
        method: 'GET',
        path: '/search',
        params: { keywords: 'x' },
        body: null,
        auth: expect.stringContaining('token=TK'),
      });
    });

    it('非 2xx 抛出的错误与 axios 同形（含 response.status/data）', async () => {
      const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      invoke.mockResolvedValue({ status: 403, body: { error_code: 1 }, headers: {} });
      await expect(get('/x')).rejects.toMatchObject({
        response: { status: 403, data: { error_code: 1 } },
      });
      errSpy.mockRestore();
    });

    it('POST 把 data 作为 body 传入', async () => {
      invoke.mockResolvedValue({ status: 200, body: {}, headers: {} });
      await post('/like', { id: 7 });
      expect(invoke).toHaveBeenCalledWith(
        'api_request',
        expect.objectContaining({ method: 'POST', path: '/like', body: { id: 7 } })
      );
    });

    it('PUT / DELETE / PATCH 分别映射方法名', async () => {
      invoke.mockResolvedValue({ status: 200, body: {}, headers: {} });
      await put('/a', { v: 1 });
      await del('/b');
      await patch('/c', { v: 2 });
      const methods = invoke.mock.calls.map((c) => c[1].method);
      expect(methods).toEqual(['PUT', 'DELETE', 'PATCH']);
    });

    it('风控已处理时返回处理结果', async () => {
      invoke.mockResolvedValue({ status: 200, body: { status: 1 }, headers: {} });
      handleRiskResponse.mockResolvedValue({ handled: true, data: { verified: true } });
      await expect(get('/x')).resolves.toEqual({ verified: true });
    });
  });

  describe('回调与上传', () => {
    it('onSuccess 收到响应，onError 收到错误', async () => {
      shouldUseInvoke.mockReturnValue(false);
      const ok = vi.fn();
      const bad = vi.fn();
      await get('/x', {}, {}, ok);
      expect(ok).toHaveBeenCalledWith({ status: 1, echo: '/x' });

      setAdapter(async (config) => {
        const err = new Error('boom');
        err.config = config;
        err.response = { status: 500, data: {} };
        throw err;
      });
      await expect(get('/y', {}, {}, null, bad)).rejects.toThrow('boom');
      expect(bad).toHaveBeenCalled();
    });

    it('uploadImage 以 multipart 发送并附带额外字段', async () => {
      let captured;
      setAdapter(async (config) => {
        captured = config;
        return { data: { ok: 1 }, status: 200, statusText: 'OK', headers: {}, config };
      });

      const file = new File(['x'], 'a.png', { type: 'image/png' });
      await uploadImage('/upload', file, { songId: '9' });
      expect(captured.headers['Content-Type']).toBe('multipart/form-data');
      expect(captured.data instanceof FormData).toBe(true);
    });
  });
});
