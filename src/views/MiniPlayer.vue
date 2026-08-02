<template>
  <div class="mini-player" :class="{ 'compact': isCompact }" :style="bgStyle">
    <!-- 顶部栏：拖拽区 + 形态切换 + 关闭 -->
    <div class="mini-top">
      <span class="mini-drag" @mousedown="startDrag"></span>
      <div class="mini-top-actions">
        <button class="mini-btn" :title="isCompact ? '展开' : '收起'" @click.stop="toggleMode">
          <i :class="isCompact ? 'fas fa-square' : 'fas fa-compress'"></i>
        </button>
        <button class="mini-btn" title="关闭" @click.stop="closeWindow">
          <i class="fas fa-times"></i>
        </button>
      </div>
    </div>

    <!-- 正常模式：大封面 + 信息 + 歌词 + 进度 + 控制 -->
    <div class="mini-body" v-if="!isCompact">
      <div class="mini-cover" :style="{ backgroundImage: 'url(' + coverImg + ')' }">
        <div class="mini-lyric-overlay" v-if="displayLyric">{{ displayLyric }}</div>
      </div>
      <div class="mini-info">
        <div class="mini-title" :title="songName">{{ songName }}</div>
        <div class="mini-artist" :title="songArtist">{{ songArtist }}</div>
      </div>
      <!-- <div class="mini-lyric-line" v-if="displayLyric">{{ displayLyric }}</div> -->
      <div class="mini-progress">
        <div class="mini-progress-bar" :style="{ width: progressPercent + '%' }"></div>
      </div>
      <div class="mini-controls">
        <button class="mini-btn" title="上一首" @click.stop="prev"><i class="fas fa-step-backward"></i></button>
        <button class="mini-btn play" title="播放/暂停" @click.stop="togglePlay">
          <i :class="playing ? 'fas fa-pause' : 'fas fa-play'"></i>
        </button>
        <button class="mini-btn" title="下一首" @click.stop="next"><i class="fas fa-step-forward"></i></button>
      </div>
    </div>

    <!-- 迷你模式：小封面 + 信息 + 基础控制 -->
    <div class="mini-compact-body" v-else>
      <img class="mini-cover-small" :src="coverImg" @click.stop="toggleMode" alt="cover" />
      <div class="mini-compact-info">
        <div class="mini-title mini-title-sm">{{ songName }}</div>
        <div class="mini-artist mini-artist-sm">{{ songArtist }}</div>
      </div>
      <div class="mini-controls compact">
        <button class="mini-btn" title="上一首" @click.stop="prev"><i class="fas fa-step-backward"></i></button>
        <button class="mini-btn play" title="播放/暂停" @click.stop="togglePlay">
          <i :class="playing ? 'fas fa-pause' : 'fas fa-play'"></i>
        </button>
        <button class="mini-btn" title="下一首" @click.stop="next"><i class="fas fa-step-forward"></i></button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { useRoute } from 'vue-router';
import { ensureHttps } from '@/utils/utils';

// ── 镜像状态（由主窗口通过 Rust 事件推送） ──
// 初始形态取自 mini 窗口 URL 携带的 ?mode=（即 Rust 端持久化的记忆），
// 避免事件到达前出现「窗口尺寸与页面形态错配」的闪烁；后续以 mini-state 事件为准保持同步。
const route = useRoute();
const songName = ref('MoeKoe Music');
const songArtist = ref('MoeJue');
const coverImg = ref('');
const playing = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const currentLyric = ref('');
const lyricsList = ref([]);
const isCompact = ref(route.query.mode === 'compact');

// 窗口位置（屏幕逻辑像素），用于拖拽与边缘吸附
const miniPos = ref({ x: 0, y: 0 });

// 根据播放时间从整份歌词中计算当前行（主窗未推送 currentLyric 时兜底）
const computeCurrentLyric = (list, time) => {
  if (!Array.isArray(list) || list.length === 0) return '';
  const ms = (time || 0) * 1000;
  for (const line of list) {
    const chars = line.characters;
    if (!Array.isArray(chars) || chars.length === 0) continue;
    const first = chars[0];
    const last = chars[chars.length - 1];
    if (ms >= first.startTime && ms <= last.endTime) {
      return chars.map((c) => c.char).join('');
    }
  }
  return '';
};
const displayLyric = computed(() => {
  if (currentLyric.value) return currentLyric.value;
  return computeCurrentLyric(lyricsList.value, currentTime.value);
});

// ── 取色背景 ──
const bgColor = ref('rgba(40,40,60,0.55)');
const bgStyle = computed(() => {
  if (isCompact.value) {
    return { background: 'rgba(28,28,38,0.92)' };
  }
  return {
    background: `linear-gradient(135deg, ${bgColor.value} 0%, rgba(20,20,30,0.94) 100%)`
  };
});

const computeColor = (url) => {
  if (!url) {
    bgColor.value = 'rgba(40,40,60,0.55)';
    return;
  }
  const img = new Image();
  img.crossOrigin = 'anonymous';
  img.onload = () => {
    try {
      const canvas = document.createElement('canvas');
      canvas.width = 1;
      canvas.height = 1;
      const ctx = canvas.getContext('2d');
      ctx.drawImage(img, 0, 0, 1, 1);
      const d = ctx.getImageData(0, 0, 1, 1).data;
      bgColor.value = `rgb(${d[0]},${d[1]},${d[2]})`;
    } catch (e) {
      // CORS 限制 canvas 读取时回退
      bgColor.value = 'rgba(40,40,60,0.55)';
    }
  };
  img.onerror = () => {
    bgColor.value = 'rgba(40,40,60,0.55)';
  };
  img.src = url;
};
watch(coverImg, (u) => computeColor(u));

// ── 进度 ──
const progressPercent = computed(() =>
  duration.value > 0 ? Math.min(100, (currentTime.value / duration.value) * 100) : 0
);

// ── 播放控制（复用 desktop-lyrics-action 通道，由主窗口执行） ──
const prev = () => sendAction('previous-song');
const next = () => sendAction('next-song');
const togglePlay = () => sendAction('toggle-play');
const sendAction = (action) => {
  if (window.electron) {
    window.electron.ipcRenderer.send('desktop-lyrics-action', action);
  }
};

// ── 形态切换 / 关闭 ──
const toggleMode = () => {
  // 不再抢跑翻状态：由 Rust 切换尺寸后通过 mini-state 回传 mode，
  // 避免前端与 Rust state 错配导致 normal 内容被压进 compact 高度。
  if (window.electron) {
    window.electron.ipcRenderer.send('mini-window-control', 'toggle');
  }
};
const closeWindow = () => {
  if (window.electron) {
    window.electron.ipcRenderer.send('mini-window-control', 'close');
  }
};

// ── 拖拽移动：交给 Tauri 系统原生拖动，彻底消除前端 IPC 往返抖动 ──
const startDrag = (e) => {
  if (e.target.closest('.mini-btn') || e.target.closest('.mini-progress')) return;
  if (window.electron) {
    window.electron.ipcRenderer.send('mini-window-drag');
  }
};

// ── 事件监听（主窗口状态同步） ──
let cleanup = [];
onMounted(() => {
  if (!window.electron) return;
  const ipc = window.electron.ipcRenderer;

  const onPlayerState = (_e, payload) => {
    if (!payload) return;
    currentTime.value = payload.currentTime || 0;
    duration.value = payload.duration || 0;
    playing.value = !!payload.playing;
    if (payload.currentLyric != null) {
      currentLyric.value = payload.currentLyric;
    }
  };
  const onPlayerMeta = (_e, payload) => {
    if (!payload) return;
    songName.value = payload.name || 'MoeKoe Music';
    songArtist.value = payload.author || 'MoeJue';
    coverImg.value = ensureHttps(payload.img || '');
  };
  const onLyricsData = (_e, payload) => {
    if (!payload) return;
    if (payload.currentLyric != null) {
      currentLyric.value = payload.currentLyric;
    }
    if (Array.isArray(payload.lyricsData)) {
      lyricsList.value = payload.lyricsData;
    }
  };
  const onMiniState = (_e, payload) => {
    if (!payload) return;
    isCompact.value = payload.mode === 'compact';
    if (typeof payload.x === 'number') miniPos.value.x = payload.x;
    if (typeof payload.y === 'number') miniPos.value.y = payload.y;
  };

  ipc.on('player-state', onPlayerState);
  ipc.on('player-meta', onPlayerMeta);
  ipc.on('lyrics-data', onLyricsData);
  ipc.on('mini-state', onMiniState);
  cleanup = [
    () => ipc.removeListener('player-state', onPlayerState),
    () => ipc.removeListener('player-meta', onPlayerMeta),
    () => ipc.removeListener('lyrics-data', onLyricsData),
    () => ipc.removeListener('mini-state', onMiniState),
  ];

  // 主动请求主窗补发一次完整状态 + 当前窗口状态（对齐形态/位置）
  ipc.send('request-player-state-sync');
  ipc.send('request-mini-state');
});

onUnmounted(() => {
  cleanup.forEach((fn) => fn());
});
</script>

<style scoped>
.mini-player {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  color: #fff;
  border-radius: 14px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  user-select: none;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Microsoft YaHei', sans-serif;
}

.mini-top {
  height: 28px;
  display: flex;
  align-items: center;
  padding: 0 6px;
  flex-shrink: 0;
}
.mini-drag {
  flex: 1;
  height: 100%;
  cursor: move;
}
.mini-top-actions {
  display: flex;
  gap: 4px;
}
.mini-btn {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.85);
  cursor: pointer;
  width: 26px;
  height: 26px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  transition: background 0.15s;
}
.mini-btn:hover {
  background: rgba(255, 255, 255, 0.15);
}
.mini-btn.play {
  color: #fff;
}

/* 正常模式 */
.mini-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 4px 16px 12px;
  min-height: 0;
  overflow: hidden;
}
.mini-cover {
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  aspect-ratio: 1 / 1;
  border-radius: 12px;
  background-size: cover;
  background-position: center;
  background-color: rgba(255, 255, 255, 0.08);
  position: relative;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}
.mini-lyric-overlay {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  padding: 18px 12px 10px;
  font-size: 13px;
  line-height: 1.4;
  text-align: center;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.7), rgba(0, 0, 0, 0));
  border-bottom-left-radius: 12px;
  border-bottom-right-radius: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mini-lyric-line {
  margin-top: 6px;
  font-size: 13px;
  line-height: 1.4;
  text-align: center;
  color: rgba(255, 255, 255, 0.92);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 0;
  min-height: 18px;
}
.mini-info {
  margin-top: 8px;
  text-align: center;
  min-height: 0;
  flex-shrink: 0;
}
.mini-title {
  font-size: 15px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mini-artist {
  font-size: 12px;
  opacity: 0.7;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mini-progress {
  margin-top: 8px;
  height: 4px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.2);
  cursor: default;
  flex-shrink: 0;
}
.mini-progress-bar {
  height: 100%;
  border-radius: 2px;
  background: #fff;
  transition: width 0.25s linear;
}
.mini-controls {
  margin-top: auto;
  padding-top: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 22px;
  flex-shrink: 0;
}
.mini-controls.compact {
  margin: 0;
  padding: 0;
  gap: 10px;
}

/* 迷你模式 */
.mini-compact-body {
  flex: 1;
  display: flex;
  align-items: flex-end;
  padding: 0 10px 10px;
  gap: 10px;
  min-height: 0;
  margin-top: -10px
}
.mini-cover-small {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  object-fit: cover;
  background: rgba(255, 255, 255, 0.08);
  cursor: pointer;
  flex-shrink: 0;
}
.mini-compact-info {
  flex: 1;
  min-width: 0;
}
.mini-title-sm {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mini-artist-sm {
  font-size: 11px;
  opacity: 0.7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mini-compact-body .mini-btn {
  width: 28px;
  height: 28px;
}
</style>
