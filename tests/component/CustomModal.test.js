// tests/component/CustomModal.test.js
// 全局弹窗组件：alert / confirm / prompt / loading 的显隐与 Promise 结算契约。
// 这些方法被业务代码以 `await $modal.xxx()` 使用，**Promise 永不 resolve 就是功能挂死**。
vi.mock('@/utils/i18n', () => ({ default: { global: { t: (k) => k } } }));

import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import CustomModal from '@/components/CustomModal.vue';

function mountModal() {
  return mount(CustomModal);
}

describe('customAlert', () => {
  it('显示消息并在点击确定后 resolve', async () => {
    const w = mountModal();
    let settled = false;
    const p = w.vm.customAlert('操作失败').then(() => { settled = true; });
    await nextTick();

    expect(w.find('.modal-overlay').exists()).toBe(true);
    expect(w.find('.modal h3').text()).toBe('操作失败');

    await w.find('.modal .btn').trigger('click');
    await p;
    expect(settled).toBe(true);
    expect(w.find('.modal-overlay').exists()).toBe(false);
  });

  it('支持对象形式配置与 small 尺寸', async () => {
    const w = mountModal();
    w.vm.customAlert({ message: '长文案', messageSize: 'small' });
    await nextTick();

    expect(w.find('.modal h3').classes()).toContain('small-message');
    expect(w.find('.modal .btn').classes()).toContain('small-button');
  });

  it('空消息降级为空串而非 undefined', async () => {
    const w = mountModal();
    w.vm.customAlert();
    await nextTick();
    expect(w.find('.modal h3').text()).toBe('');
  });
});

describe('customConfirm', () => {
  it('点确定 resolve true', async () => {
    const w = mountModal();
    const p = w.vm.customConfirm('确定删除？');
    await nextTick();

    await w.findAll('.buttons .btn')[0].trigger('click');
    await expect(p).resolves.toBe(true);
  });

  it('点取消 resolve false', async () => {
    const w = mountModal();
    const p = w.vm.customConfirm('确定删除？');
    await nextTick();

    await w.findAll('.buttons .btn')[1].trigger('click');
    await expect(p).resolves.toBe(false);
  });

  it('支持自定义按钮文案', async () => {
    const w = mountModal();
    w.vm.customConfirm({ message: 'x', confirmText: '删除', cancelText: '保留' });
    await nextTick();

    const [ok, cancel] = w.findAll('.buttons .btn');
    expect(ok.text()).toBe('删除');
    expect(cancel.text()).toBe('保留');
  });
});

describe('customPrompt', () => {
  it('带默认值打开并返回输入内容', async () => {
    const w = mountModal();
    const p = w.vm.customPrompt('请输入队伍码', 'ABC');
    await nextTick();

    const input = w.find('.prompt-input');
    expect(input.element.value).toBe('ABC');

    await input.setValue('XYZ');
    await w.findAll('.buttons .btn')[0].trigger('click');
    await expect(p).resolves.toBe('XYZ');
  });

  it('点取消返回 null（必须结算，否则调用方 await 永久挂起）', async () => {
    const w = mountModal();
    const p = w.vm.customPrompt('请输入');
    await nextTick();

    await w.findAll('.buttons .btn')[1].trigger('click');
    await expect(p).resolves.toBeNull();
    expect(w.find('.prompt-input').exists()).toBe(false);
  });

  it('对象配置中的 defaultValue 生效', async () => {
    const w = mountModal();
    w.vm.customPrompt({ message: '输入', defaultValue: '预设' });
    await nextTick();
    expect(w.find('.prompt-input').element.value).toBe('预设');
  });

  it('空默认值渲染为空串', async () => {
    const w = mountModal();
    w.vm.customPrompt('输入', '');
    await nextTick();
    expect(w.find('.prompt-input').element.value).toBe('');
  });
});

describe('loading 遮罩', () => {
  it('显示与隐藏', async () => {
    const w = mountModal();
    expect(w.find('.loading-overlay').exists()).toBe(false);

    w.vm.showCustomLoading();
    await nextTick();
    expect(w.find('.loading-overlay').exists()).toBe(true);

    w.vm.hideCustomLoading();
    await nextTick();
    expect(w.find('.loading-overlay').exists()).toBe(false);
  });
});

describe('多弹窗互不干扰', () => {
  it('alert 与 loading 可同时存在，各自独立关闭', async () => {
    const w = mountModal();
    w.vm.customAlert('提示');
    w.vm.showCustomLoading();
    await nextTick();

    expect(w.findAll('.modal-overlay')).toHaveLength(1);
    expect(w.find('.loading-overlay').exists()).toBe(true);

    w.vm.hideCustomLoading();
    await nextTick();
    expect(w.find('.modal-overlay').exists()).toBe(true);
    expect(w.find('.loading-overlay').exists()).toBe(false);
  });
});
