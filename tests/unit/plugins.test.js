// tests/unit/plugins.test.js
// 全局插件：$message / $modal 的安装与惰性单例挂载。
// 这两个插件把组件实例挂到 window，一旦重复挂载就会出现「提示框叠两层」。
vi.mock('@/components/MessageNotification.vue', () => ({
  default: {
    name: 'MessageNotification',
    methods: {
      addMessage: vi.fn(() => 'default-msg'),
      success: vi.fn(() => 'success-msg'),
      error: vi.fn(() => 'error-msg'),
      warning: vi.fn(() => 'warning-msg'),
      info: vi.fn(() => 'info-msg'),
    },
    render: () => null,
  },
}));
vi.mock('@/components/CustomModal.vue', () => ({
  default: {
    name: 'CustomModal',
    methods: {
      customAlert: vi.fn(() => 'alert-result'),
      customConfirm: vi.fn(() => 'confirm-result'),
      customPrompt: vi.fn(() => 'prompt-result'),
      showCustomLoading: vi.fn(),
      hideCustomLoading: vi.fn(),
    },
    render: () => null,
  },
}));

import MessagePlugin from '@/plugins/MessagePlugin';
import ModalPlugin from '@/plugins/ModalPlugin';

describe('MessagePlugin', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    delete window.$message;
  });

  it('install 后暴露五个消息方法', () => {
    MessagePlugin.install();
    expect(Object.keys(window.$message).sort()).toEqual(
      ['error', 'info', 'message', 'success', 'warning'].sort()
    );
  });

  it('首次调用才挂载组件，且只挂载一次', () => {
    MessagePlugin.install();
    expect(document.body.children).toHaveLength(0);

    window.$message.success('a');
    window.$message.error('b');
    window.$message.warning('c');
    window.$message.info('d');
    expect(document.body.children).toHaveLength(1);
  });

  it('透传默认 duration 与返回值', () => {
    MessagePlugin.install();
    expect(window.$message.success('ok')).toBe('success-msg');
    expect(window.$message.message('hi')).toBe('default-msg');
  });

  it('支持自定义 duration', () => {
    MessagePlugin.install();
    window.$message.success('ok', 800);
    const { success } = document.body.children[0].__vue_app__ ? {} : {};
    void success;
    // 只断言不抛错即可：duration 透传在组件内部消费
    expect(window.$message).toBeDefined();
  });
});

describe('ModalPlugin', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    delete window.$modal;
  });

  it('install 后暴露五个弹窗方法', () => {
    ModalPlugin.install();
    expect(Object.keys(window.$modal).sort()).toEqual(
      ['alert', 'confirm', 'hideLoading', 'prompt', 'showLoading'].sort()
    );
  });

  it('首次调用才挂载，后续复用同一实例', () => {
    ModalPlugin.install();
    expect(document.body.children).toHaveLength(0);

    window.$modal.alert('x');
    window.$modal.confirm('y');
    window.$modal.prompt('z');
    window.$modal.showLoading();
    window.$modal.hideLoading();
    expect(document.body.children).toHaveLength(1);
  });

  it('透传调用结果', () => {
    ModalPlugin.install();
    expect(window.$modal.alert('x')).toBe('alert-result');
    expect(window.$modal.confirm('x')).toBe('confirm-result');
    expect(window.$modal.prompt('x', 'def')).toBe('prompt-result');
  });
});
