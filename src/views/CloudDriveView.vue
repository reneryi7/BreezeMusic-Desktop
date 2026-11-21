<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { formatDuration, resolveSongQualityBadge as resolveCoreQualityBadge } from "../api/kugou";
import { useViewportStagger } from "../composables/useViewportStagger";
import type { Song } from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";
import FloatingLocateFab from "../components/FloatingLocateFab.vue";
import { useInfiniteScroll } from "../composables/useInfiniteScroll";

type QualityFilter = "all" | "hires" | "sq" | "hq" | "std";
type SortMode = "default" | "title" | "artist" | "size" | "duration";

const store = useAppStore();

const searchKeyword = ref("");
const qualityFilter = ref<QualityFilter>("all");
const sortMode = ref<SortMode>("default");
const isBatchMode = ref(false);
const selectedKeys = ref<Set<string>>(new Set());
const lastSelectedIdx = ref<number | null>(null);
const cloudSongsGridRef = ref<HTMLElement | null>(null);
const cloudStagger = useViewportStagger(cloudSongsGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 260,
});

const showPlaylistPickerModal = ref(false);
const singleAddSong = ref<Song | null>(null);
const pendingDeleteSongs = ref<Song[]>([]);
const isDeletingCloud = ref(false);
const brokenCoverKeys = ref<Set<string>>(new Set());

function markCoverBroken(key: string) {
  if (!key || brokenCoverKeys.value.has(key)) return;
  const next = new Set(brokenCoverKeys.value);
  next.add(key);
  brokenCoverKeys.value = next;
}

function songRowKey(song: Song, idx?: number): string {
  const fid = Number(song.fileId || 0);
  const hash = String(song.hash || song.id || "").trim();
  if (fid && hash) return `${fid}::${hash}`;
  if (hash) return `${hash}::${song.albumAudioId || 0}`;
  return `${song.title}::${song.artist}::${idx ?? 0}`;
}

function getSongQualityTier(song: Song): Exclude<QualityFilter, "all"> {
  const badge = resolveCoreQualityBadge(song);
  return badge ? badge.tier : "std";
}

function resolveSongQualityBadge(song: Song): { label: string; tier: string } | null {
  return resolveCoreQualityBadge(song) ?? { label: "标准", tier: "std" };
}

function getSongFileSize(song: Song): number {
  return Math.max(
    Number(song.sizeHiRes || 0),
    Number(song.sizeFlac || 0),
    Number(song.size320 || 0),
    Number(song.size128 || 0)
  );
}

function formatBytes(bytes?: number): string {
  const n = Number(bytes || 0);
  if (!Number.isFinite(n) || n <= 0) return "0 MB";
  const kb = 1024;
  const mb = kb * 1024;
  const gb = mb * 1024;
  if (n >= gb) return `${(n / gb).toFixed(2)} GB`;
  if (n >= mb) return `${(n / mb).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(n / kb))} KB`;
}

function getTitleInitial(song: Song): string {
  const ch = String(song.title || "云").trim().charAt(0);
  return ch ? ch.toUpperCase() : "云";
}

const totalCloudCount = computed(() =>
  Math.max(store.cloudDiskTotal || 0, store.cloudDiskSongs.length)
);

const qualityStats = computed(() => {
  let hires = 0;
  let sq = 0;
  let hq = 0;
  for (const s of store.cloudDiskSongs) {
    const t = getSongQualityTier(s);
    if (t === "hires") hires++;
    else if (t === "sq") sq++;
    else if (t === "hq") hq++;
  }
  return { hires, sq, hq };
});

const quotaRatio = computed(() => {
  const used = Number(store.cloudDiskUsedBytes || 0);
  const max = Number(store.cloudDiskMaxBytes || 0);
  if (used <= 0 || max <= 0) return 0;
  return Math.min(1, Math.max(0, used / max));
});

const quotaPctText = computed(() => {
  if (store.cloudDiskMaxBytes <= 0) return "--";
  const pct = quotaRatio.value * 100;
  if (pct > 0 && pct < 0.1) return "<0.1%";
  return `${pct.toFixed(pct >= 10 ? 1 : 2)}%`;
});

const quotaRingRadius = 26;
const quotaRingCircumference = 2 * Math.PI * quotaRingRadius;
const quotaRingDashoffset = computed(
  () => quotaRingCircumference * (1 - quotaRatio.value)
);

const filteredSongs = computed<Song[]>(() => {
  const kw = searchKeyword.value.trim().toLowerCase();
  let list = store.cloudDiskSongs.filter((song) => {
    if (qualityFilter.value !== "all" && getSongQualityTier(song) !== qualityFilter.value) {
      return false;
    }
    if (!kw) return true;
    return (
      String(song.title || "").toLowerCase().includes(kw) ||
      String(song.artist || "").toLowerCase().includes(kw) ||
      String(song.album || "").toLowerCase().includes(kw) ||
      String(song.highestQuality || "").toLowerCase().includes(kw)
    );
  });

  if (sortMode.value !== "default") {
    list = [...list];
    if (sortMode.value === "title") {
      list.sort((a, b) => (a.title || "").localeCompare(b.title || "", "zh-CN"));
    } else if (sortMode.value === "artist") {
      list.sort((a, b) => (a.artist || "").localeCompare(b.artist || "", "zh-CN"));
    } else if (sortMode.value === "size") {
      list.sort((a, b) => getSongFileSize(b) - getSongFileSize(a));
    } else if (sortMode.value === "duration") {
      list.sort((a, b) => Number(b.durationMs || 0) - Number(a.durationMs || 0));
    }
  }

  return list;
});

const selectedSongs = computed<Song[]>(() =>
  filteredSongs.value.filter((song, idx) =>
    selectedKeys.value.has(songRowKey(song, idx))
  )
);

const allFilteredSelected = computed(
  () =>
    filteredSongs.value.length > 0 &&
    selectedSongs.value.length === filteredSongs.value.length
);

function playAllFiltered() {
  if (filteredSongs.value.length === 0) return;
  store.playSongFromList(filteredSongs.value[0], filteredSongs.value, "我的云盘");
}

function toggleBatchMode() {
  isBatchMode.value = !isBatchMode.value;
  selectedKeys.value = new Set();
  lastSelectedIdx.value = null;
}

function toggleSelectAll() {
  if (allFilteredSelected.value) {
    selectedKeys.value = new Set();
    lastSelectedIdx.value = null;
    return;
  }
  const next = new Set<string>();
  filteredSongs.value.forEach((song, idx) => {
    next.add(songRowKey(song, idx));
  });
  selectedKeys.value = next;
}

function invertSelection() {
  const next = new Set<string>();
  filteredSongs.value.forEach((song, idx) => {
    const key = songRowKey(song, idx);
    if (!selectedKeys.value.has(key)) next.add(key);
  });
  selectedKeys.value = next;
}

function handleRowClick(ev: MouseEvent, song: Song, idx: number) {
  if (!isBatchMode.value) {
    store.playSongFromList(song, filteredSongs.value, "我的云盘");
    return;
  }
  const key = songRowKey(song, idx);
  const next = new Set(selectedKeys.value);
  if (ev.shiftKey && lastSelectedIdx.value !== null) {
    const start = Math.min(lastSelectedIdx.value, idx);
    const end = Math.max(lastSelectedIdx.value, idx);
    for (let i = start; i <= end; i++) {
      const target = filteredSongs.value[i];
      if (target) next.add(songRowKey(target, i));
    }
  } else {
    if (next.has(key)) next.delete(key);
    else next.add(key);
  }
  lastSelectedIdx.value = idx;
  selectedKeys.value = next;
}

function handleRowContextMenu(ev: MouseEvent, song: Song) {
  if (isBatchMode.value) return;
  store.openSongContextMenu(ev, song, filteredSongs.value);
}

function handleBatchPlaySelected() {
  if (selectedSongs.value.length === 0) return;
  store.playSongFromList(selectedSongs.value[0], selectedSongs.value, "我的云盘 · 已选");
}

function handleBatchPlayNext() {
  if (selectedSongs.value.length === 0) return;
  for (let i = selectedSongs.value.length - 1; i >= 0; i--) {
    store.insertPlayNext(selectedSongs.value[i]);
  }
  store.notify(`已将 ${selectedSongs.value.length} 首云盘曲目插队至下一首播放`);
}

function handleBatchAppendQueue() {
  if (selectedSongs.value.length === 0) return;
  store.batchAppendSongsToQueue(selectedSongs.value);
}

function handleBatchDownload() {
  if (selectedSongs.value.length === 0) return;
  store.batchDownloadSongs(selectedSongs.value);
}

function openBatchAddToPlaylist() {
  if (selectedSongs.value.length === 0) return;
  singleAddSong.value = null;
  showPlaylistPickerModal.value = true;
}

function openSingleAddToPlaylist(song: Song) {
  singleAddSong.value = song;
  showPlaylistPickerModal.value = true;
}

async function handlePickPlaylist(listId?: number) {
  if (!listId) return;
  const target = singleAddSong.value ? [singleAddSong.value] : selectedSongs.value;
  if (target.length === 0) return;
  await store.batchAddSongsToPlaylist(listId, target);
  showPlaylistPickerModal.value = false;
  singleAddSong.value = null;
}

function requestDeleteCloudSongs(songs: Song[]) {
  if (songs.length === 0) return;
  pendingDeleteSongs.value = [...songs];
}

function cancelDeleteCloudSongs() {
  if (isDeletingCloud.value) return;
  pendingDeleteSongs.value = [];
}

async function confirmDeleteCloudSongs() {
  if (pendingDeleteSongs.value.length === 0 || isDeletingCloud.value) return;
  isDeletingCloud.value = true;
  try {
    const ok = await store.deleteCloudDiskSongs(pendingDeleteSongs.value);
    if (ok) {
      pendingDeleteSongs.value = [];
      if (isBatchMode.value) {
        selectedKeys.value = new Set();
        lastSelectedIdx.value = null;
      }
    }
  } finally {
    isDeletingCloud.value = false;
  }
}

const cloudSentinelRef = ref<HTMLElement | null>(null);
const { hasError: cloudLoadMoreError, retryLoad: retryLoadMoreCloud } =
  useInfiniteScroll({
    sentinelRef: cloudSentinelRef,
    enabled: computed(() => store.isLoggedIn && store.cloudDiskSongs.length > 0),
    loading: computed(() => store.isLoadingCloudDisk),
    hasMore: computed(() => store.cloudDiskHasMore),
    onLoadMore: () => store.loadMoreCloudDisk(),
  });

function handleEscapeModal(e: Event) {
  if (showPlaylistPickerModal.value) {
    showPlaylistPickerModal.value = false;
    e.preventDefault();
  } else if (pendingDeleteSongs.value.length > 0 && !isDeletingCloud.value) {
    pendingDeleteSongs.value = [];
    e.preventDefault();
  }
}

watch(
  () => store.isLoggedIn,
  (loggedIn) => {
    if (loggedIn && store.cloudDiskSongs.length === 0 && !store.isLoadingCloudDisk) {
      store.loadUserCloudDisk(false);
    }
  }
);

onMounted(() => {
  window.addEventListener("breeze:escape-modal", handleEscapeModal);
  if (store.isLoggedIn && store.cloudDiskSongs.length === 0 && !store.isLoadingCloudDisk) {
    store.loadUserCloudDisk(false);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("breeze:escape-modal", handleEscapeModal);
});
</script>

<template>
  <div class="cdv-workspace" @contextmenu.prevent>
    <!-- Bento 头部双卡工作区 -->
    <section class="cdv-bento-hero">
      <div class="cdv-hero-card">
        <div class="cdv-eyebrow">
          <PlumpIcon name="cloud" :size="14" color="var(--primary)" />
          <span>概念版私人云盘</span>
        </div>

        <div class="cdv-hero-title-row">
          <div class="cdv-hero-copy">
            <h1 class="cdv-title">我的音乐云盘</h1>
            <p class="cdv-subtitle">
              跨设备私人音频仓库与转存母带直链，支持分页拉取、无损甄选与云端曲目管理
            </p>
          </div>
        </div>

        <div class="cdv-hero-actions">
          <button
            type="button"
            class="cdv-action-btn primary"
            :disabled="filteredSongs.length === 0"
            @click="playAllFiltered"
          >
            <PlumpIcon name="play" :size="15" color="#fff" />
            <span>播放全部 ({{ filteredSongs.length }})</span>
          </button>

          <button
            type="button"
            class="cdv-action-btn"
            :class="{ active: isBatchMode }"
            :disabled="filteredSongs.length === 0"
            @click="toggleBatchMode"
          >
            <PlumpIcon name="batch-check" :size="15" />
            <span>{{ isBatchMode ? "退出批量" : "批量管理" }}</span>
          </button>

          <button
            type="button"
            class="cdv-action-btn"
            :disabled="!store.isLoggedIn || store.isLoadingCloudDisk"
            @click="store.loadUserCloudDisk(true)"
          >
            <PlumpIcon
              name="refresh"
              :size="15"
              :class="{ 'is-spinning': store.isLoadingCloudDisk }"
            />
            <span>{{ store.isLoadingCloudDisk ? "同步中..." : "刷新云端" }}</span>
          </button>
        </div>
      </div>

      <!-- 右侧 Bento 配额与资产遥测卡 -->
      <aside class="cdv-quota-card">
        <div class="cdv-quota-top">
          <div class="cdv-quota-ring-wrap" :aria-label="`云盘容量使用 ${quotaPctText}`">
            <svg class="cdv-quota-svg" viewBox="0 0 64 64">
              <circle class="cdv-ring-track" cx="32" cy="32" :r="quotaRingRadius" />
              <circle
                class="cdv-ring-progress"
                cx="32"
                cy="32"
                :r="quotaRingRadius"
                :stroke-dasharray="quotaRingCircumference"
                :stroke-dashoffset="quotaRingDashoffset"
              />
            </svg>
            <span class="cdv-ring-pct">{{ quotaPctText }}</span>
          </div>

          <div class="cdv-quota-summary">
            <div class="cdv-quota-label">云盘已用空间</div>
            <div class="cdv-quota-value">
              <template v-if="store.isLoggedIn">
                {{ formatBytes(store.cloudDiskUsedBytes) }}
                <span class="cdv-quota-max">
                  / {{ store.cloudDiskMaxBytes > 0 ? formatBytes(store.cloudDiskMaxBytes) : "动态扩容" }}
                </span>
              </template>
              <template v-else>未连接账号</template>
            </div>
            <div class="cdv-quota-meter">
              <div
                class="cdv-quota-meter-fill"
                :style="{ width: `${Math.round(quotaRatio * 100)}%` }"
              ></div>
            </div>
          </div>
        </div>

        <div class="cdv-telemetry-grid">
          <div class="cdv-stat-cell">
            <span class="cdv-stat-num">{{ totalCloudCount }}</span>
            <span class="cdv-stat-cap">云端总曲目</span>
          </div>
          <div class="cdv-stat-cell">
            <span class="cdv-stat-num">{{ qualityStats.hires }}</span>
            <span class="cdv-stat-cap">Hi-Res 母带</span>
          </div>
          <div class="cdv-stat-cell">
            <span class="cdv-stat-num">{{ qualityStats.sq }}</span>
            <span class="cdv-stat-cap">SQ 无损</span>
          </div>
          <div class="cdv-stat-cell">
            <span class="cdv-stat-num">{{ qualityStats.hq }}</span>
            <span class="cdv-stat-cap">HQ 高品质</span>
          </div>
        </div>
      </aside>
    </section>

    <!-- 未登录引导 -->
    <section v-if="!store.isLoggedIn" class="cdv-empty-card">
      <div class="cdv-empty-badge">
        <PlumpIcon name="cloud" :size="32" color="var(--primary)" />
      </div>
      <h2 class="cdv-empty-title">登录后解锁你的概念版音乐云盘</h2>
      <p class="cdv-empty-desc">
        同步酷狗概念版账号云端私人曲库，查看已用空间配额、无损音质文件并支持原盘删除与收藏同步。
      </p>
      <button
        type="button"
        class="cdv-action-btn primary"
        @click="store.isLoginModalOpen = true"
      >
        <span>立即登录酷狗概念版</span>
      </button>
    </section>

    <template v-else>
      <!-- 工具筛选与排序条 -->
      <section class="cdv-toolbar">
        <div class="cdv-search-box">
          <PlumpIcon name="search" :size="15" />
          <input
            v-model="searchKeyword"
            type="text"
            class="cdv-search-input"
            placeholder="检索云盘歌名、歌手、专辑或格式 (如 FLAC)..."
          />
          <button
            v-if="searchKeyword"
            type="button"
            class="cdv-clear-btn"
            title="清空检索词"
            @click="searchKeyword = ''"
          >
            <PlumpIcon name="close" :size="12" />
          </button>
        </div>

        <div class="cdv-pill-groups">
          <div class="cdv-pill-cluster" role="group" aria-label="音质筛选">
            <button
              v-for="item in [
                { id: 'all', label: '全部音质' },
                { id: 'hires', label: 'Hi-Res' },
                { id: 'sq', label: 'SQ 无损' },
                { id: 'hq', label: 'HQ' },
                { id: 'std', label: '标准' },
              ]"
              :key="item.id"
              type="button"
              class="cdv-filter-pill"
              :class="{ active: qualityFilter === item.id }"
              @click="qualityFilter = item.id as QualityFilter"
            >
              {{ item.label }}
            </button>
          </div>

          <div class="cdv-pill-cluster" role="group" aria-label="排序方式">
            <button
              v-for="item in [
                { id: 'default', label: '默认' },
                { id: 'title', label: '歌名' },
                { id: 'artist', label: '歌手' },
                { id: 'size', label: '体积' },
                { id: 'duration', label: '时长' },
              ]"
              :key="item.id"
              type="button"
              class="cdv-filter-pill"
              :class="{ active: sortMode === item.id }"
              @click="sortMode = item.id as SortMode"
            >
              {{ item.label }}
            </button>
          </div>
        </div>
      </section>

      <!-- 无边框内联永久删除确认条（零 window.confirm） -->
      <transition name="fade">
        <div v-if="pendingDeleteSongs.length > 0" class="cdv-delete-confirm-banner" role="alert">
          <div class="cdv-confirm-left">
            <span class="cdv-confirm-badge">
              <PlumpIcon name="trash" :size="15" />
            </span>
            <div class="cdv-confirm-copy">
              <div class="cdv-confirm-title">
                <template v-if="pendingDeleteSongs.length === 1">
                  确认从云盘永久删除《{{ pendingDeleteSongs[0].title }}》？
                </template>
                <template v-else>
                  确认从云盘永久删除已选的 {{ pendingDeleteSongs.length }} 首歌曲？
                </template>
              </div>
              <div class="cdv-confirm-sub">
                该操作将同步清除酷狗概念版云端源文件并释放配额空间，删除后不可恢复。
              </div>
            </div>
          </div>
          <div class="cdv-confirm-actions">
            <button
              type="button"
              class="cdv-confirm-cancel"
              :disabled="isDeletingCloud"
              @click="cancelDeleteCloudSongs"
            >
              取消
            </button>
            <button
              type="button"
              class="cdv-confirm-danger"
              :disabled="isDeletingCloud"
              @click="confirmDeleteCloudSongs"
            >
              {{ isDeletingCloud ? "正在从云盘删除..." : "确认永久删除" }}
            </button>
          </div>
        </div>
      </transition>

      <!-- 批量多选悬浮操作 bar（复用全局 zero-border batch bar 规范） -->
      <transition name="fade">
        <div v-if="isBatchMode" class="batch-floating-action-bar">
          <div class="bfab-left">
            <button type="button" class="bfab-pill" @click="toggleSelectAll">
              {{ allFilteredSelected ? "取消全选" : "全选" }}
            </button>
            <button
              type="button"
              class="bfab-pill"
              :disabled="filteredSongs.length === 0"
              @click="invertSelection"
            >
              反选
            </button>
            <span class="bfab-count">
              已选 {{ selectedSongs.length }} 首
              <span class="bfab-hint">（按住 Shift 点击可连续框选）</span>
            </span>
          </div>

          <div class="bfab-actions">
            <button
              type="button"
              class="bfab-btn primary"
              :disabled="selectedSongs.length === 0"
              @click="handleBatchPlaySelected"
            >
              <PlumpIcon name="play" :size="14" color="#fff" />
              <span>播放</span>
            </button>
            <button
              type="button"
              class="bfab-btn"
              :disabled="selectedSongs.length === 0"
              @click="handleBatchPlayNext"
            >
              <PlumpIcon name="plus" :size="14" />
              <span>下一首播放</span>
            </button>
            <button
              type="button"
              class="bfab-btn"
              :disabled="selectedSongs.length === 0"
              @click="handleBatchAppendQueue"
            >
              <PlumpIcon name="queue" :size="14" />
              <span>加入队列</span>
            </button>
            <button
              type="button"
              class="bfab-btn"
              :disabled="selectedSongs.length === 0"
              @click="openBatchAddToPlaylist"
            >
              <PlumpIcon name="playlist-add" :size="14" />
              <span>加到歌单</span>
            </button>
            <button
              v-if="store.downloadFeatureUnlocked"
              type="button"
              class="bfab-btn"
              :disabled="selectedSongs.length === 0"
              @click="handleBatchDownload"
            >
              <PlumpIcon name="download" :size="14" />
              <span>下载</span>
            </button>
            <button
              type="button"
              class="bfab-btn danger"
              :disabled="selectedSongs.length === 0"
              @click="requestDeleteCloudSongs(selectedSongs)"
            >
              <PlumpIcon name="trash" :size="14" />
              <span>删除云盘</span>
            </button>
            <button type="button" class="bfab-btn ghost" @click="toggleBatchMode">
              <span>取消</span>
            </button>
          </div>
        </div>
      </transition>

      <!-- 云盘曲库主体 -->
      <section v-if="store.isLoadingCloudDisk && store.cloudDiskSongs.length === 0" class="cdv-empty-card">
        <PlumpIcon name="refresh" :size="28" class="is-spinning" color="var(--primary)" />
        <h2 class="cdv-empty-title">正在拉取酷狗概念版云盘曲库...</h2>
        <p class="cdv-empty-desc">正在校验云盘资产配额并解析音频 Hash。</p>
      </section>

      <section v-else-if="filteredSongs.length === 0" class="cdv-empty-card">
        <div class="cdv-empty-badge">
          <PlumpIcon name="cloud" :size="28" color="var(--primary)" />
        </div>
        <h2 class="cdv-empty-title">
          {{ store.cloudDiskSongs.length === 0 ? "你的音乐云盘暂时空空如也" : "未筛选到匹配的云盘歌曲" }}
        </h2>
        <p class="cdv-empty-desc">
          {{
            store.cloudDiskSongs.length === 0
              ? "通过概念版移动端上传的私人音乐会自动汇聚于此，点击上方「刷新云端」可重新同步。"
              : "尝试切换音质筛选标签或清空上方搜索关键词。"
          }}
        </p>
        <button
          v-if="searchKeyword || qualityFilter !== 'all'"
          type="button"
          class="cdv-action-btn"
          @click="
            searchKeyword = '';
            qualityFilter = 'all';
          "
        >
          <span>重置筛选条件</span>
        </button>
      </section>

      <section v-else class="cdv-list-stage">
        <div ref="cloudSongsGridRef" class="frameless-songs-grid-2col">
          <div
            v-for="(song, idx) in filteredSongs"
            :key="songRowKey(song, idx)"
            :data-locate-song-key="song.hash || song.id"
            class="unified-song-row"
            :class="[
              {
                active: store.currentSong?.hash === song.hash,
                selected: isBatchMode && selectedKeys.has(songRowKey(song, idx)),
                'is-batch-mode': isBatchMode,
              },
              cloudStagger.getStaggerClass(idx),
            ]"
            :style="cloudStagger.getStaggerStyle(idx)"
            @click="handleRowClick($event, song, idx)"
            @contextmenu.prevent="handleRowContextMenu($event, song)"
          >
            <div class="usr-index-slot">
              <span
                v-if="isBatchMode"
                class="usr-batch-checkbox"
                :class="{ checked: selectedKeys.has(songRowKey(song, idx)) }"
              >
                <PlumpIcon
                  v-if="selectedKeys.has(songRowKey(song, idx))"
                  name="check"
                  :size="12"
                  color="#fff"
                />
              </span>
              <span
                v-else-if="store.currentSong?.hash === song.hash"
                class="mini-eq-bars"
                :class="{ paused: !store.isPlaying }"
              >
                <i></i><i></i><i></i>
              </span>
              <span v-else>{{ String(idx + 1).padStart(2, "0") }}</span>
            </div>

            <div class="usr-cover-wrap">
              <img
                v-if="song.coverUrl && !brokenCoverKeys.has(songRowKey(song, idx))"
                :src="song.coverUrl"
                loading="lazy"
                decoding="async"
                alt=""
                @error="markCoverBroken(songRowKey(song, idx))"
              />
              <div v-else class="usr-cover-fallback cdv-monogram">
                <span>{{ getTitleInitial(song) }}</span>
              </div>
            </div>

            <div class="usr-meta-stack">
              <div class="usr-line-1">
                <span class="usr-title">{{ song.title }}</span>
                <span v-if="getSongFileSize(song) > 0" class="cdv-inline-tech">
                  {{ formatBytes(getSongFileSize(song)) }}
                </span>
              </div>

              <div class="usr-line-2">
                <span v-if="resolveSongQualityBadge(song)" class="usr-badges">
                  <span
                    class="usr-badge-quality"
                    :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                  >
                    {{ resolveSongQualityBadge(song)!.label }}
                  </span>
                </span>

                <span class="usr-subtext">
                  <button
                    type="button"
                    class="usr-artist usr-meta-link"
                    :title="isBatchMode ? '点击选中歌曲' : '点击进入歌手主页'"
                    @click="
                      isBatchMode
                        ? null
                        : ($event.stopPropagation(), store.navigateToSongArtist(song))
                    "
                  >
                    {{ song.artist }}
                  </button>
                  <template v-if="song.album">
                    <span class="usr-sep"> · </span>
                    <button
                      type="button"
                      class="usr-album usr-meta-link"
                      :title="isBatchMode ? '点击选中歌曲' : '点击进入专辑详情页'"
                      @click="
                        isBatchMode
                          ? null
                          : ($event.stopPropagation(), store.navigateToSongAlbum(song))
                      "
                    >
                      {{ song.album }}
                    </button>
                  </template>
                </span>
              </div>
            </div>

            <div class="usr-right-zone">
              <div v-if="!isBatchMode" class="usr-hover-actions" @click.stop>
                <button
                  type="button"
                  class="row-icon-btn"
                  :class="{ liked: store.isSongLiked(song) }"
                  title="喜欢"
                  @click="store.toggleFavoriteSong(song)"
                >
                  <PlumpIcon
                    :name="store.isSongLiked(song) ? 'heart-filled' : 'heart'"
                    :size="15"
                  />
                </button>
                <button
                  type="button"
                  class="row-icon-btn"
                  title="下一首播放"
                  @click="store.insertPlayNext(song)"
                >
                  <PlumpIcon name="plus" :size="15" />
                </button>
                <button
                  type="button"
                  class="row-icon-btn"
                  title="收藏到歌单"
                  @click="openSingleAddToPlaylist(song)"
                >
                  <PlumpIcon name="playlist-add" :size="15" />
                </button>
                <button
                  type="button"
                  class="row-icon-btn danger"
                  title="从云盘删除"
                  @click="requestDeleteCloudSongs([song])"
                >
                  <PlumpIcon name="trash" :size="14" />
                </button>
              </div>
              <span class="usr-duration">{{ formatDuration(song) }}</span>
            </div>
          </div>
        </div>

        <!-- 无感静默分页哨兵与状态底栏 -->
        <div ref="cloudSentinelRef" class="cdv-pagination-bar">
          <span class="cdv-pagination-info">
            已载入 {{ store.cloudDiskSongs.length }} 首 / 云端共 {{ totalCloudCount }} 首
          </span>
          <span v-if="store.cloudDiskHasMore && store.isLoadingCloudDisk" class="cdv-pagination-info">
            <PlumpIcon name="refresh" :size="13" class="is-spinning" />
            正在静默加载下一页...
          </span>
          <button
            v-else-if="store.cloudDiskHasMore && cloudLoadMoreError"
            type="button"
            class="cdv-load-more-btn"
            @click="retryLoadMoreCloud()"
          >
            <PlumpIcon name="refresh" :size="14" />
            <span>加载中断，点击继续</span>
          </button>
        </div>
      </section>
    </template>

    <!-- 添加到自建歌单弹窗 -->
    <transition name="fade">
      <div
        v-if="showPlaylistPickerModal"
        class="modal-backdrop"
        @click.self="showPlaylistPickerModal = false"
      >
        <div class="compact-glass-modal cdv-picker-modal">
          <div class="modal-header">
            <h3>
              {{
                singleAddSong
                  ? `收藏《${singleAddSong.title}》到歌单`
                  : `批量收藏 ${selectedSongs.length} 首云盘曲目至歌单`
              }}
            </h3>
            <button
              type="button"
              class="icon-close-btn"
              @click="showPlaylistPickerModal = false"
            >
              <PlumpIcon name="close" :size="15" />
            </button>
          </div>

          <div v-if="store.createdPlaylists.length === 0" class="cdv-picker-empty">
            暂无自建云歌单，请先在「我的音乐」创建一个歌单。
          </div>

          <div v-else class="playlist-pick-list">
            <button
              v-for="pl in store.createdPlaylists"
              :key="pl.id"
              type="button"
              class="playlist-pick-row"
              @click="handlePickPlaylist(pl.listId)"
            >
              <div class="ppr-left">
                <PlumpIcon :name="pl.isLiked ? 'heart' : 'library'" :size="18" />
                <div>
                  <div class="ppr-title">{{ pl.title }}</div>
                  <div class="ppr-count">{{ pl.trackCount }} 首曲目</div>
                </div>
              </div>
              <PlumpIcon name="plus" :size="15" />
            </button>
          </div>
        </div>
      </div>
    </transition>

    <FloatingLocateFab
      :songs="filteredSongs"
      :has-batch-bar="isBatchMode && filteredSongs.length > 0"
    />
  </div>
</template>

<style scoped>
.cdv-workspace {
  padding: 12px 24px 108px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-width: 0;
}

.cdv-bento-hero {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(300px, 0.9fr);
  gap: 16px;
  align-items: stretch;
}

@media (max-width: 960px) {
  .cdv-bento-hero {
    grid-template-columns: 1fr;
  }
}

.cdv-hero-card,
.cdv-quota-card {
  border-radius: var(--bm-r-xl, 24px);
  padding: 22px 24px;
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 10%, rgba(255, 255, 255, 0.04)),
    rgba(255, 255, 255, 0.025)
  );
  box-shadow: 0 18px 40px -24px rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(22px) saturate(135%);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
}

.cdv-eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  align-self: flex-start;
  padding: 5px 12px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  color: var(--primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  letter-spacing: 0.02em;
}

.cdv-hero-title-row {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.cdv-hero-copy {
  min-width: 0;
}

.cdv-title {
  margin: 0 0 6px;
  font-size: clamp(24px, 2.3vw, 30px);
  font-weight: var(--bm-fw-heavy, 800);
  line-height: 1.18;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.cdv-subtitle {
  margin: 0;
  font-size: var(--bm-fs-body, 14.5px);
  line-height: 1.5;
  color: var(--text-secondary);
}

.cdv-hero-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.cdv-action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 40px;
  padding: 0 18px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: transform 0.16s ease, background 0.16s ease, box-shadow 0.16s ease, opacity 0.16s ease;
}

.cdv-action-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.13);
  transform: translateY(-1px);
}

.cdv-action-btn.primary {
  background: var(--primary);
  color: #fff;
  box-shadow: 0 10px 24px -10px color-mix(in srgb, var(--primary) 65%, transparent);
}

.cdv-action-btn.primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 88%, #ffffff);
}

.cdv-action-btn.active {
  background: color-mix(in srgb, var(--primary) 22%, transparent);
  color: var(--primary);
}

.cdv-action-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.cdv-action-btn:focus-visible,
.cdv-filter-pill:focus-visible,
.cdv-load-more-btn:focus-visible,
.cdv-confirm-cancel:focus-visible,
.cdv-confirm-danger:focus-visible,
.cdv-clear-btn:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.cdv-quota-top {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.cdv-quota-ring-wrap {
  position: relative;
  width: 68px;
  height: 68px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.cdv-quota-svg {
  width: 68px;
  height: 68px;
  transform: rotate(-90deg);
}

.cdv-ring-track {
  fill: none;
  stroke: rgba(255, 255, 255, 0.09);
  stroke-width: 5.5;
}

.cdv-ring-progress {
  fill: none;
  stroke: var(--primary);
  stroke-width: 5.5;
  stroke-linecap: round;
  transition: stroke-dashoffset 0.4s ease;
}

.cdv-ring-pct {
  position: absolute;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.cdv-quota-summary {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.cdv-quota-label {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  font-weight: var(--bm-fw-semibold, 600);
}

.cdv-quota-value {
  font-size: var(--bm-fs-title, 20px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cdv-quota-max {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-secondary);
}

.cdv-quota-meter {
  height: 6px;
  width: 100%;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}

.cdv-quota-meter-fill {
  height: 100%;
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(90deg, var(--primary), var(--cyan, var(--primary)));
  transition: width 0.35s ease;
}

.cdv-telemetry-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.cdv-stat-cell {
  border-radius: var(--bm-r-md, 14px);
  padding: 10px 10px;
  background: rgba(255, 255, 255, 0.045);
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.cdv-stat-num {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.cdv-stat-cap {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cdv-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.cdv-search-box {
  flex: 1 1 280px;
  max-width: 420px;
  height: 40px;
  padding: 0 14px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.06);
  display: flex;
  align-items: center;
  gap: 9px;
  color: var(--text-secondary);
  transition: background 0.16s ease;
}

.cdv-search-box:focus-within {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-primary);
}

.cdv-search-input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  outline: none;
}

.cdv-search-input::placeholder {
  color: var(--text-tertiary, var(--text-secondary));
}

.cdv-clear-btn {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: none;
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.cdv-pill-groups {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.cdv-pill-cluster {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.045);
}

.cdv-filter-pill {
  height: 30px;
  padding: 0 13px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.cdv-filter-pill:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.06);
}

.cdv-filter-pill.active {
  background: var(--primary);
  color: #fff;
}

.cdv-delete-confirm-banner {
  border-radius: var(--bm-r-lg, 18px);
  padding: 14px 18px;
  background: color-mix(in srgb, var(--coral, #e11d48) 14%, rgba(255, 255, 255, 0.05));
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.cdv-confirm-left {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1 1 320px;
}

.cdv-confirm-badge {
  width: 36px;
  height: 36px;
  border-radius: var(--bm-r-sm, 10px);
  background: color-mix(in srgb, var(--coral, #e11d48) 22%, transparent);
  color: var(--coral, #fb7185);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.cdv-confirm-copy {
  min-width: 0;
}

.cdv-confirm-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.cdv-confirm-sub {
  margin-top: 2px;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.cdv-confirm-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.cdv-confirm-cancel,
.cdv-confirm-danger {
  height: 36px;
  padding: 0 16px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
}

.cdv-confirm-cancel {
  background: rgba(255, 255, 255, 0.09);
  color: var(--text-primary);
}

.cdv-confirm-danger {
  background: var(--coral, #e11d48);
  color: #fff;
}

.cdv-empty-card {
  border-radius: var(--bm-r-xl, 24px);
  padding: 56px 28px;
  background: rgba(255, 255, 255, 0.035);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 12px;
}

.cdv-empty-badge {
  width: 68px;
  height: 68px;
  border-radius: var(--bm-r-xl, 24px);
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.cdv-empty-title {
  margin: 0;
  font-size: var(--bm-fs-title, 20px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.cdv-empty-desc {
  margin: 0 0 8px;
  max-width: 460px;
  font-size: var(--bm-fs-body, 14.5px);
  line-height: 1.55;
  color: var(--text-secondary);
}

.cdv-list-stage {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.cdv-monogram {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--primary) 42%, transparent),
    color-mix(in srgb, var(--cyan, var(--primary)) 28%, transparent)
  );
  color: var(--text-primary);
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
}

.cdv-inline-tech {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-semibold, 600);
  font-variant-numeric: tabular-nums;
}

.cdv-pagination-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 18px;
  border-radius: var(--bm-r-lg, 18px);
  background: rgba(255, 255, 255, 0.035);
}

.cdv-pagination-info {
  font-size: var(--bm-fs-meta, 13.5px);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.cdv-load-more-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 18px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: color-mix(in srgb, var(--primary) 18%, transparent);
  color: var(--primary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
}

.cdv-load-more-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 28%, transparent);
}

.cdv-load-more-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.cdv-picker-modal {
  width: min(430px, 92vw);
}

.cdv-picker-empty {
  padding: 24px 8px;
  text-align: center;
  font-size: var(--bm-fs-body, 14.5px);
  color: var(--text-secondary);
}

.cdv-spin {
  animation: cdvSpin 1s linear infinite;
}

@keyframes cdvSpin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* D2 暗紫星云微调 */
.scheme-abyssal_dark.dark-variant-velvet_nebula .cdv-hero-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .cdv-quota-card,
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .cdv-hero-card,
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .cdv-quota-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 13%, rgba(34, 26, 42, 0.72)),
    rgba(24, 19, 30, 0.68)
  );
}

/* L1 晨雾水彩浅色适配 */
.scheme-watercolor_light .cdv-hero-card,
.scheme-watercolor_light .cdv-quota-card,
[data-theme="watercolor_light"] .cdv-hero-card,
[data-theme="watercolor_light"] .cdv-quota-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 7%, rgba(255, 255, 255, 0.86)),
    rgba(255, 255, 255, 0.72)
  );
  box-shadow: 0 18px 36px -24px rgba(45, 38, 28, 0.16);
}

.scheme-watercolor_light .cdv-stat-cell,
.scheme-watercolor_light .cdv-search-box,
.scheme-watercolor_light .cdv-pill-cluster,
.scheme-watercolor_light .cdv-empty-card,
.scheme-watercolor_light .cdv-pagination-bar,
.scheme-watercolor_light .cdv-inline-tech,
[data-theme="watercolor_light"] .cdv-stat-cell,
[data-theme="watercolor_light"] .cdv-search-box,
[data-theme="watercolor_light"] .cdv-pill-cluster,
[data-theme="watercolor_light"] .cdv-empty-card,
[data-theme="watercolor_light"] .cdv-pagination-bar,
[data-theme="watercolor_light"] .cdv-inline-tech {
  background: rgba(255, 255, 255, 0.68);
}

.scheme-watercolor_light .cdv-action-btn:not(.primary):not(.active),
.scheme-watercolor_light .cdv-confirm-cancel,
[data-theme="watercolor_light"] .cdv-action-btn:not(.primary):not(.active),
[data-theme="watercolor_light"] .cdv-confirm-cancel {
  background: rgba(34, 30, 26, 0.07);
}

.scheme-watercolor_light .cdv-ring-track,
[data-theme="watercolor_light"] .cdv-ring-track {
  stroke: rgba(34, 30, 26, 0.1);
}

.scheme-watercolor_light .cdv-quota-meter,
[data-theme="watercolor_light"] .cdv-quota-meter {
  background: rgba(34, 30, 26, 0.08);
}

/* L2 北欧燕麦浅色暖纸适配 */
.scheme-watercolor_light.light-variant-nordic_oat .cdv-hero-card,
.scheme-watercolor_light.light-variant-nordic_oat .cdv-quota-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-hero-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-quota-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 6%, #FCF9F2),
    #F5F0E6
  );
  box-shadow: 0 16px 34px -22px rgba(64, 52, 34, 0.14);
}

.scheme-watercolor_light.light-variant-nordic_oat .cdv-stat-cell,
.scheme-watercolor_light.light-variant-nordic_oat .cdv-search-box,
.scheme-watercolor_light.light-variant-nordic_oat .cdv-pill-cluster,
.scheme-watercolor_light.light-variant-nordic_oat .cdv-empty-card,
.scheme-watercolor_light.light-variant-nordic_oat .cdv-pagination-bar,
.scheme-watercolor_light.light-variant-nordic_oat .cdv-inline-tech,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-stat-cell,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-search-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-pill-cluster,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-empty-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-pagination-bar,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .cdv-inline-tech {
  background: #F5F0E6;
}
</style>
