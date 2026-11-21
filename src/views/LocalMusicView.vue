<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { formatDuration } from "../api/kugou";
import { useViewportStagger } from "../composables/useViewportStagger";
import type { Song } from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";
import FloatingLocateFab from "../components/FloatingLocateFab.vue";

type ViewTab = "all" | "artist" | "album" | "folder";
type FormatFilter = "all" | "hires" | "lossless" | "compressed";
type SortMode = "default" | "title" | "artist" | "size" | "duration";

interface GroupBucket {
  key: string;
  label: string;
  subLabel: string;
  songs: Song[];
  totalBytes: number;
  losslessCount: number;
}

const store = useAppStore();

const viewTab = ref<ViewTab>("all");
const selectedGroupKey = ref<string>("");
const searchKeyword = ref("");
const formatFilter = ref<FormatFilter>("all");
const sortMode = ref<SortMode>("default");
const isBatchMode = ref(false);
const selectedKeys = ref<Set<string>>(new Set());
const localSongsGridRef = ref<HTMLElement | null>(null);
const localStagger = useViewportStagger(localSongsGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 260,
});
const lastSelectedIdx = ref<number | null>(null);

const savedDirs = computed(() => store.localMusicSavedDirs);

const currentScanDir = computed(() =>
  (store.localMusicDir || store.downloadMusicDir || "./downloads").trim()
);

const managedDirs = computed<string[]>(() => {
  const list = [...savedDirs.value];
  const cur = currentScanDir.value;
  if (cur) {
    const curNorm = cur.replace(/[\\/]+$/, "").toLowerCase();
    if (!list.some((d) => d.replace(/[\\/]+$/, "").toLowerCase() === curNorm)) {
      list.unshift(cur);
    }
  }
  if (store.downloadMusicDir) {
    const dl = store.downloadMusicDir.trim();
    const dlNorm = dl.replace(/[\\/]+$/, "").toLowerCase();
    if (dl && !list.some((d) => d.replace(/[\\/]+$/, "").toLowerCase() === dlNorm)) {
      list.push(dl);
    }
  }
  return list;
});

function isDirActive(dir: string): boolean {
  const a = dir.replace(/[\\/]+$/, "").toLowerCase();
  const b = currentScanDir.value.replace(/[\\/]+$/, "").toLowerCase();
  return a === b;
}

function getDirShortName(dir: string): string {
  const clean = dir.replace(/[\\/]+$/, "");
  const sepIdx = Math.max(clean.lastIndexOf("\\"), clean.lastIndexOf("/"));
  return sepIdx >= 0 ? clean.slice(sepIdx + 1) || clean : clean;
}

function removeSavedDir(dir: string) {
  store.removeLocalMusicSavedDir(dir);
}

async function switchAndScanDir(dir: string) {
  if (!dir || store.isScanningLocalMusic) return;
  store.addLocalMusicSavedDir(dir);
  await store.scanLocalMusicLibrary(dir);
}

async function handlePickAndAddDir() {
  await store.pickAndScanLocalMusicDir();
}

function extractRawLocalPath(song: Song): string {
  const mix = String(song.mixSongId || "");
  if (mix.toLowerCase().startsWith("local://")) {
    return mix.slice(8);
  }
  return mix;
}

function getSongFileSize(song: Song): number {
  return Math.max(
    Number(song.sizeHiRes || 0),
    Number(song.sizeFlac || 0),
    Number(song.size320 || 0),
    Number(song.size128 || 0)
  );
}

function extractFileExt(song: Song): string {
  const raw = extractRawLocalPath(song);
  const dot = raw.lastIndexOf(".");
  if (dot >= 0 && dot < raw.length - 1) {
    return raw.slice(dot + 1).toLowerCase();
  }
  return "audio";
}

function extractFolderPath(song: Song): string {
  const raw = extractRawLocalPath(song);
  const sep = Math.max(raw.lastIndexOf("\\"), raw.lastIndexOf("/"));
  if (sep > 0) return raw.slice(0, sep);
  return currentScanDir.value || "本地目录";
}

function extractCleanAlbumLabel(song: Song): string {
  const raw = String(song.album || "").trim();
  if (!raw) return "未知专辑";
  return raw.replace(/\s*\([A-Z0-9]+\)\s*$/i, "").trim() || raw;
}

function isLosslessExt(ext: string, song: Song): boolean {
  if (song.isHiRes || song.isLossless || song.hasHiRes || song.hasFlac) return true;
  return (
    ext === "flac" ||
    ext === "wav" ||
    ext === "alac" ||
    ext === "ape" ||
    ext === "aiff" ||
    ext === "aif" ||
    ext === "wv" ||
    ext === "dsf"
  );
}

function getFormatCategory(song: Song): Exclude<FormatFilter, "all"> {
  if (song.isHiRes || song.hasHiRes) return "hires";
  const ext = extractFileExt(song);
  if (isLosslessExt(ext, song)) return "lossless";
  return "compressed";
}

function resolveLocalQualityBadge(song: Song): { label: string; tier: string } {
  const ext = extractFileExt(song).toLowerCase();
  if (song.isHiRes || song.hasHiRes) return { label: "Hi-Res", tier: "hires" };
  if (isLosslessExt(ext, song)) return { label: "SQ 无损", tier: "sq" };
  const hqTag = String(song.highestQuality || "").toUpperCase();
  if (song.has320 || Boolean(song.hash320) || hqTag === "LOC-HQ") {
    return { label: "HQ 320K", tier: "hq" };
  }
  return { label: "标准", tier: "std" };
}

const brokenLocalCoverKeys = ref<Set<string>>(new Set());
function markLocalCoverBroken(key: string) {
  if (!key || brokenLocalCoverKeys.value.has(key)) return;
  const next = new Set(brokenLocalCoverKeys.value);
  next.add(key);
  brokenLocalCoverKeys.value = next;
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

function getMonogramChar(song: Song): string {
  const ch = String(song.title || "音").trim().charAt(0);
  return ch ? ch.toUpperCase() : "音";
}

function songRowKey(song: Song, idx?: number): string {
  const key = String(song.hash || song.id || song.mixSongId || "").trim();
  if (key) return key;
  return `${song.title}::${song.artist}::${idx ?? 0}`;
}

const libraryTelemetry = computed(() => {
  let totalBytes = 0;
  let losslessCount = 0;
  const folderSet = new Set<string>();
  for (const s of store.localMusicSongs) {
    totalBytes += getSongFileSize(s);
    const ext = extractFileExt(s);
    if (isLosslessExt(ext, s)) losslessCount++;
    folderSet.add(extractFolderPath(s).toLowerCase());
  }
  return {
    totalCount: store.localMusicSongs.length,
    losslessCount,
    totalBytes,
    folderCount: folderSet.size,
  };
});

function buildGroups(mode: Exclude<ViewTab, "all">): GroupBucket[] {
  const map = new Map<string, GroupBucket>();
  for (const song of store.localMusicSongs) {
    let key = "";
    let label = "";
    let subLabel = "";
    if (mode === "artist") {
      label = String(song.artist || "未知歌手").trim() || "未知歌手";
      key = label.toLowerCase();
      subLabel = "本地歌手";
    } else if (mode === "album") {
      label = extractCleanAlbumLabel(song);
      key = label.toLowerCase();
      subLabel = String(song.artist || "本地合辑");
    } else {
      const folderPath = extractFolderPath(song);
      key = folderPath.toLowerCase();
      label = getDirShortName(folderPath);
      subLabel = folderPath;
    }

    let bucket = map.get(key);
    if (!bucket) {
      bucket = {
        key,
        label,
        subLabel,
        songs: [],
        totalBytes: 0,
        losslessCount: 0,
      };
      map.set(key, bucket);
    }
    bucket.songs.push(song);
    bucket.totalBytes += getSongFileSize(song);
    if (isLosslessExt(extractFileExt(song), song)) {
      bucket.losslessCount++;
    }
  }
  return Array.from(map.values()).sort((a, b) => b.songs.length - a.songs.length);
}

const artistGroups = computed<GroupBucket[]>(() => buildGroups("artist"));
const albumGroups = computed<GroupBucket[]>(() => buildGroups("album"));
const folderGroups = computed<GroupBucket[]>(() => buildGroups("folder"));

const activeGroups = computed<GroupBucket[]>(() => {
  if (viewTab.value === "artist") return artistGroups.value;
  if (viewTab.value === "album") return albumGroups.value;
  if (viewTab.value === "folder") return folderGroups.value;
  return [];
});

const selectedGroupObj = computed<GroupBucket | null>(() => {
  if (viewTab.value === "all" || !selectedGroupKey.value) return null;
  return activeGroups.value.find((g) => g.key === selectedGroupKey.value) || null;
});

watch(viewTab, () => {
  selectedGroupKey.value = "";
});

const filteredSongs = computed<Song[]>(() => {
  let base = store.localMusicSongs;
  if (selectedGroupObj.value) {
    base = selectedGroupObj.value.songs;
  }

  const kw = searchKeyword.value.trim().toLowerCase();
  let list = base.filter((song) => {
    if (formatFilter.value !== "all" && getFormatCategory(song) !== formatFilter.value) {
      return false;
    }
    if (!kw) return true;
    return (
      String(song.title || "").toLowerCase().includes(kw) ||
      String(song.artist || "").toLowerCase().includes(kw) ||
      String(song.album || "").toLowerCase().includes(kw) ||
      extractRawLocalPath(song).toLowerCase().includes(kw) ||
      extractFileExt(song).includes(kw)
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
  const label = selectedGroupObj.value
    ? `本地曲库 · ${selectedGroupObj.value.label}`
    : "本地无损曲库";
  store.playSongFromList(filteredSongs.value[0], filteredSongs.value, label);
}

function playGroupSongs(group: GroupBucket) {
  if (group.songs.length === 0) return;
  store.playSongFromList(group.songs[0], group.songs, `本地 · ${group.label}`);
}

function focusSongFolder(song: Song) {
  const folderKey = extractFolderPath(song).toLowerCase();
  viewTab.value = "folder";
  selectedGroupKey.value = folderKey;
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
    store.playSongFromList(song, filteredSongs.value, "本地无损曲库");
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

function handleBatchPlaySelected() {
  if (selectedSongs.value.length === 0) return;
  store.playSongFromList(selectedSongs.value[0], selectedSongs.value, "本地曲库 · 已选");
}

function handleBatchPlayNext() {
  if (selectedSongs.value.length === 0) return;
  for (let i = selectedSongs.value.length - 1; i >= 0; i--) {
    store.insertPlayNext(selectedSongs.value[i]);
  }
  store.notify(`已将 ${selectedSongs.value.length} 首本地曲目插队至下一首播放`);
}

function handleBatchAppendQueue() {
  if (selectedSongs.value.length === 0) return;
  store.batchAppendSongsToQueue(selectedSongs.value);
}

watch(
  () => store.localMusicDir,
  (dir) => {
    if (dir && dir.trim()) {
      store.addLocalMusicSavedDir(dir);
    }
  }
);

onMounted(() => {
  if (store.localMusicDir) {
    store.addLocalMusicSavedDir(store.localMusicDir);
  }
  if (store.localMusicSongs.length === 0 && !store.isScanningLocalMusic) {
    store.scanLocalMusicLibrary();
  }
});
</script>

<template>
  <div class="lmv-workspace" @contextmenu.prevent>
    <!-- Bento 双卡工作台头部 -->
    <section class="lmv-bento-hero">
      <div class="lmv-hero-card">
        <div class="lmv-eyebrow">
          <PlumpIcon name="folder" :size="14" color="var(--primary)" />
          <span>AudioStreamProxy 本地零拷贝直通</span>
        </div>

        <div class="lmv-hero-copy">
          <h1 class="lmv-title">本地无损曲库</h1>
          <p class="lmv-subtitle">
            支持 FLAC、WAV、MP3、M4A、OGG 与 AAC 毫秒级解析索引，直接读取本机磁盘母带流式解码
          </p>
        </div>

        <div class="lmv-hero-actions">
          <button
            type="button"
            class="lmv-action-btn primary"
            :disabled="filteredSongs.length === 0"
            @click="playAllFiltered"
          >
            <PlumpIcon name="play" :size="15" color="#fff" />
            <span>播放全部 ({{ filteredSongs.length }})</span>
          </button>

          <button
            type="button"
            class="lmv-action-btn"
            :disabled="store.isScanningLocalMusic"
            @click="handlePickAndAddDir"
          >
            <PlumpIcon name="folder" :size="15" />
            <span>添加本地目录</span>
          </button>

          <button
            type="button"
            class="lmv-action-btn"
            :disabled="store.isScanningLocalMusic"
            @click="store.scanLocalMusicLibrary()"
          >
            <PlumpIcon
              name="refresh"
              :size="15"
              :class="{ 'is-spinning': store.isScanningLocalMusic }"
            />
            <span>{{ store.isScanningLocalMusic ? "扫描中..." : "重新扫描" }}</span>
          </button>

          <button
            type="button"
            class="lmv-action-btn"
            :class="{ active: isBatchMode }"
            :disabled="filteredSongs.length === 0"
            @click="toggleBatchMode"
          >
            <PlumpIcon name="batch-check" :size="15" />
            <span>{{ isBatchMode ? "退出批量" : "批量管理" }}</span>
          </button>
        </div>
      </div>

      <!-- 右侧 Bento 本地目录管理 + 遥测卡 -->
      <aside class="lmv-stats-card">
        <div class="lmv-telemetry-grid">
          <div class="lmv-stat-cell">
            <span class="lmv-stat-num">{{ libraryTelemetry.totalCount }}</span>
            <span class="lmv-stat-cap">已索引歌曲</span>
          </div>
          <div class="lmv-stat-cell">
            <span class="lmv-stat-num">{{ libraryTelemetry.losslessCount }}</span>
            <span class="lmv-stat-cap">Hi-Res / 无损</span>
          </div>
          <div class="lmv-stat-cell">
            <span class="lmv-stat-num">{{ formatBytes(libraryTelemetry.totalBytes) }}</span>
            <span class="lmv-stat-cap">音频总体积</span>
          </div>
          <div class="lmv-stat-cell">
            <span class="lmv-stat-num">{{ libraryTelemetry.folderCount }}</span>
            <span class="lmv-stat-cap">涉及目录</span>
          </div>
        </div>

        <div class="lmv-dir-panel">
          <div class="lmv-dir-head">
            <span class="lmv-dir-label">已管理目录（点击切换并立即扫描）</span>
            <span class="lmv-dir-current" :title="currentScanDir">
              当前：{{ currentScanDir }}
            </span>
          </div>

          <div class="lmv-dir-chips">
            <div
              v-for="dir in managedDirs"
              :key="dir"
              class="lmv-dir-chip"
              :class="{ active: isDirActive(dir) }"
            >
              <button
                type="button"
                class="lmv-dir-chip-main"
                :title="dir"
                :disabled="store.isScanningLocalMusic"
                @click="switchAndScanDir(dir)"
              >
                <PlumpIcon name="folder" :size="13" />
                <span class="lmv-dir-chip-name">{{ getDirShortName(dir) }}</span>
              </button>
              <button
                v-if="savedDirs.some((s) => s.toLowerCase() === dir.toLowerCase()) && !isDirActive(dir)"
                type="button"
                class="lmv-dir-chip-remove"
                title="从常用目录移除（不删除磁盘文件）"
                @click.stop="removeSavedDir(dir)"
              >
                <PlumpIcon name="close" :size="11" />
              </button>
            </div>
          </div>
        </div>
      </aside>
    </section>

    <!-- 视图模式标签 + 搜索 + 音质筛选 + 排序工具栏 -->
    <section class="lmv-toolbar-stack">
      <div class="lmv-view-tabs-row">
        <div class="lmv-segmented-tabs" role="tablist" aria-label="本地曲库分组视图">
          <button
            v-for="tab in [
              { id: 'all', label: '全部歌曲', count: store.localMusicSongs.length },
              { id: 'artist', label: '按歌手', count: artistGroups.length },
              { id: 'album', label: '按专辑', count: albumGroups.length },
              { id: 'folder', label: '按文件夹', count: folderGroups.length },
            ]"
            :key="tab.id"
            type="button"
            role="tab"
            :aria-selected="viewTab === tab.id"
            class="lmv-tab-pill"
            :class="{ active: viewTab === tab.id }"
            @click="viewTab = tab.id as ViewTab"
          >
            <span>{{ tab.label }}</span>
            <span class="lmv-tab-count">{{ tab.count }}</span>
          </button>
        </div>

        <div v-if="selectedGroupObj" class="lmv-group-breadcrumb">
          <span class="lmv-crumb-label">
            当前分组：<strong>{{ selectedGroupObj.label }}</strong>（{{ selectedGroupObj.songs.length }} 首）
          </span>
          <button
            type="button"
            class="lmv-crumb-clear"
            @click="selectedGroupKey = ''"
          >
            查看全部分组
          </button>
        </div>
      </div>

      <div class="lmv-toolbar">
        <div class="lmv-search-box">
          <PlumpIcon name="search" :size="15" />
          <input
            v-model="searchKeyword"
            type="text"
            class="lmv-search-input"
            placeholder="检索本地文件名、歌手、专辑、目录路径或格式 (FLAC / WAV)..."
          />
          <button
            v-if="searchKeyword"
            type="button"
            class="lmv-clear-btn"
            title="清空检索词"
            @click="searchKeyword = ''"
          >
            <PlumpIcon name="close" :size="12" />
          </button>
        </div>

        <div class="lmv-pill-groups">
          <div class="lmv-pill-cluster" role="group" aria-label="格式筛选">
            <button
              v-for="item in [
                { id: 'all', label: '全部格式' },
                { id: 'hires', label: 'Hi-Res' },
                { id: 'lossless', label: 'FLAC / WAV' },
                { id: 'compressed', label: 'MP3 / AAC' },
              ]"
              :key="item.id"
              type="button"
              class="lmv-filter-pill"
              :class="{ active: formatFilter === item.id }"
              @click="formatFilter = item.id as FormatFilter"
            >
              {{ item.label }}
            </button>
          </div>

          <div class="lmv-pill-cluster" role="group" aria-label="排序方式">
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
              class="lmv-filter-pill"
              :class="{ active: sortMode === item.id }"
              @click="sortMode = item.id as SortMode"
            >
              {{ item.label }}
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- 分组聚合卡片墙（按歌手 / 按专辑 / 按文件夹） -->
    <section
      v-if="viewTab !== 'all' && activeGroups.length > 0 && !selectedGroupObj"
      class="lmv-group-grid"
    >
      <div
        v-for="grp in activeGroups"
        :key="grp.key"
        class="lmv-group-card"
        @click="selectedGroupKey = grp.key"
      >
        <div class="lmv-group-avatar" :class="{ 'is-folder': viewTab === 'folder' }">
          <PlumpIcon v-if="viewTab === 'folder'" name="folder" :size="20" color="var(--primary)" />
          <span v-else>{{ grp.label.trim().charAt(0).toUpperCase() || "音" }}</span>
        </div>

        <div class="lmv-group-info">
          <div class="lmv-group-title" :title="grp.label">{{ grp.label }}</div>
          <div class="lmv-group-sub" :title="grp.subLabel">
            {{ grp.songs.length }} 首 · {{ formatBytes(grp.totalBytes) }}
            <template v-if="grp.losslessCount > 0">
              · {{ grp.losslessCount }} 无损
            </template>
          </div>
        </div>

        <button
          type="button"
          class="lmv-group-play"
          title="播放此分组"
          @click.stop="playGroupSongs(grp)"
        >
          <PlumpIcon name="play" :size="14" color="#fff" />
        </button>
      </div>
    </section>

    <!-- 批量多选浮动条 -->
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
            <span class="bfab-hint">（按住 Shift 点击可连选）</span>
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
          <button type="button" class="bfab-btn ghost" @click="toggleBatchMode">
            <span>取消</span>
          </button>
        </div>
      </div>
    </transition>

    <!-- 空状态与扫描态 -->
    <section
      v-if="store.isScanningLocalMusic && store.localMusicSongs.length === 0"
      class="lmv-empty-card"
    >
      <PlumpIcon name="refresh" :size="28" class="is-spinning" color="var(--primary)" />
      <h2 class="lmv-empty-title">正在索引本地音频目录...</h2>
      <p class="lmv-empty-desc">正在读取 FLAC / WAV / MP3 采样头部并注册至零拷贝流式通道。</p>
    </section>

    <section v-else-if="filteredSongs.length === 0" class="lmv-empty-card">
      <div class="lmv-empty-badge">
        <PlumpIcon name="folder" :size="30" color="var(--primary)" />
      </div>
      <h2 class="lmv-empty-title">
        {{
          store.localMusicSongs.length === 0
            ? "当前目录尚未扫描到本地音乐"
            : "未筛选到符合条件的本地曲目"
        }}
      </h2>
      <p class="lmv-empty-desc">
        {{
          store.localMusicSongs.length === 0
            ? `已检查目录「${currentScanDir}」，支持扫描磁盘中保存的无损 FLAC、WAV 与高质量 MP3/AAC，点击上方「添加本地目录」即可导入。`
            : "尝试切换格式筛选条件、返回全部分组或清空搜索关键词。"
        }}
      </p>
      <div
        v-if="searchKeyword || formatFilter !== 'all' || selectedGroupKey"
        class="lmv-empty-actions"
      >
        <button
          type="button"
          class="lmv-action-btn"
          @click="
            searchKeyword = '';
            formatFilter = 'all';
            selectedGroupKey = '';
          "
        >
          <span>重置筛选与分组</span>
        </button>
      </div>
    </section>

    <!-- 统一双列歌曲列表 -->
    <section v-else class="lmv-list-stage">
      <div ref="localSongsGridRef" class="frameless-songs-grid-2col">
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
            localStagger.getStaggerClass(idx),
          ]"
          :style="localStagger.getStaggerStyle(idx)"
          @click="handleRowClick($event, song, idx)"
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
              v-if="song.coverUrl && !brokenLocalCoverKeys.has(songRowKey(song, idx))"
              :src="song.coverUrl"
              loading="lazy"
              decoding="async"
              alt=""
              @error="markLocalCoverBroken(songRowKey(song, idx))"
            />
            <div
              v-else
              class="usr-cover-fallback lmv-monogram"
              :class="{ 'is-lossless': isLosslessExt(extractFileExt(song), song) }"
            >
              <span>{{ getMonogramChar(song) }}</span>
            </div>
          </div>

          <div class="usr-meta-stack">
            <div class="usr-line-1">
              <span class="usr-title">{{ song.title }}</span>
              <span class="lmv-inline-tech">
                {{ extractFileExt(song).toUpperCase() }}
                <template v-if="getSongFileSize(song) > 0">
                  · {{ formatBytes(getSongFileSize(song)) }}
                </template>
              </span>
            </div>

            <div class="usr-line-2">
              <span class="usr-badges">
                <span
                  class="usr-badge-quality"
                  :class="`tier-${resolveLocalQualityBadge(song).tier}`"
                >
                  {{ resolveLocalQualityBadge(song).label }}
                </span>
              </span>

              <span class="usr-subtext" :title="extractRawLocalPath(song)">
                <span class="usr-artist">{{ song.artist || "未知歌手" }}</span>
                <span class="usr-sep"> · </span>
                <span class="usr-album">{{ extractCleanAlbumLabel(song) }}</span>
              </span>
            </div>
          </div>

          <div class="usr-right-zone">
            <div v-if="!isBatchMode" class="usr-hover-actions" @click.stop>
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
                title="加入播放队列"
                @click="store.appendSongToQueue(song)"
              >
                <PlumpIcon name="queue" :size="15" />
              </button>
              <button
                type="button"
                class="row-icon-btn"
                title="定位同文件夹曲目"
                @click="focusSongFolder(song)"
              >
                <PlumpIcon name="folder" :size="14" />
              </button>
            </div>
            <span class="usr-duration">{{ formatDuration(song) }}</span>
          </div>
        </div>
      </div>
    </section>

    <FloatingLocateFab
      :songs="filteredSongs"
      :has-batch-bar="isBatchMode && filteredSongs.length > 0"
    />
  </div>
</template>

<style scoped>
.lmv-workspace {
  padding: 12px 24px 108px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-width: 0;
}

.lmv-bento-hero {
  display: grid;
  grid-template-columns: minmax(0, 1.3fr) minmax(320px, 0.95fr);
  gap: 16px;
  align-items: stretch;
}

@media (max-width: 980px) {
  .lmv-bento-hero {
    grid-template-columns: 1fr;
  }
}

.lmv-hero-card,
.lmv-stats-card {
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

.lmv-eyebrow {
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
}

.lmv-hero-copy {
  min-width: 0;
}

.lmv-title {
  margin: 0 0 6px;
  font-size: clamp(24px, 2.3vw, 30px);
  font-weight: var(--bm-fw-heavy, 800);
  line-height: 1.18;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.lmv-subtitle {
  margin: 0;
  font-size: var(--bm-fs-body, 14.5px);
  line-height: 1.5;
  color: var(--text-secondary);
}

.lmv-hero-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.lmv-action-btn {
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

.lmv-action-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.13);
  transform: translateY(-1px);
}

.lmv-action-btn.primary {
  background: var(--primary);
  color: #fff;
  box-shadow: 0 10px 24px -10px color-mix(in srgb, var(--primary) 65%, transparent);
}

.lmv-action-btn.primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 88%, #ffffff);
}

.lmv-action-btn.active {
  background: color-mix(in srgb, var(--primary) 22%, transparent);
  color: var(--primary);
}

.lmv-action-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.lmv-action-btn:focus-visible,
.lmv-tab-pill:focus-visible,
.lmv-filter-pill:focus-visible,
.lmv-dir-chip-main:focus-visible,
.lmv-dir-chip-remove:focus-visible,
.lmv-group-play:focus-visible,
.lmv-crumb-clear:focus-visible,
.lmv-clear-btn:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.lmv-telemetry-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.lmv-stat-cell {
  border-radius: var(--bm-r-md, 14px);
  padding: 10px 10px;
  background: rgba(255, 255, 255, 0.045);
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.lmv-stat-num {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lmv-stat-cap {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lmv-dir-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}

.lmv-dir-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
}

.lmv-dir-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.lmv-dir-current {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-tertiary, var(--text-secondary));
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.lmv-dir-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.lmv-dir-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 3px 6px 3px 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.06);
  max-width: 240px;
}

.lmv-dir-chip.active {
  background: color-mix(in srgb, var(--primary) 22%, transparent);
}

.lmv-dir-chip-main {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 8px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  min-width: 0;
}

.lmv-dir-chip.active .lmv-dir-chip-main {
  color: var(--primary);
}

.lmv-dir-chip-name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lmv-dir-chip-remove {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: none;
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}

.lmv-toolbar-stack {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.lmv-view-tabs-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.lmv-segmented-tabs {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.05);
}

.lmv-tab-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 15px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}

.lmv-tab-pill:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.06);
}

.lmv-tab-pill.active {
  background: var(--primary);
  color: #fff;
}

.lmv-tab-count {
  font-size: var(--bm-fs-micro, 11px);
  opacity: 0.85;
  font-variant-numeric: tabular-nums;
}

.lmv-group-breadcrumb {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding: 6px 14px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--primary) 14%, transparent);
}

.lmv-crumb-label {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-primary);
}

.lmv-crumb-clear {
  border: none;
  padding: 4px 10px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--primary);
  color: #fff;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
}

.lmv-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.lmv-search-box {
  flex: 1 1 280px;
  max-width: 440px;
  height: 40px;
  padding: 0 14px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.06);
  display: flex;
  align-items: center;
  gap: 9px;
  color: var(--text-secondary);
}

.lmv-search-box:focus-within {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-primary);
}

.lmv-search-input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  outline: none;
}

.lmv-search-input::placeholder {
  color: var(--text-tertiary, var(--text-secondary));
}

.lmv-clear-btn {
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

.lmv-pill-groups {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.lmv-pill-cluster {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.045);
}

.lmv-filter-pill {
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

.lmv-filter-pill:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.06);
}

.lmv-filter-pill.active {
  background: var(--primary);
  color: #fff;
}

.lmv-group-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}

.lmv-group-card {
  border-radius: var(--bm-r-lg, 18px);
  padding: 14px 16px;
  background: rgba(255, 255, 255, 0.045);
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  min-width: 0;
  transition: transform 0.16s ease, background 0.16s ease;
}

.lmv-group-card:hover {
  background: rgba(255, 255, 255, 0.085);
  transform: translateY(-2px);
}

.lmv-group-avatar {
  width: 44px;
  height: 44px;
  border-radius: var(--bm-r-md, 14px);
  background: color-mix(in srgb, var(--primary) 20%, transparent);
  color: var(--primary);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  flex-shrink: 0;
}

.lmv-group-info {
  flex: 1;
  min-width: 0;
}

.lmv-group-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lmv-group-sub {
  margin-top: 3px;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lmv-group-play {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: none;
  background: var(--primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  opacity: 0.9;
  transition: transform 0.15s ease, opacity 0.15s ease;
}

.lmv-group-play:hover {
  opacity: 1;
  transform: scale(1.06);
}

.lmv-empty-card {
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

.lmv-empty-badge {
  width: 68px;
  height: 68px;
  border-radius: var(--bm-r-xl, 24px);
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.lmv-empty-title {
  margin: 0;
  font-size: var(--bm-fs-title, 20px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.lmv-empty-desc {
  margin: 0 0 8px;
  max-width: 480px;
  font-size: var(--bm-fs-body, 14.5px);
  line-height: 1.55;
  color: var(--text-secondary);
}

.lmv-empty-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 10px;
}

.lmv-list-stage {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.lmv-monogram {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--primary) 34%, transparent),
    color-mix(in srgb, var(--cyan, var(--primary)) 22%, transparent)
  );
  color: var(--text-primary);
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
}

.lmv-monogram.is-lossless {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--mint, var(--primary)) 34%, transparent),
    color-mix(in srgb, var(--primary) 26%, transparent)
  );
}

.lmv-inline-tech {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-semibold, 600);
  font-variant-numeric: tabular-nums;
}

.lmv-spin {
  animation: lmvSpin 1s linear infinite;
}

@keyframes lmvSpin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* D2 暗紫星云微调 */
.scheme-abyssal_dark.dark-variant-velvet_nebula .lmv-hero-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .lmv-stats-card,
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .lmv-hero-card,
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .lmv-stats-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 13%, rgba(34, 26, 42, 0.72)),
    rgba(24, 19, 30, 0.68)
  );
}

/* L1 晨雾水彩浅色适配 */
.scheme-watercolor_light .lmv-hero-card,
.scheme-watercolor_light .lmv-stats-card,
[data-theme="watercolor_light"] .lmv-hero-card,
[data-theme="watercolor_light"] .lmv-stats-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 7%, rgba(255, 255, 255, 0.86)),
    rgba(255, 255, 255, 0.72)
  );
  box-shadow: 0 18px 36px -24px rgba(45, 38, 28, 0.16);
}

.scheme-watercolor_light .lmv-stat-cell,
.scheme-watercolor_light .lmv-dir-chip,
.scheme-watercolor_light .lmv-segmented-tabs,
.scheme-watercolor_light .lmv-search-box,
.scheme-watercolor_light .lmv-pill-cluster,
.scheme-watercolor_light .lmv-group-card,
.scheme-watercolor_light .lmv-empty-card,
.scheme-watercolor_light .lmv-inline-tech,
[data-theme="watercolor_light"] .lmv-stat-cell,
[data-theme="watercolor_light"] .lmv-dir-chip,
[data-theme="watercolor_light"] .lmv-segmented-tabs,
[data-theme="watercolor_light"] .lmv-search-box,
[data-theme="watercolor_light"] .lmv-pill-cluster,
[data-theme="watercolor_light"] .lmv-group-card,
[data-theme="watercolor_light"] .lmv-empty-card,
[data-theme="watercolor_light"] .lmv-inline-tech {
  background: rgba(255, 255, 255, 0.68);
}

.scheme-watercolor_light .lmv-action-btn:not(.primary):not(.active),
.scheme-watercolor_light .lmv-dir-chip-remove,
.scheme-watercolor_light .lmv-clear-btn,
[data-theme="watercolor_light"] .lmv-action-btn:not(.primary):not(.active),
[data-theme="watercolor_light"] .lmv-dir-chip-remove,
[data-theme="watercolor_light"] .lmv-clear-btn {
  background: rgba(34, 30, 26, 0.07);
}

/* L2 北欧燕麦浅色暖纸适配 */
.scheme-watercolor_light.light-variant-nordic_oat .lmv-hero-card,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-stats-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-hero-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-stats-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 6%, #FCF9F2),
    #F5F0E6
  );
  box-shadow: 0 16px 34px -22px rgba(64, 52, 34, 0.14);
}

.scheme-watercolor_light.light-variant-nordic_oat .lmv-stat-cell,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-dir-chip,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-segmented-tabs,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-search-box,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-pill-cluster,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-group-card,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-empty-card,
.scheme-watercolor_light.light-variant-nordic_oat .lmv-inline-tech,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-stat-cell,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-dir-chip,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-segmented-tabs,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-search-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-pill-cluster,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-group-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-empty-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .lmv-inline-tech {
  background: #F5F0E6;
}
</style>
