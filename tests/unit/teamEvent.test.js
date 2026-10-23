// tests/unit/teamEvent.test.js
// 组队活动：邀请文案分支、创建/加入队伍的成功与失败路径、弹窗单例与销毁。
// 该模块是模块级单例状态（currentPeriod / 弹窗开关），因此每个用例都重置模块图。
vi.mock('@/utils/request', () => {
  const get = vi.fn();
  const post = vi.fn();
  globalThis.__req = { get, post };
  return { get, post };
});
vi.mock('@/components/TeamEvent.vue', () => ({
  default: { name: 'TeamEvent', props: ['closePopup', 'status'], render: () => null },
}));

let actions;
let createTeamEventPopup;
let get;
let post;

/** refreshStatus 的三次请求按顺序返回的默认成功数据 */
function mockRefreshOk(periodId = 77) {
  get.mockImplementation(async (url) => {
    if (url.startsWith('/team/period/info')) {
      return { status: 1, data: { current_period_info: { id: periodId } } };
    }
    if (url.startsWith('/team/my/status')) {
      return { data: { is_create_team: true, is_join_team: true } };
    }
    if (url.startsWith('/team/my/info')) {
      return {
        data: {
          my_create_team_list: [{ team_id: 1 }],
          my_join_team_list: [{ team_id: 2 }],
        },
      };
    }
    return { data: {} };
  });
}

/** 让模块私有的 currentPeriod 从 0 变为有效值（只能通过 refreshStatus 间接驱动） */
async function primePeriod() {
  actions.refreshStatus();
  await vi.waitFor(() => {
    expect(get.mock.calls.some((c) => String(c[0]).startsWith('/team/my/info'))).toBe(true);
  });
}

beforeEach(async () => {
  vi.resetModules();
  ({ actions, createTeamEventPopup } = await import('@/utils/teamEvent'));
  ({ get, post } = globalThis.__req);
  get.mockReset();
  post.mockReset();
  delete window.electron;
  mockRefreshOk();
});

afterEach(() => {
  document.getElementById('team-event')?.remove();
});

describe('copyTeamCode', () => {
  it('桌面端使用深链文案', async () => {
    window.electron = { platform: 'win32' };
    await actions.copyTeamCode('ABC123');
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      expect.stringContaining('moekoe://team/join?code=ABC123')
    );
    expect(window.$message.success).toHaveBeenCalled();
  });

  it('Web 端回退为纯队伍码文案', async () => {
    await actions.copyTeamCode('XYZ');
    const text = navigator.clipboard.writeText.mock.calls[0][0];
    expect(text).toContain('XYZ');
    expect(text).not.toContain('moekoe://');
  });

  it('空 code 视为失败并提示', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    await actions.copyTeamCode('');
    expect(window.$message.error).toHaveBeenCalledWith('复制失败!');
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    errSpy.mockRestore();
  });

  it('剪贴板拒绝时提示失败', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    navigator.clipboard.writeText.mockRejectedValueOnce(new Error('denied'));
    await actions.copyTeamCode('A');
    expect(window.$message.error).toHaveBeenCalledWith('复制失败!');
    errSpy.mockRestore();
  });
});

describe('createTeam', () => {
  it('当期未就绪时直接失败提示（不发起创建请求）', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    await actions.createTeam();
    expect(window.$message.error).toHaveBeenCalledWith('创建队伍失败');
    expect(get.mock.calls.some((c) => String(c[0]).startsWith('/team/my?'))).toBe(false);
    errSpy.mockRestore();
  });

  it('创建成功后刷新状态并提示', async () => {
    await primePeriod();
    get.mockImplementation(async (url) => {
      if (url.startsWith('/team/perio')) return { status: 1, data: { current_period_info: { id: 77 } } };
      if (url.startsWith('/team/my?') || url.includes('/team/my?period_id')) {
        return { data: { team_id: 'T1' } };
      }
      if (url.startsWith('/team/my/status')) return { data: {} };
      if (url.startsWith('/team/my/info')) return { data: { my_create_team_list: [], my_join_team_list: [] } };
      return { data: {} };
    });

    await actions.createTeam();
    expect(window.$message.success).toHaveBeenCalledWith('已成功创建队伍!');
  });

  it('响应无 team_id 时报错', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    await primePeriod();
    get.mockImplementation(async (url) => {
      if (url.startsWith('/team/perio')) return { status: 1, data: { current_period_info: { id: 77 } } };
      if (url.includes('/team/my?period_id')) return { data: {}, error_msg: '已达上限' };
      return { data: {} };
    });

    await actions.createTeam();
    expect(window.$message.error).toHaveBeenCalledWith('已达上限');
    errSpy.mockRestore();
  });
});

describe('joinTeam', () => {
  it('传入队伍码直接加入', async () => {
    post.mockResolvedValue({ status: 1 });
    await actions.joinTeam('CODE1');
    expect(post).toHaveBeenCalledWith(expect.stringContaining('/team/join'), { team_code: 'CODE1' });
    expect(window.$message.success).toHaveBeenCalledWith('已成功加入队伍!');
  });

  it('未传队伍码时走弹窗输入', async () => {
    window.$modal.prompt = vi.fn(() => Promise.resolve('INPUT'));
    post.mockResolvedValue({ status: 1 });
    await actions.joinTeam();
    expect(post).toHaveBeenCalledWith(expect.any(String), { team_code: 'INPUT' });
  });

  it('用户取消输入时不发请求', async () => {
    window.$modal.prompt = vi.fn(() => Promise.resolve(null));
    await actions.joinTeam();
    expect(post).not.toHaveBeenCalled();
  });

  it('上游返回 error_code 时提示错误信息', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    post.mockResolvedValue({ error_code: 30001, error_msg: '队伍不存在' });
    await actions.joinTeam('BAD');
    expect(window.$message.error).toHaveBeenCalledWith('队伍不存在');
    errSpy.mockRestore();
  });

  it('请求抛错时提示兜底文案', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    post.mockRejectedValue(new Error('network'));
    await actions.joinTeam('X');
    expect(window.$message.error).toHaveBeenCalledWith('加入队伍失败');
    errSpy.mockRestore();
  });
});

describe('refreshStatus', () => {
  it('成功后提示刷新成功（fire-and-forget，需等内部 then）', async () => {
    actions.refreshStatus();
    await vi.waitFor(() => {
      expect(window.$message.success).toHaveBeenCalledWith('刷新成功');
    });
  });

  it('当期活动获取失败时弹窗告警', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    get.mockResolvedValue({ status: 0, error_msg: '活动未开始', data: {} });
    await actions.refreshStatus();
    expect(window.$modal.alert).toHaveBeenCalledWith(expect.stringContaining('活动未开始'));
    errSpy.mockRestore();
  });
});

describe('createTeamEventPopup', () => {
  it('挂载弹窗容器，重复调用不重复挂载', async () => {
    await createTeamEventPopup();
    expect(document.getElementById('team-event')).not.toBeNull();

    await createTeamEventPopup();
    expect(document.querySelectorAll('#team-event')).toHaveLength(1);
  });
});
