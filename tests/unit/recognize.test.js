// tests/unit/recognize.test.js
// 听歌识曲映射逻辑测试：验证 PCM -> 酷狗指纹响应 -> 标准化结果 的字段映射与置信度计算。
// recognize.js 调用 @tauri-apps/api/core 的 invoke，这里整体打桩。
vi.mock('@/utils/i18n', () => ({ default: { global: { t: (k) => k } } }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { recognizeAudio } from '@/utils/recognize';

describe('recognizeAudio', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('将酷狗指纹响应映射为标准结果，并计算置信度', async () => {
    invoke.mockResolvedValue({
      status: 1,
      data: [
        {
          hash: 'h1',
          songname: 'Song A',
          singername: 'Singer A',
          album_name: 'Album A',
          union_cover: 'http://cover/{size}',
          timelength: 200000,
          dist: 0.2,
        },
      ],
    });

    const result = await recognizeAudio(new Uint8Array([1, 2, 3]));
    expect(result).toHaveLength(1);
    const m = result[0];
    expect(m.confidence).toBeCloseTo(0.8, 5); // 1 - dist
    expect(m.song.FileHash).toBe('h1');
    expect(m.song.SongName).toBe('Song A');
    expect(m.song.SingerName).toBe('Singer A');
    expect(m.song.OriSongName).toBe('Singer A - Song A');
    expect(m.song.AlbumName).toBe('Album A');
    expect(m.song.Image).toBe('https://cover/480');
    expect(m.song.Duration).toBe(200000); // >1000 不缩放
    expect(invoke).toHaveBeenCalledWith('recognize_audio', { pcm: expect.any(Uint8Array) });
  });

  it('status 非 1 或 data 非数组时返回空数组', async () => {
    invoke.mockResolvedValue({ status: 0, data: [] });
    expect(await recognizeAudio(new Uint8Array([1]))).toEqual([]);
    invoke.mockResolvedValue({ status: 1, data: 'not-array' });
    expect(await recognizeAudio(new Uint8Array([1]))).toEqual([]);
  });

  it('过滤掉缺少 hash 的命中项', async () => {
    invoke.mockResolvedValue({
      status: 1,
      data: [{ songname: 'no hash', singername: 'x', dist: 0.1 }],
    });
    expect(await recognizeAudio(new Uint8Array([1]))).toEqual([]);
  });

  it('invoke 抛错时降级为空数组', async () => {
    invoke.mockRejectedValue(new Error('closed'));
    expect(await recognizeAudio(new Uint8Array([1]))).toEqual([]);
  });

  it('结果按置信度降序排序', async () => {
    invoke.mockResolvedValue({
      status: 1,
      data: [
        { hash: 'low', songname: 'a', singername: 'b', dist: 0.9 },
        { hash: 'high', songname: 'c', singername: 'd', dist: 0.1 },
      ],
    });
    const r = await recognizeAudio(new Uint8Array([1]));
    expect(r[0].song.FileHash).toBe('high');
    expect(r[1].song.FileHash).toBe('low');
  });
});
