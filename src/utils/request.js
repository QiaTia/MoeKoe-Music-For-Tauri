// src/services/request.js
import axios from 'axios';
import { invoke } from '@tauri-apps/api/core';
import { MoeAuthStore } from '../stores/store';
import { getApiBaseUrl } from './apiBaseUrl';
import { shouldUseInvoke } from './invokeBridge';
import { handleRiskResponse } from './riskVerify';

// 创建一个 axios 实例
const httpClient = axios.create({
    baseURL: getApiBaseUrl(),
    timeout: 10000,
    headers: {
        'Content-Type': 'application/json',
    },
    withCredentials: true,
});

// 构造认证串（HTTP 模式作为 Authorization 头；invoke 模式作为 auth 参数）
function buildAuth() {
    const MoeAuth = MoeAuthStore();
    const token = MoeAuth.UserInfo?.token;
    const userid = MoeAuth.UserInfo?.userid;
    const t1 = MoeAuth.UserInfo?.t1;
    const dfid = MoeAuth.Device?.dfid;
    const mid = MoeAuth.Device?.mid;
    const guid = MoeAuth.Device?.guid;
    const serverDev = MoeAuth.Device?.serverDev;
    const mac = MoeAuth.Device?.mac;

    const authParts = [];
    if (token) authParts.push(`token=${(token)}`);
    if (userid) authParts.push(`userid=${(userid)}`);
    if (dfid) authParts.push(`dfid=${(dfid)}`);
    if (t1) authParts.push(`t1=${(t1)}`);
    if (mid) authParts.push(`KUGOU_API_MID=${(mid)}`);
    if (guid) authParts.push(`KUGOU_API_GUID=${(guid)}`);
    if (serverDev) authParts.push(`KUGOU_API_DEV=${(serverDev)}`);
    if (mac) authParts.push(`KUGOU_API_MAC=${(mac)}`);
    return authParts.join(';');
}

// 请求拦截器
httpClient.interceptors.request.use(
    config => {
        const auth = buildAuth();
        if (auth) {
            config.headers = {
                ...config.headers,
                Authorization: auth
            };
        }
        return config;
    },
    error => Promise.reject(error)
);

// 响应拦截器
httpClient.interceptors.response.use(
    async response => {
        if (response.config.__rawResponse) {
            return response;
        }

        const riskResult = await handleRiskResponse(response, (retryConfig) =>
            httpClient.request({ ...retryConfig, __rawResponse: true }),
            (url, params) => httpClient.get(url, { params, __skipRisk: true }),
        );
        if (riskResult.handled) {
            return riskResult.data;
        }

        return response.data;
    },
    async error => {
        if (error.config?.__rawResponse) {
            return Promise.reject(error);
        }

        if (error.response) {
            const riskResult = await handleRiskResponse(error.response, (retryConfig) =>
                httpClient.request({ ...retryConfig, __rawResponse: true }),
                (url, params) => httpClient.get(url, { params, __skipRisk: true }),
            );
            if (riskResult.handled) {
                return riskResult.data;
            }
        }

        if (error.response) {
            console.error(`http error status:${error.response.status}`,error.response.data);
            if (error.response?.data?.data) {
                console.error(error.response.data.data);
            // } else {
            //     $message.error('服务器错误,请稍后再试!');
            }
        } else if (error.request) {
            console.error('No response received:', error.request);
            $message.error('服务器未响应,请稍后再试!');
        } else {
            console.error('Error:', error.message);
            $message.error('请求错误,请稍后再试!');
        }
        return Promise.reject(error);
    }
);

// ── Tauri invoke 通道：进程内直调 + 风控接入 ──────────────────
// 对齐 axios 拦截器：把响应（含响应头 ssa-code）交给 handleRiskResponse，
// 命中风控（error_code 20028 / ssa-code）时弹出安全验证并自动重试。
const invokeApiResponse = async (method, url, { params = {}, data = null, auth = '' } = {}) => {
    const resp = await invoke('api_request', {
        method,
        path: url,
        params,
        body: data,
        auth: auth || null,
    });
    const status = resp?.status ?? 500;
    const body = resp?.body;
    const headers = resp?.headers ?? {};
    if (status >= 200 && status < 300) {
        return { status, data: body, headers, config: {} };
    }
    const error = new Error(`Request failed with status code ${status}`);
    error.response = { status, data: body, headers, config: {} };
    throw error;
};

const invokeWithRisk = async (method, url, { params = {}, data = null, auth = '' } = {}) => {
    const response = await invokeApiResponse(method, url, { params, data, auth });
    const riskResult = await handleRiskResponse(
        response,
        // 验证通过后重试原请求（invoke 直调，不走 HTTP）
        () => invokeApiResponse(method, url, { params, data, auth }),
        // 验证码信息/提交走原 axios（与 Electron 一致，避免再触发风控）
        (riskUrl, riskParams) => httpClient.get(riskUrl, { params: riskParams, __skipRisk: true }),
    );
    return riskResult.handled ? riskResult.data : response.data;
};

// 封装 GET 请求
export const get = async (url, params = {}, config = {}, onSuccess = null, onError = null) => {
    try {
        const response = shouldUseInvoke()
            ? await invokeWithRisk('GET', url, { params, auth: buildAuth() })
            : await httpClient.get(url, { params, ...config });
        if (onSuccess) onSuccess(response);
        return response;
    } catch (error) {
        if (onError) onError(error);
        throw error;
    }
};

// 封装 POST 请求
export const post = async (url, data = {}, config = {}, onSuccess = null, onError = null) => {
    try {
        const response = shouldUseInvoke()
            ? await invokeWithRisk('POST', url, { params: config?.params || {}, data, auth: buildAuth() })
            : await httpClient.post(url, data, config);
        if (onSuccess) onSuccess(response);
        return response;
    } catch (error) {
        if (onError) onError(error);
        throw error;
    }
};

// 封装 PUT 请求
export const put = async (url, data = {}, config = {}, onSuccess = null, onError = null) => {
    try {
        const response = shouldUseInvoke()
            ? await invokeWithRisk('PUT', url, { params: config?.params || {}, data, auth: buildAuth() })
            : await httpClient.put(url, data, config);
        if (onSuccess) onSuccess(response);
        return response;
    } catch (error) {
        if (onError) onError(error);
        throw error;
    }
};

// 封装 DELETE 请求
export const del = async (url, config = {}, onSuccess = null, onError = null) => {
    try {
        const response = shouldUseInvoke()
            ? await invokeWithRisk('DELETE', url, { params: config?.params || {}, auth: buildAuth() })
            : await httpClient.delete(url, config);
        if (onSuccess) onSuccess(response);
        return response;
    } catch (error) {
        if (onError) onError(error);
        throw error;
    }
};

// 封装 PATCH 请求
export const patch = async (url, data = {}, config = {}, onSuccess = null, onError = null) => {
    try {
        const response = shouldUseInvoke()
            ? await invokeWithRisk('PATCH', url, { params: config?.params || {}, data, auth: buildAuth() })
            : await httpClient.patch(url, data, config);
        if (onSuccess) onSuccess(response);
        return response;
    } catch (error) {
        if (onError) onError(error);
        throw error;
    }
};

// 封装上传图片请求
export const uploadImage = async (url, file, additionalData = {}, config = {}, onSuccess = null, onError = null) => {
    try {
        const formData = new FormData();
        formData.append('file', file);

        // 如果有其他数据（如关联的商品信息等），也可以添加到 formData
        for (const key in additionalData) {
            if (Object.prototype.hasOwnProperty.call(additionalData, key)) {
                formData.append(key, additionalData[key]);
            }
        }

        // 需要确保 Content-Type 被设置为 multipart/form-data
        const response = await httpClient.post(url, formData, {
            ...config,
            headers: {
                ...config.headers,
                'Content-Type': 'multipart/form-data'
            }
        });

        if (onSuccess) onSuccess(response);
        return response;
    } catch (error) {
        if (onError) onError(error);
        throw error;
    }
};

// 导出 httpClient 以便在需要的时候直接使用 axios 实例
export default httpClient;
