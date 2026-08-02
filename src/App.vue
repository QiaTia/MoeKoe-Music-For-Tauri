<template>
    <div id="app">
        <TitleBar v-if="showTitleBar && !isLyricsRoute && !isMiniRoute" />
        <RouterView />
        <Disclaimer v-if="!isLyricsRoute && !isMiniRoute" />
        <StatusBarLyrics v-if="!isLyricsRoute && !isMiniRoute" ref="statusBarLyricsRef" />
    </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import Disclaimer from '@/components/Disclaimer.vue';
import TitleBar from '@/components/TitleBar.vue';
import StatusBarLyrics from '@/components/StatusBarLyrics.vue';
import { MoeAuthStore } from '@/stores/store';
import logoImageSrc from '@/assets/images/tray/tray-icon@2x.png?url';

const route = useRoute();
const isLyricsRoute = computed(() => route.path === '/lyrics');
const isMiniRoute = computed(() => route.path === '/mini');

// 状态栏歌词逻辑
const statusBarLyricsRef = ref(null);
let cleanupStatusBarIPC = null;

// 动态控制 TitleBar 的显示
const showTitleBar = ref(true);

onMounted(async () => {
    const settings = JSON.parse(localStorage.getItem('settings')) || {};
    showTitleBar.value = settings.nativeTitleBar !== 'on'; // 如果值为 'on'，则不显示 TitleBar

    const MoeAuth = MoeAuthStore();
    await MoeAuth.initDevice();

    // 初始化状态栏歌词（小屏模式下不渲染该组件，?. 安全跳过）
    cleanupStatusBarIPC = statusBarLyricsRef.value?.initStatusBar(logoImageSrc, settings);
});

onUnmounted(() => {
    statusBarLyricsRef.value?.cleanupStatusBar();
    cleanupStatusBarIPC?.();
});
</script>

<style scoped>
.container {
    max-width: 1400px;
    margin: 0 auto;
    padding: 20px;
}
</style>
