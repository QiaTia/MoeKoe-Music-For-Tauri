// tests/unit/musicQueue.test.js
// 播放队列 store：增删改查契约。队列是播放核心状态，索引错位会直接导致切歌错乱。
import { setActivePinia, createPinia } from 'pinia';
import { useMusicQueueStore } from '@/stores/musicQueue';

describe('useMusicQueueStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('初始队列为空', () => {
    expect(useMusicQueueStore().getQueue()).toEqual([]);
  });

  it('addSong 追加到队尾并保持顺序', () => {
    const store = useMusicQueueStore();
    store.addSong({ hash: 'a' });
    store.addSong({ hash: 'b' });
    expect(store.queue.map((s) => s.hash)).toEqual(['a', 'b']);
  });

  it('setQueue 整体替换（而非追加）', () => {
    const store = useMusicQueueStore();
    store.addSong({ hash: 'old' });
    store.setQueue([{ hash: 'x' }, { hash: 'y' }]);
    expect(store.queue.map((s) => s.hash)).toEqual(['x', 'y']);
  });

  it('removeSong 按索引移除', () => {
    const store = useMusicQueueStore();
    store.setQueue([{ hash: 'a' }, { hash: 'b' }, { hash: 'c' }]);
    store.removeSong(1);
    expect(store.queue.map((s) => s.hash)).toEqual(['a', 'c']);
  });

  it('removeSong(-1) 移除队尾（splice 语义，与现有播放逻辑一致）', () => {
    const store = useMusicQueueStore();
    store.setQueue([{ hash: 'a' }, { hash: 'b' }]);
    store.removeSong(-1);
    expect(store.queue.map((s) => s.hash)).toEqual(['a']);
  });

  it('clearQueue 清空', () => {
    const store = useMusicQueueStore();
    store.setQueue([{ hash: 'a' }]);
    store.clearQueue();
    expect(store.getQueue()).toEqual([]);
  });

  it('getQueue 返回同一引用（调用方按引用做切歌索引）', () => {
    const store = useMusicQueueStore();
    store.setQueue([{ hash: 'a' }]);
    expect(store.getQueue()).toBe(store.queue);
  });
});
