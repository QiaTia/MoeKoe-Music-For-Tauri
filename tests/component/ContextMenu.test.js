// tests/component/ContextMenu.test.js
// 歌曲右键菜单：显示定位、登录态分支、子菜单歌单过滤、各操作入口的副作用。
// 菜单项依赖「登录态 + 歌曲字段 + 收藏归属」三重条件，条件写错就会出现点了没反应的按钮。
const authRef = (globalThis.__auth = {
  isAuthenticated: false,
  UserInfo: null,
});

vi.mock('@/stores/store', () => ({
  MoeAuthStore: () => globalThis.__auth,
}));
vi.mock('@/utils/request', () => {
  const get = vi.fn();
  globalThis.__reqGet = get;
  return { get };
});
vi.mock('@/utils/utils', () => ({
  share: vi.fn(),
  openMvPlayer: vi.fn(),
}));
vi.mock('@/utils/i18n', () => ({ default: { global: { t: (k) => k } } }));
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }));

import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import ContextMenu from '@/components/ContextMenu.vue';
import { get } from '@/utils/request';
import { share, openMvPlayer } from '@/utils/utils';

const SONG = {
  FileHash: 'HASH1',
  fileid: 'F1',
  OriSongName: '周杰伦 - 晴天',
  mvhash: '',
  userid: '100',
};

const playerControl = { addToNext: vi.fn(), pause: vi.fn() };

function mountMenu() {
  return mount(ContextMenu, {
    props: { playerControl },
    global: { mocks: { $t: (k) => k } },
  });
}

function openMenu(wrapper, song = SONG, listId = 0) {
  const event = { preventDefault: vi.fn(), clientX: 100, clientY: 120 };
  wrapper.vm.openContextMenu(event, song, listId);
  return event;
}

beforeEach(() => {
  globalThis.__auth = { isAuthenticated: false, UserInfo: null };
  get.mockReset();
  share.mockReset();
  openMvPlayer.mockReset();
  playerControl.addToNext.mockReset();
  playerControl.pause.mockReset();
});

describe('显示与定位', () => {
  it('初始不渲染', () => {
    const w = mountMenu();
    expect(w.find('.context-menu').exists()).toBe(false);
  });

  it('打开后按鼠标位置定位，并阻止浏览器默认菜单', async () => {
    const w = mountMenu();
    const event = openMenu(w);
    await nextTick();
    await nextTick();

    expect(event.preventDefault).toHaveBeenCalled();
    const style = w.find('.context-menu').element.getAttribute('style');
    expect(style).toContain('top: 120px');
    expect(style).toContain('left: 100px');
  });

  it('点击外部关闭', async () => {
    const w = mountMenu();
    openMenu(w);
    await nextTick();

    const outside = document.createElement('div');
    document.body.appendChild(outside);
    outside.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await nextTick();

    expect(w.find('.context-menu').exists()).toBe(false);
  });

  it('页面滚动时关闭', async () => {
    const w = mountMenu();
    openMenu(w);
    await nextTick();

    document.dispatchEvent(new Event('scroll'));
    await nextTick();
    expect(w.find('.context-menu').exists()).toBe(false);
  });
});

describe('登录态分支', () => {
  it('未登录只显示「请先登录」入口，不渲染子菜单', async () => {
    const w = mountMenu();
    openMenu(w);
    await nextTick();

    expect(w.text()).toContain('qing-xian-deng-lu');
    expect(w.find('.submenu').exists()).toBe(false);
    expect(w.text()).not.toContain('添加到下一首');
  });

  it('已登录显示「添加歌单」与「添加到下一首」', async () => {
    globalThis.__auth = { isAuthenticated: true, UserInfo: { userid: '100' } };
    const w = mountMenu();
    openMenu(w);
    await nextTick();

    expect(w.text()).toContain('tian-jia-ge-dan');
    expect(w.text()).toContain('添加到下一首');
  });

  it('悬停「添加歌单」拉取并只保留自己创建的歌单', async () => {
    globalThis.__auth = { isAuthenticated: true, UserInfo: { userid: '100' } };
    get.mockResolvedValue({
      status: 1,
      data: {
        info: [
          { listid: 1, name: '我的收藏', list_create_userid: '100' },
          { listid: 2, name: '别人的歌单', list_create_userid: '999' },
        ],
      },
    });

    const w = mountMenu();
    openMenu(w);
    await nextTick();

    await w.findAll('li')[0].trigger('mouseenter');
    await nextTick();
    await nextTick();

    const items = w.findAll('.submenu li');
    expect(items).toHaveLength(1);
    expect(items[0].text()).toBe('我的收藏');
  });

  it('歌单拉取失败时提示错误且不抛未捕获异常', async () => {
    globalThis.__auth = { isAuthenticated: true, UserInfo: { userid: '100' } };
    get.mockRejectedValue(new Error('network'));

    const w = mountMenu();
    openMenu(w);
    await nextTick();
    await w.findAll('li')[0].trigger('mouseenter');
    await new Promise((r) => setTimeout(r, 0));

    expect(window.$message.error).toHaveBeenCalledWith('huo-qu-ge-dan-shi-bai');
  });

  it('取消收藏仅在自己的歌单中显示', async () => {
    globalThis.__auth = { isAuthenticated: true, UserInfo: { userid: '100' } };
    const w = mountMenu();

    openMenu(w, SONG, 0);
    await nextTick();
    expect(w.text()).not.toContain('取消收藏');

    openMenu(w, { ...SONG, userid: '999' }, 88);
    await nextTick();
    expect(w.text()).not.toContain('取消收藏');

    openMenu(w, SONG, 88);
    await nextTick();
    expect(w.text()).toContain('取消收藏');
  });
});

describe('操作入口', () => {
  it('分享调用 share 并关闭菜单', async () => {
    const w = mountMenu();
    openMenu(w);
    await nextTick();

    const shareItem = w.findAll('li').find((li) => li.text().includes('分享'));
    await shareItem.trigger('click');

    expect(share).toHaveBeenCalledWith('周杰伦 - 晴天', 'HASH1');
    expect(w.find('.context-menu').exists()).toBe(false);
  });

  it('添加到下一首透传歌曲信息并提示成功', async () => {
    globalThis.__auth = { isAuthenticated: true, UserInfo: { userid: '100' } };
    const w = mountMenu();
    openMenu(w, { ...SONG, cover: 'c.png', timeLength: 200000 });
    await nextTick();

    const item = w.findAll('li').find((li) => li.text().includes('添加到下一首'));
    await item.trigger('click');

    expect(playerControl.addToNext).toHaveBeenCalledWith(
      'HASH1',
      '晴天',
      'c.png',
      '周杰伦',
      200000
    );
    expect(window.$message.success).toHaveBeenCalled();
  });

  it('取消收藏调用删除接口并通知父组件', async () => {
    globalThis.__auth = { isAuthenticated: true, UserInfo: { userid: '100' } };
    get.mockResolvedValue({ status: 1 });
    const w = mountMenu();
    openMenu(w, SONG, 88);
    await nextTick();

    const item = w.findAll('li').find((li) => li.text().includes('取消收藏'));
    await item.trigger('click');
    await nextTick();

    expect(get).toHaveBeenCalledWith('/playlist/tracks/del?listid=88&fileids=F1');
    expect(w.emitted('songRemoved')).toEqual([['F1']]);
  });

  it('有 mvhash 时才出现播放 MV，点击先暂停再开窗', async () => {
    const w = mountMenu();
    openMenu(w);
    await nextTick();
    expect(w.text()).not.toContain('mv');

    openMenu(w, { ...SONG, mvhash: 'MV1' });
    await nextTick();
    const item = w.findAll('li').find((li) => li.text().includes('MV'));
    await item.trigger('click');
    await nextTick();

    expect(playerControl.pause).toHaveBeenCalled();
    expect(openMvPlayer).toHaveBeenCalledWith(expect.anything(), 'MV1', '周杰伦 - 晴天');
  });
});
