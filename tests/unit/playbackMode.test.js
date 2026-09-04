// tests/unit/playbackMode.test.js
// 播放模式状态机：顺序/随机/单曲/列表循环 四态切换与 localStorage 持久化。
import usePlaybackMode from '@/components/player/PlaybackMode';

const identity = (k) => k;

describe('usePlaybackMode', () => {
  let audio;
  beforeEach(() => {
    localStorage.clear();
    audio = { loop: false };
  });

  it('默认模式为列表循环(index=1)，未开启单曲 loop', () => {
    const { currentPlaybackModeIndex, initPlaybackMode } = usePlaybackMode(identity, audio);
    initPlaybackMode();
    expect(currentPlaybackModeIndex.value).toBe(1);
    expect(audio.loop).toBe(false);
  });

  it('initPlaybackMode 读取持久化模式', () => {
    localStorage.setItem('player_playback_mode', '2');
    const { currentPlaybackModeIndex, initPlaybackMode } = usePlaybackMode(identity, audio);
    initPlaybackMode();
    expect(currentPlaybackModeIndex.value).toBe(2);
    expect(audio.loop).toBe(true); // 单曲循环
  });

  it('setPlaybackMode 写入 localStorage 并切换 audio.loop', () => {
    const { setPlaybackMode } = usePlaybackMode(identity, audio);
    setPlaybackMode(2);
    expect(localStorage.getItem('player_playback_mode')).toBe('2');
    expect(audio.loop).toBe(true);
    setPlaybackMode(0);
    expect(audio.loop).toBe(false);
  });

  it('togglePlaybackMode 在四态间循环', () => {
    const { currentPlaybackModeIndex, initPlaybackMode, togglePlaybackMode } =
      usePlaybackMode(identity, audio);
    initPlaybackMode(); // 1
    togglePlaybackMode(); // 2
    expect(currentPlaybackModeIndex.value).toBe(2);
    togglePlaybackMode(); // 3
    togglePlaybackMode(); // 0
    togglePlaybackMode(); // 1
    expect(currentPlaybackModeIndex.value).toBe(1);
  });
});
