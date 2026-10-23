// tests/unit/authStore.test.js
// 登录态 store：配置读取、状态写入、设备注册的双通道（invoke / HTTP）与失败降级。
vi.mock('axios', () => {
  const get = vi.fn();
  return { default: { create: () => ({ get }), __get: get } };
});
vi.mock('@/utils/invokeBridge', () => ({
  shouldUseInvoke: vi.fn(),
  invokeApi: vi.fn(),
}));

import axios from 'axios';
import { setActivePinia, createPinia } from 'pinia';
import { shouldUseInvoke, invokeApi } from '@/utils/invokeBridge';
import { MoeAuthStore } from '@/stores/store';

const httpGet = axios.__get;

describe('MoeAuthStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    httpGet.mockReset();
    invokeApi.mockReset();
    shouldUseInvoke.mockReset();
  });

  describe('fetchConfig', () => {
    it('Config 未加载时返回 null（不抛错）', () => {
      expect(MoeAuthStore().fetchConfig('theme')).toBeNull();
    });

    it('按键取值', () => {
      const store = MoeAuthStore();
      store.Config = [
        { key: 'theme', value: 'dark' },
        { key: 'lang', value: 'zh' },
      ];
      expect(store.fetchConfig('theme')).toBe('dark');
      expect(store.fetchConfig('missing')).toBeNull();
    });
  });

  describe('setData / clearData', () => {
    it('只写入传入的字段，未传的保持原值', () => {
      const store = MoeAuthStore();
      store.Config = [{ key: 'a', value: 1 }];
      store.setData({ UserInfo: { userid: 9 } });
      expect(store.UserInfo).toEqual({ userid: 9 });
      expect(store.Config).toEqual([{ key: 'a', value: 1 }]);
    });

    it('clearData 只清用户信息，保留配置与设备', () => {
      const store = MoeAuthStore();
      store.UserInfo = { userid: 9 };
      store.Config = [{ key: 'a', value: 1 }];
      store.Device = { dfid: 'd' };
      store.clearData();
      expect(store.UserInfo).toBeNull();
      expect(store.Config).toHaveLength(1);
      expect(store.Device).toEqual({ dfid: 'd' });
    });
  });

  describe('isAuthenticated', () => {
    it('有用户信息即为已登录', () => {
      const store = MoeAuthStore();
      expect(store.isAuthenticated).toBe(false);
      store.UserInfo = { userid: 1 };
      expect(store.isAuthenticated).toBe(true);
      store.UserInfo = null;
      expect(store.isAuthenticated).toBe(false);
    });
  });

  describe('initDevice', () => {
    it('已注册过设备时直接复用，不重复请求', async () => {
      const store = MoeAuthStore();
      store.Device = { dfid: 'cached' };
      await expect(store.initDevice()).resolves.toEqual({ dfid: 'cached' });
      expect(invokeApi).not.toHaveBeenCalled();
      expect(httpGet).not.toHaveBeenCalled();
    });

    it('Tauri 环境走 invoke 直调', async () => {
      shouldUseInvoke.mockReturnValue(true);
      invokeApi.mockResolvedValue({ status: 1, data: { dfid: 'from-invoke' } });
      const store = MoeAuthStore();

      await expect(store.initDevice()).resolves.toEqual({ dfid: 'from-invoke' });
      expect(invokeApi).toHaveBeenCalledWith('GET', '/register/dev');
      expect(store.Device).toEqual({ dfid: 'from-invoke' });
      expect(httpGet).not.toHaveBeenCalled();
    });

    it('Web/Electron 环境走 HTTP', async () => {
      shouldUseInvoke.mockReturnValue(false);
      httpGet.mockResolvedValue({ data: { data: { dfid: 'from-http' } } });
      const store = MoeAuthStore();

      await expect(store.initDevice()).resolves.toEqual({ dfid: 'from-http' });
      expect(httpGet).toHaveBeenCalledWith('/register/dev');
      expect(invokeApi).not.toHaveBeenCalled();
    });

    it('响应无 data 时返回 null 且不写入 Device', async () => {
      shouldUseInvoke.mockReturnValue(false);
      httpGet.mockResolvedValue({ data: {} });
      const store = MoeAuthStore();

      await expect(store.initDevice()).resolves.toBeNull();
      expect(store.Device).toBeNull();
    });

    it('请求失败时降级为 null，不向上抛（避免启动被阻断）', async () => {
      const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      shouldUseInvoke.mockReturnValue(true);
      invokeApi.mockRejectedValue(new Error('network down'));
      const store = MoeAuthStore();

      await expect(store.initDevice()).resolves.toBeNull();
      expect(errSpy).toHaveBeenCalled();
      errSpy.mockRestore();
    });
  });
});
