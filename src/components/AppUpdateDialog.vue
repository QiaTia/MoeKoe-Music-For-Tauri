<template>
    <div v-if="showUpdateDialog" class="modal-overlay" @click="closeUpdateDialog">
        <div class="modal-content update-dialog" @click.stop>
            <h2>发现新版本 V{{ latestVersion }}</h2>
            <MarkdownContent class="update-log-box" :content="updateLog" :repo-url="projectRepoUrl" />
            <div v-if="progressVisible" class="update-progress-inline">
                <div class="update-progress-bar">
                    <div class="update-progress-fill" :style="{ width: `${updateProgress}%` }"></div>
                </div>
                <span class="update-progress-percent">{{ updateProgress.toFixed(1) }}%</span>
            </div>
            <p v-if="updateStatusText" class="update-status">{{ updateStatusText }}</p>
            <p v-if="updateSpeedText || updateSizeText" class="update-meta">
                <span v-if="updateSpeedText">{{ updateSpeedText }}</span>
                <span v-if="updateSizeText">{{ updateSizeText }}</span>
            </p>
            <p v-if="isUpdateDownloaded && gatekeeperScript" class="update-hint">
                若首次打开提示「已损坏」或「无法验证开发者」，在终端执行：
                <code>{{ gatekeeperScript }}</code>
                <a class="link-button" @click="copyGatekeeperScript">复制命令</a>
            </p>
            <div class="update-dialog-actions">
                <button v-if="isStartingUpdate || isDownloadingUpdate" class="secondary-button" @click="cancelOnlineUpdate">
                    取消下载
                </button>
                <button v-else-if="isUpdateDownloaded" @click="runInstaller">运行安装包</button>
                <button v-else @click="startOnlineUpdate">在线更新</button>
                <button @click="openManualUpdatePage">手动更新</button>
                <button class="secondary-button" @click="closeUpdateDialog">关闭</button>
            </div>
        </div>
    </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import MarkdownContent from './MarkdownContent.vue';
import { openRegisterUrl } from '../utils/utils';

const props = defineProps({
    appVersion: {
        type: String,
        default: ''
    },
    platform: {
        type: String,
        default: ''
    }
});

const emit = defineEmits(['badge-change']);

const projectRepoUrl = 'https://github.com/MoeKoeMusic/MoeKoeMusic';
const repoUrl = `${projectRepoUrl}/releases`;
const showUpdateDialog = ref(false);
const showNewBadge = ref(false);
const downloadUrl = ref('');
const updateLog = ref('暂无更新日志');
const latestVersion = ref('');
const isStartingUpdate = ref(false);
const isDownloadingUpdate = ref(false);
const isUpdateDownloaded = ref(false);
const updateProgress = ref(0);
const updateStatusText = ref('');
const updateSpeedText = ref('');
const updateSizeText = ref('');
const updateStage = ref('');
const gatekeeperScript = ref('');

const progressVisible = computed(() => isStartingUpdate.value || isDownloadingUpdate.value || isUpdateDownloaded.value);

const setShowNewBadge = (value) => {
    showNewBadge.value = value;
    emit('badge-change', value);
};

const formatBytes = (bytes = 0) => {
    if (!bytes) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / (1024 ** index);
    return `${value.toFixed(value >= 100 || index === 0 ? 0 : 1)} ${units[index]}`;
};

const isVersionLower = (current, latest) => {
    const currentParts = current.split('.').map(Number);
    const latestParts = latest.split('.').map(Number);
    for (let i = 0; i < Math.max(currentParts.length, latestParts.length); i++) {
        if ((latestParts[i] || 0) > (currentParts[i] || 0)) {
            return true;
        } else if ((latestParts[i] || 0) < (currentParts[i] || 0)) {
            return false;
        }
    }
    return false;
};

const getPlatformDownloadUrl = (release) => {
    const assets = Array.isArray(release?.assets) ? release.assets : [];
    if (!assets.length) {
        return release?.html_url || repoUrl;
    }

    const platformMatchers = {
        win32: [/\.exe$/i, /\.msi$/i],
        darwin: [/\.dmg$/i, /\.zip$/i],
        linux: [/\.AppImage$/i, /\.deb$/i, /\.rpm$/i]
    };

    const matchers = platformMatchers[props.platform] || [];
    const asset = assets.find(item => matchers.some(matcher => matcher.test(item.name)));
    return asset?.browser_download_url || release?.html_url || repoUrl;
};

const applyReleaseInfo = (info) => {
    latestVersion.value = info.latestVersion || '';
    updateLog.value = String(info.notes || '').trim() || '暂无更新日志';
    downloadUrl.value = info.htmlUrl || repoUrl;
    setShowNewBadge(!!info.available);
};

const fetchLatestVersion = async () => {
    if (!props.appVersion) {
        return;
    }

    // 优先走 Rust：多源回退（直连 → 镜像）、按架构选包、与下载使用同一份清单。
    // 此前前端直连上游仓库 releases，与后端（本 fork 仓库）版本可能不一致。
    try {
        const info = await window.electronAPI?.checkUpdate?.();
        if (info?.success) {
            applyReleaseInfo(info);
            return;
        }
        if (info) console.warn('[更新] 检查更新失败:', info.message);
    } catch (error) {
        console.warn('[更新] 调用本地检查失败:', error);
    }

    // Web 环境（无 Tauri 宿主）兜底：直连 GitHub API，仅用于展示
    try {
        const response = await fetch('https://api.github.com/repos/qiatia/MoeKoe-Music-For-Tauri/releases/latest');
        const data = await response.json();
        downloadUrl.value = getPlatformDownloadUrl(data);
        latestVersion.value = String(data.tag_name || '').replace(/^v/, '');
        updateLog.value = data.body?.trim() || '暂无更新日志';
        setShowNewBadge(isVersionLower(props.appVersion, latestVersion.value));
    } catch (error) {
        console.error('获取最新版本号失败:', error);
    }
};

const closeUpdateDialog = () => {
    showUpdateDialog.value = false;
};

const openManualUpdatePage = () => {
    openRegisterUrl(downloadUrl.value || repoUrl);
};

const handleEntryClick = () => {
    if (!showNewBadge.value && !isStartingUpdate.value && !isDownloadingUpdate.value && !isUpdateDownloaded.value) {
        openRegisterUrl(repoUrl);
        return;
    }
    showUpdateDialog.value = true;
};

const handleUpdateProgress = (_event, progressObj) => {
    isStartingUpdate.value = false;
    isDownloadingUpdate.value = true;
    isUpdateDownloaded.value = false;
    updateProgress.value = Math.max(0, Math.min(100, Number(progressObj?.percent || 0)));
    updateStage.value = progressObj?.stage || 'downloading';
    updateStatusText.value = updateStage.value === 'verifying' ? '正在校验安装包...' : '正在下载更新包...';
    updateSpeedText.value = progressObj?.bytesPerSecond ? `${formatBytes(progressObj.bytesPerSecond)}/s` : '';
    if (progressObj?.transferred || progressObj?.total) {
        updateSizeText.value = `${formatBytes(progressObj.transferred || 0)} / ${formatBytes(progressObj.total || 0)}`;
    } else {
        updateSizeText.value = '';
    }
};

const handleUpdateDownloaded = async () => {
    isStartingUpdate.value = false;
    isDownloadingUpdate.value = false;
    isUpdateDownloaded.value = true;
    updateProgress.value = 100;
    updateSpeedText.value = '';
    updateStatusText.value = '更新包已下载完成，正在启动安装程序...';

    // macOS 未签名/未公证时，替换后首次打开可能被 Gatekeeper 拦截，预先取好放行命令
    if (props.platform === 'darwin') {
        try {
            gatekeeperScript.value = (await window.electronAPI?.updateGatekeeperScript?.()) || '';
        } catch (error) {
            console.warn('[更新] 获取 Gatekeeper 命令失败:', error);
        }
    }
};

const handleUpdateError = (_event, payload) => {
    isStartingUpdate.value = false;
    isDownloadingUpdate.value = false;
    if (payload?.cancelled) {
        isUpdateDownloaded.value = false;
        updateProgress.value = 0;
        updateStatusText.value = '已取消更新';
    } else if (!isUpdateDownloaded.value) {
        updateStatusText.value = payload?.message || '更新失败，请稍后重试';
    }
    updateSpeedText.value = '';
};

const cancelOnlineUpdate = async () => {
    updateStatusText.value = '正在取消下载...';
    try {
        await window.electronAPI?.cancelUpdate?.();
    } catch (error) {
        console.warn('[更新] 取消下载失败:', error);
    }
};

const runInstaller = async () => {
    try {
        const result = await window.electronAPI?.installUpdate?.();
        if (!result?.success) {
            await window.$modal.alert(result?.message || '无法启动安装程序，请手动打开下载目录中的安装包。');
        }
    } catch (error) {
        await window.$modal.alert(`无法启动安装程序：${error.message}`);
    }
};

const copyGatekeeperScript = async () => {
    try {
        await navigator.clipboard.writeText(gatekeeperScript.value);
        window.$message?.success('命令已复制');
    } catch (error) {
        window.$message?.error(`复制失败：${error.message}`);
    }
};

const startOnlineUpdate = async () => {
    if (isStartingUpdate.value || isDownloadingUpdate.value || isUpdateDownloaded.value) {
        return;
    }

    if (!window.electronAPI?.startUpdateDownload) {
        await window.$modal.alert('当前环境暂不支持在线更新，请前往官网或应用市场下载最新版本。');
        return;
    }

    isStartingUpdate.value = true;
    updateProgress.value = 0;
    updateStatusText.value = '正在检查更新...';
    updateSpeedText.value = '';
    updateSizeText.value = '';
    try {
        // 传入弹窗展示的版本号：若期间已发布更高版本，后端会拒绝并提示重新检查，
        // 避免「看到 1.7.2、装成 1.7.3」这类静默不一致
        const result = await window.electronAPI.startUpdateDownload(latestVersion.value || undefined);
        if (result?.success) {
            isDownloadingUpdate.value = true;
            updateStatusText.value = '正在下载更新包...';
            return;
        }

        isStartingUpdate.value = false;
        isDownloadingUpdate.value = false;

        if (result?.reason === 'unsupported') {
            updateStatusText.value = '当前环境暂不支持在线更新';
            await window.$modal.alert('当前环境暂不支持在线更新，请前往官网或应用市场下载最新版本。');
            return;
        }

        if (result?.reason === 'not-available') {
            setShowNewBadge(false);
            updateStatusText.value = '当前没有可用更新';
            await window.$modal.alert('当前没有可用更新，请前往仓库查看最新发布状态。');
            return;
        }

        if (result?.reason === 'version-changed') {
            latestVersion.value = result.latestVersion || latestVersion.value;
            updateStatusText.value = result.message || '更新版本已变化，请重新检查更新';
            await window.$modal.alert(updateStatusText.value);
            return;
        }

        updateStatusText.value = result?.message || '启动在线更新失败，请稍后重试。';
        await window.$modal.alert(result?.message || '启动在线更新失败，请稍后重试。');
    } finally {
        isStartingUpdate.value = false;
    }
};

watch(() => [props.appVersion, props.platform], ([appVersion]) => {
    if (appVersion) {
        fetchLatestVersion();
    }
}, { immediate: true });

onMounted(() => {
    if (window.electron) {
        window.electron.ipcRenderer.on('update-progress', handleUpdateProgress);
        window.electron.ipcRenderer.on('update-downloaded', handleUpdateDownloaded);
        window.electron.ipcRenderer.on('update-error', handleUpdateError);
    }
});

onUnmounted(() => {
    if (window.electron) {
        window.electron.ipcRenderer.removeListener('update-progress', handleUpdateProgress);
        window.electron.ipcRenderer.removeListener('update-downloaded', handleUpdateDownloaded);
        window.electron.ipcRenderer.removeListener('update-error', handleUpdateError);
    }
});

defineExpose({
    handleEntryClick
});
</script>

<style lang="scss" scoped>
.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal-content {
    position: relative;
    background: #fff;
    padding: 20px;
    border-radius: 8px;
    max-width: 700px;
    width: 90%;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
    text-align: left;
    animation: fadeIn 0.3s ease;

    h2 {
        margin-top: 20px;
        color: var(--primary-color);
    }

    button {
        margin-top: 15px;
        padding: 8px 12px;
        background-color: var(--primary-color);
        color: #fff;
        border: none;
        border-radius: 5px;
        cursor: pointer;
    }
}

.update-dialog {
    max-width: 600px;

    .update-log-box {
        max-height: 250px;
        overflow-y: auto;
        margin-top: 16px;
        padding: 14px 16px;
        border-radius: 10px;
        background: #faf7fb;
        border: 1px solid rgba(var(--primary-color-rgb), 0.18);
    }

    .update-dialog-actions {
        display: flex;
        justify-content: flex-end;
        gap: 12px;
        margin-top: 18px;
        flex-wrap: wrap;
    }

    .update-status {
        margin: 10px 0 0;
        font-size: 13px;
        color: #555;
    }

    .update-meta {
        display: flex;
        gap: 16px;
        margin: 6px 0 0;
        font-size: 12px;
        color: #888;
        font-variant-numeric: tabular-nums;
    }

    .update-hint {
        margin: 10px 0 0;
        padding: 10px 12px;
        border-radius: 8px;
        background: #fff7e6;
        border: 1px solid rgba(255, 152, 0, 0.3);
        font-size: 12px;
        line-height: 1.6;
        color: #7a5200;

        code {
            display: block;
            margin: 6px 0;
            padding: 6px 8px;
            border-radius: 6px;
            background: rgba(0, 0, 0, 0.06);
            font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
            word-break: break-all;
        }
    }

    .link-button {
        color: var(--primary-color);
        cursor: pointer;
        text-decoration: underline;
    }

    .update-progress-inline {
        display: flex;
        align-items: center;
        gap: 12px;
    }

    .update-progress-bar {
        flex: 1;
        height: 6px;
        border-radius: 999px;
        overflow: hidden;
        background: rgba(var(--primary-color-rgb), 0.12);
    }

    .update-progress-fill {
        height: 100%;
        border-radius: inherit;
        background: linear-gradient(90deg, var(--primary-color), #ff8ac2);
        transition: width 0.2s ease;
    }

    .update-progress-percent {
        min-width: 52px;
        text-align: right;
        font-size: 13px;
        color: #555;
        font-variant-numeric: tabular-nums;
    }

    button {
        min-width: 124px;
        border-radius: 999px;
        padding: 10px 18px;
        margin-top: 0;
    }

    .secondary-button {
        background: #eef1f6;
        color: #333;
    }
}

@keyframes fadeIn {
    from {
        opacity: 0;
        transform: scale(0.95);
    }

    to {
        opacity: 1;
        transform: scale(1);
    }
}
</style>
