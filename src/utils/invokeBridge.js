// src/utils/invokeBridge.js
// Tauri invoke 桥：前端 → Rust 进程内直调酷狗 API（不经 HTTP，不占端口）。
//
// 路由策略：
// - Tauri 环境 && 未配置自定义远程 API 地址 → 走 invoke（api_request 命令）
// - 其他情况（纯 Web / Electron / 用户自定义远程 API）→ 走原 axios HTTP
//
// 返回值与错误形状对齐 axios 封装：
// - 2xx → resolve 响应 body（等价于原来拦截器返回的 response.data）
// - 非 2xx → reject 一个带 { response: { status, data } } 的 Error，
//   使现有 `error.response?.data` 判断逻辑无需改动。

import { invoke } from '@tauri-apps/api/core';

/** 是否运行在 Tauri 环境 */
export function isTauriEnv() {
    return typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
}

/** 用户是否配置了自定义远程 API 地址（配置了则尊重用户选择，继续走 HTTP） */
function hasCustomApiBaseUrl() {
    try {
        const settingsRaw = localStorage.getItem('settings');
        const settings = settingsRaw ? JSON.parse(settingsRaw) : {};
        return !!(settings?.apiBaseUrl && String(settings.apiBaseUrl).trim());
    } catch {
        return false;
    }
}

/** 当前请求是否应走 invoke 直调 */
export function shouldUseInvoke() {
    return isTauriEnv() && !hasCustomApiBaseUrl();
}

/**
 * 通过 Tauri invoke 调用 Rust API 分发器。
 * @param {string} method GET/POST/PUT/DELETE/PATCH
 * @param {string} url    如 '/search' 或 '/search?keywords=x'
 * @param {object} opts   { params, data, auth }
 * @returns {Promise<any>} 响应 body（JSON 信封）
 */
export async function invokeApi(method, url, { params = {}, data = null, auth = '' } = {}) {
    let resp;
    try {
        resp = await invoke('api_request', {
            method,
            path: url,
            params,
            body: data,
            auth: auth || null,
        });
    } catch (e) {
        // invoke 自身失败（命令不存在 / 序列化错误等）
        console.error('invoke api_request failed:', e);
        const error = new Error(typeof e === 'string' ? e : 'invoke failed');
        error.request = { method, url };
        throw error;
    }

    const status = resp?.status ?? 500;
    const body = resp?.body;

    if (status >= 200 && status < 300) {
        return body;
    }

    // 对齐 axios 的非 2xx 错误形状
    console.error(`invoke api error status:${status}`, body);
    if (body?.data) console.error(body.data);
    const error = new Error(`Request failed with status code ${status}`);
    error.response = { status, data: body };
    throw error;
}
