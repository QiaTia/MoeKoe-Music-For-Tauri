// tests/unit/apiBaseUrl.test.js
// API 基址解析：用户自定义地址的校验/归一化，以及默认值回退。
import {
  DEFAULT_API_BASE_URL,
  normalizeApiBaseUrl,
  validateApiBaseUrl,
  getApiBaseUrl,
  joinApiUrl,
  testApiBaseUrl,
} from '@/utils/apiBaseUrl';

describe('validateApiBaseUrl', () => {
  it('空值视为「未自定义」且合法', () => {
    expect(validateApiBaseUrl('')).toEqual({ ok: true, value: '' });
    expect(validateApiBaseUrl(null)).toEqual({ ok: true, value: '' });
    expect(validateApiBaseUrl('   ')).toEqual({ ok: true, value: '' });
  });

  it('非 URL 结构返回格式错误', () => {
    const r = validateApiBaseUrl('not-a-url');
    expect(r.ok).toBe(false);
    expect(r.error).toBe('请输入完整的 http(s):// 地址');
  });

  it('拒绝非 http(s) 协议', () => {
    const r = validateApiBaseUrl('ftp://example.com');
    expect(r.ok).toBe(false);
    expect(r.error).toBe('仅支持 http:// 或 https://');
  });

  it('去掉尾部斜杠并保留其余原文', () => {
    expect(validateApiBaseUrl('https://a.com/api///')).toEqual({
      ok: true,
      value: 'https://a.com/api',
    });
  });

  it('接受带端口的本地地址', () => {
    expect(validateApiBaseUrl('http://127.0.0.1:6521')).toEqual({
      ok: true,
      value: 'http://127.0.0.1:6521',
    });
  });
});

describe('normalizeApiBaseUrl', () => {
  it('合法返回归一化值，非法返回空串', () => {
    expect(normalizeApiBaseUrl('https://a.com/')).toBe('https://a.com');
    expect(normalizeApiBaseUrl('bad')).toBe('');
    expect(normalizeApiBaseUrl(undefined)).toBe('');
  });
});

describe('getApiBaseUrl', () => {
  it('未配置时用默认地址', () => {
    expect(getApiBaseUrl()).toBe(DEFAULT_API_BASE_URL);
  });

  it('settings 中的自定义地址优先', () => {
    localStorage.setItem('settings', JSON.stringify({ apiBaseUrl: 'https://custom.com//' }));
    expect(getApiBaseUrl()).toBe('https://custom.com');
  });

  it('自定义地址非法时回退默认值', () => {
    localStorage.setItem('settings', JSON.stringify({ apiBaseUrl: 'ftp://x' }));
    expect(getApiBaseUrl()).toBe(DEFAULT_API_BASE_URL);
  });

  it('settings JSON 损坏时回退默认值', () => {
    localStorage.setItem('settings', '{broken');
    expect(getApiBaseUrl()).toBe(DEFAULT_API_BASE_URL);
  });
});

describe('joinApiUrl', () => {
  it('拼接时消解两侧多余斜杠', () => {
    expect(joinApiUrl('https://a.com/', '/search')).toBe('https://a.com/search');
    expect(joinApiUrl('https://a.com///', 'search')).toBe('https://a.com/search');
  });

  it('空 path 返回带尾斜杠的基址', () => {
    expect(joinApiUrl('https://a.com', '')).toBe('https://a.com/');
    expect(joinApiUrl('https://a.com', '/')).toBe('https://a.com/');
  });
});

describe('testApiBaseUrl', () => {
  let originalFetch;
  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('返回 dfid 时判定连通', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: true,
      json: async () => ({ status: 1, data: { dfid: 'abc123' } }),
    }));
    const r = await testApiBaseUrl('https://a.com');
    expect(r).toEqual({ ok: true, data: { status: 1, data: { dfid: 'abc123' } }, dfid: 'abc123' });
    expect(globalThis.fetch).toHaveBeenCalledWith(
      'https://a.com/register/dev',
      expect.objectContaining({ method: 'GET' })
    );
  });

  it('缺少 dfid 视为不可用', async () => {
    globalThis.fetch = vi.fn(async () => ({ ok: true, json: async () => ({ data: {} }) }));
    const r = await testApiBaseUrl('https://a.com');
    expect(r.ok).toBe(false);
    expect(r.error).toBe('no_dfid');
  });

  it('JSON 解析失败不抛错，按缺少 dfid 处理', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: true,
      json: async () => { throw new Error('bad json'); },
    }));
    const r = await testApiBaseUrl('https://a.com');
    expect(r.ok).toBe(false);
    expect(r.error).toBe('no_dfid');
  });

  it('HTTP 非 2xx 返回状态码', async () => {
    globalThis.fetch = vi.fn(async () => ({ ok: false, status: 502, statusText: 'Bad Gateway' }));
    const r = await testApiBaseUrl('https://a.com');
    expect(r).toEqual({ ok: false, status: 502, statusText: 'Bad Gateway' });
  });

  it('超时被识别为 timeout', async () => {
    globalThis.fetch = vi.fn(async () => {
      const err = new Error('aborted');
      err.name = 'AbortError';
      throw err;
    });
    const r = await testApiBaseUrl('https://a.com', { timeoutMs: 5 });
    expect(r).toEqual({ ok: false, error: 'timeout' });
  });

  it('其他异常回传 message', async () => {
    globalThis.fetch = vi.fn(async () => { throw new Error('ECONNREFUSED'); });
    const r = await testApiBaseUrl('https://a.com');
    expect(r).toEqual({ ok: false, error: 'ECONNREFUSED' });
  });

  it('支持自定义探测路径', async () => {
    globalThis.fetch = vi.fn(async () => ({ ok: true, json: async () => ({ data: { dfid: 'd' } }) }));
    await testApiBaseUrl('https://a.com', { path: '/ping' });
    expect(globalThis.fetch.mock.calls[0][0]).toBe('https://a.com/ping');
  });
});
