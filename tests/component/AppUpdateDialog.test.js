// tests/component/AppUpdateDialog.test.js
// 更新弹窗的状态机：下载完成后再报错必须能看见「启动安装程序失败」，
// 否则用户会永久停在「正在启动安装程序…」且没有重试入口（P1-3 回归保护）。
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (k) => k }) }));
vi.mock('@/components/MarkdownContent.vue', () => ({
    default: { name: 'MarkdownContent', template: '<div class="markdown-stub" />' }
}));
vi.mock('@/utils/utils', () => ({ openRegisterUrl: vi.fn() }));

import { mount, flushPromises } from '@vue/test-utils';
import { nextTick } from 'vue';
import AppUpdateDialog from '@/components/AppUpdateDialog.vue';

/** 伪造的 IPC 监听表：组件 onMounted 会把 handler 注册进来 */
const listeners = {};

function emitMain(channel, payload) {
    (listeners[channel] || []).forEach((fn) => fn({}, payload));
}

function installFakes() {
    window.electron = {
        platform: 'win32',
        ipcRenderer: {
            on: (channel, fn) => {
                (listeners[channel] = listeners[channel] || []).push(fn);
            },
            removeListener: (channel, fn) => {
                listeners[channel] = (listeners[channel] || []).filter((f) => f !== fn);
            }
        }
    };
    window.electronAPI = {
        checkUpdate: vi.fn(async () => ({
            success: true,
            available: true,
            latestVersion: '1.7.2',
            notes: '- 修复若干问题',
            htmlUrl: 'https://example.com/releases/v1.7.2',
            assetName: 'a.exe'
        })),
        startUpdateDownload: vi.fn(async () => ({ success: true })),
        cancelUpdate: vi.fn(async () => ({ success: true })),
        installUpdate: vi.fn(async () => ({ success: true })),
        updateGatekeeperScript: vi.fn(async () => '')
    };
    window.$modal = { alert: vi.fn(async () => {}) };
    window.$message = { success: vi.fn(), error: vi.fn() };
}

/** 打开弹窗并进入「下载中」状态 */
async function mountAndDownloading() {
    const wrapper = mount(AppUpdateDialog, {
        props: { appVersion: '1.7.1', platform: 'win32' }
    });
    await flushPromises();
    await nextTick();

    wrapper.vm.handleEntryClick();
    await nextTick();

    const primary = wrapper.findAll('.update-dialog-actions button')[0];
    await primary.trigger('click');
    await flushPromises();
    return wrapper;
}

beforeEach(() => {
    Object.keys(listeners).forEach((k) => delete listeners[k]);
    installFakes();
});

afterEach(() => {
    document.body.innerHTML = '';
    delete window.electron;
    delete window.electronAPI;
});

describe('更新弹窗错误处理', () => {
    it('下载完成后安装器启动失败：必须显示失败原因并保留重试入口', async () => {
        const wrapper = await mountAndDownloading();

        emitMain('update-downloaded', { path: '/tmp/a.exe', version: '1.7.2' });
        await nextTick();
        expect(wrapper.find('.update-status').text()).toContain('正在启动安装程序');

        emitMain('update-error', { message: '无法打开安装包: permission denied' });
        await nextTick();

        const status = wrapper.find('.update-status').text();
        expect(status).toContain('启动安装程序失败');
        expect(status).toContain('permission denied');
        // 仍可重试：按钮为「运行安装包」而非退回「在线更新」
        expect(wrapper.text()).toContain('运行安装包');

        wrapper.unmount();
    });

    it('下载过程中失败：直接展示后端错误信息', async () => {
        const wrapper = await mountAndDownloading();

        emitMain('update-error', { message: '下载中断: connection reset' });
        await nextTick();

        expect(wrapper.find('.update-status').text()).toBe('下载中断: connection reset');
        // 未完成下载，按钮回到「在线更新」可重试
        expect(wrapper.text()).toContain('在线更新');

        wrapper.unmount();
    });

    it('用户取消：状态为已取消且进度归零', async () => {
        const wrapper = await mountAndDownloading();

        emitMain('update-progress', { percent: 42.5, transferred: 425, total: 1000, stage: 'downloading' });
        await nextTick();
        expect(wrapper.find('.update-progress-percent').text()).toBe('42.5%');

        emitMain('update-error', { message: '已取消更新', cancelled: true });
        await nextTick();

        expect(wrapper.find('.update-status').text()).toBe('已取消更新');
        // 取消后进度区域整体收起（未开始/未完成态不展示进度条）
        expect(wrapper.find('.update-progress-inline').exists()).toBe(false);
        expect(wrapper.text()).toContain('在线更新');

        wrapper.unmount();
    });
});
