// tests/unit/utils.test.js
// 纯工具函数测试：URL 升级、封面处理、时间格式化。
// 注意：utils.js 顶部 import 了 i18n（仅 share 函数使用），测试中不调用 share，故打桩避免加载 vue-i18n。
vi.mock('@/utils/i18n', () => ({
  default: { global: { t: (k) => k } },
}));

import {
  ensureHttps,
  getCover,
  formatMilliseconds,
  formatTimestampToAgo,
  formatDateTime,
} from '@/utils/utils';

describe('ensureHttps', () => {
  it('将 http:// 升级为 https://', () => {
    expect(ensureHttps('http://a.com/x.png')).toBe('https://a.com/x.png');
  });
  it('保留已有的 https://', () => {
    expect(ensureHttps('https://a.com/x.png')).toBe('https://a.com/x.png');
  });
  it('不处理 data:/blob:/相对路径', () => {
    expect(ensureHttps('data:image/png;base64,xxx')).toBe('data:image/png;base64,xxx');
    expect(ensureHttps('/assets/x.png')).toBe('/assets/x.png');
    expect(ensureHttps('')).toBe('');
  });
});

describe('getCover', () => {
  it('无封面时回退默认图标', () => {
    expect(getCover('')).toBe('./assets/images/ico.png');
    expect(getCover(null)).toBe('./assets/images/ico.png');
  });
  it('替换 {size} 并升级为 https', () => {
    expect(getCover('http://img/{size}', 480)).toBe('https://img/480');
  });
});

describe('formatMilliseconds', () => {
  it('毫秒 > 3600 直接按毫秒解读', () => {
    expect(formatMilliseconds(65000)).toBe('1分5秒'); // 65s
  });
  it('秒级数值（< 3600）被当作毫秒放大', () => {
    // 125 视为 125000ms = 125s = 2分5秒
    expect(formatMilliseconds(125)).toBe('2分5秒');
  });
});

describe('formatTimestampToAgo', () => {
  const now = Math.floor(Date.now() / 1000);
  it('非法/零时间戳返回空', () => {
    expect(formatTimestampToAgo(0)).toBe('');
    expect(formatTimestampToAgo('abc')).toBe('');
  });
  it('当前时刻返回 刚刚', () => {
    expect(formatTimestampToAgo(now)).toBe('刚刚');
  });
  it('不同粒度', () => {
    expect(formatTimestampToAgo(now - 5)).toBe('5秒前');
    expect(formatTimestampToAgo(now - 65)).toBe('1分钟前');
    expect(formatTimestampToAgo(now - 3600)).toBe('1小时前');
    expect(formatTimestampToAgo(now - 86400)).toBe('1天前');
  });
});

describe('formatDateTime', () => {
  it('按指定格式格式化（补零）', () => {
    const d = new Date(2026, 0, 15, 13, 5, 9); // 2026-01-15 13:05:09
    expect(formatDateTime(d, 'yyyy-MM-dd HH:mm:ss')).toBe('2026-01-15 13:05:09');
  });
  it('默认格式', () => {
    const d = new Date(2026, 10, 3, 8, 0, 0); // 2026-11-03 08:00:00
    expect(formatDateTime(d)).toBe('2026-11-03 08:00:00');
  });
});
