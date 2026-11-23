<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useAppStore } from "../../stores/appStore";
import type {
  ConceptAudioQualityValue,
  DownloadFileNamingMode,
  DownloadFolderOrganizeMode,
  DownloadHistoryRecord,
  DownloadQueueItem,
} from "../../types";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

const props = defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();

type SubTab = "queue" | "history" | "prefs";
type QueueFilter = "all" | "active" | "paused" | "done" | "error";
type HistoryQualityFilter = "all" | "hires" | "sq" | "hq" | "std";

const activeSubTab = ref<SubTab>("queue");
const queueFilter = ref<QueueFilter>("all");
const historySearch = ref("");
const historyQualityFilter = ref<HistoryQualityFilter>("all");

watch(
  () => props.highlightedId,
  (id) => {
    if (id === "downloads-history") activeSubTab.value = "history";
    else if (id === "downloads-prefs") activeSubTab.value = "prefs";
    else if (id === "downloads-queue") activeSubTab.value = "queue";
  },
  { immediate: true }
);

const dlQualityOptions: ReadonlyArray<{
  id: ConceptAudioQualityValue;
  label: string;
}> = [
  { id: "high", label: "Hi-Res 无损" },
  { id: "flac", label: "SQ FLAC" },
  { id: "320", label: "HQ 320K" },
  { id: "128", label: "标准 128K" },
];

const fileNamingOptions: ReadonlyArray<{
  id: DownloadFileNamingMode;
  label: string;
}> = [
  { id: "artist_title", label: "歌手 - 歌名" },
  { id: "title_artist", label: "歌名 - 歌手" },
  { id: "title_only", label: "仅歌名" },
];

const folderOrganizeOptions: ReadonlyArray<{
  id: DownloadFolderOrganizeMode;
  label: string;
}> = [
  { id: "flat", label: "平铺目录" },
  { id: "artist", label: "按歌手建目录" },
  { id: "artist_album", label: "按歌手 / 专辑" },
];

const concurrencyOptions: ReadonlyArray<1 | 2 | 3> = [1, 2, 3];

const namingPreviewExample = computed(() => {
  const filePart =
    store.downloadFileNaming === "title_artist"
      ? "晴天 - 周杰伦.flac"
      : store.downloadFileNaming === "title_only"
      ? "晴天.flac"
      : "周杰伦 - 晴天.flac";
  if (store.downloadFolderOrganize === "artist") return `周杰伦\\${filePart}`;
  if (store.downloadFolderOrganize === "artist_album") return `周杰伦\\叶惠美\\${filePart}`;
  return filePart;
});

const normalizedDownloadQuality = computed<ConceptAudioQualityValue>(() => {
  const q = store.downloadQuality;
  if (q === "128" || q === "320" || q === "flac" || q === "high") return q;
  return "high";
});

const activeQueueCount = computed(
  () =>
    store.downloadQueue.filter(
      (t) =>
        t.status === "queued" ||
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging"
    ).length
);

const pausedQueueCount = computed(
  () => store.downloadQueue.filter((t) => t.status === "paused").length
);

const failedQueueCount = computed(
  () =>
    store.downloadQueue.filter(
      (t) => t.status === "error" || t.status === "canceled"
    ).length
);

const doneQueueCount = computed(
  () => store.downloadQueue.filter((t) => t.status === "done").length
);

const filteredQueue = computed<DownloadQueueItem[]>(() => {
  return store.downloadQueue.filter((t) => {
    if (queueFilter.value === "all") return true;
    if (queueFilter.value === "active") {
      return (
        t.status === "queued" ||
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging"
      );
    }
    if (queueFilter.value === "paused") return t.status === "paused";
    if (queueFilter.value === "done") return t.status === "done";
    return t.status === "error" || t.status === "canceled";
  });
});

const historyTotalSizeText = computed(() => {
  let bytes = 0;
  for (const r of store.downloadHistory) {
    if (r.fileSizeBytes && Number.isFinite(r.fileSizeBytes)) {
      bytes += r.fileSizeBytes;
    } else if (r.fileSizeText) {
      const m = r.fileSizeText.match(/([\d.]+)\s*(GB|MB|KB)/i);
      if (m) {
        const val = parseFloat(m[1]);
        const unit = m[2].toUpperCase();
        if (unit === "GB") bytes += val * 1024 * 1024 * 1024;
        else if (unit === "MB") bytes += val * 1024 * 1024;
        else if (unit === "KB") bytes += val * 1024;
      }
    }
  }
  if (bytes <= 0) return "0 MB";
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
});

function matchHistoryQuality(rec: DownloadHistoryRecord, filter: HistoryQualityFilter): boolean {
  if (filter === "all") return true;
  const lbl = String(rec.qualityLabel || "").toLowerCase();
  if (filter === "hires") return lbl.includes("hi-res") || lbl.includes("hires");
  if (filter === "sq") return lbl.includes("flac") || lbl.includes("sq") || lbl.includes("无损");
  if (filter === "hq") return lbl.includes("320") || lbl.includes("hq");
  return lbl.includes("128") || lbl.includes("标准") || (!lbl.includes("flac") && !lbl.includes("320") && !lbl.includes("hi-res"));
}

const filteredHistory = computed<DownloadHistoryRecord[]>(() => {
  const kw = historySearch.value.trim().toLowerCase();
  return store.downloadHistory.filter((rec) => {
    if (!matchHistoryQuality(rec, historyQualityFilter.value)) return false;
    if (!kw) return true;
    return (
      String(rec.title || "").toLowerCase().includes(kw) ||
      String(rec.artist || "").toLowerCase().includes(kw) ||
      String(rec.album || "").toLowerCase().includes(kw) ||
      String(rec.savedPath || "").toLowerCase().includes(kw)
    );
  });
});

function formatQueueQualityLabel(task: DownloadQueueItem): string {
  if (task.qualityLabel) return task.qualityLabel;
  if (task.quality === "high") return "Hi-Res";
  if (task.quality === "flac") return "SQ FLAC";
  if (task.quality === "320") return "HQ 320K";
  return "128K MP3";
}

function formatQueueStatusText(task: DownloadQueueItem): string {
  switch (task.status) {
    case "queued":
      return "排队等待";
    case "paused":
      return "已暂停";
    case "resolving":
      return "解析直链";
    case "downloading":
      return `${Math.round(task.percent || 0)}%`;
    case "tagging":
      return "写入封面/歌词";
    case "done":
      return "已完成";
    case "canceled":
      return "已取消";
    case "error":
      return "失败";
    default:
      return "";
  }
}

function formatSpeed(bps?: number): string {
  const n = Number(bps || 0);
  if (n < 1024) return "";
  if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(2)} MB/s`;
  return `${Math.round(n / 1024)} KB/s`;
}

function formatEta(sec?: number): string {
  const s = Math.round(Number(sec || 0));
  if (s <= 0) return "";
  if (s < 60) return `剩余 ${s}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return `剩余 ${m}m${rem > 0 ? `${rem}s` : ""}`;
}

function formatQueueBytes(task: DownloadQueueItem): string {
  const dlMb = (task.downloadedBytes / (1024 * 1024)).toFixed(1);
  if (task.totalBytes > 0) {
    const totalMb = (task.totalBytes / (1024 * 1024)).toFixed(1);
    return `${dlMb} / ${totalMb} MB`;
  }
  if (task.downloadedBytes > 0) return `${dlMb} MB`;
  return "";
}

function isQueueTaskIndeterminate(task: DownloadQueueItem): boolean {
  return task.status === "queued" || task.status === "resolving" || task.status === "tagging";
}

async function handleClearHistory() {
  const ok = await store.askConfirm({
    title: "清空全部下载记录",
    message: "仅清空客户端内的历史清单，不会删除已保存到本地磁盘的音频或歌词文件。",
    confirmText: "确认清空",
    cancelText: "取消",
    tone: "danger",
  });
  if (ok) {
    store.clearDownloadHistory();
  }
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 顶部 3 子标签栏：队列 / 记录 / 偏好 -->
    <div class="bms-subtabs" role="tablist" aria-label="下载管理分区">
      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeSubTab === 'queue' }"
        :aria-selected="activeSubTab === 'queue'"
        @click="activeSubTab = 'queue'"
      >
        <PlumpIcon name="download" :size="14" :color="activeSubTab === 'queue' ? '#ffffff' : 'var(--primary)'" />
        <span>下载队列</span>
        <span v-if="store.downloadQueue.length > 0" class="bms-subtab-count">
          {{ activeQueueCount > 0 ? `${activeQueueCount} 进行中` : store.downloadQueue.length }}
        </span>
      </button>

      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeSubTab === 'history' }"
        :aria-selected="activeSubTab === 'history'"
        @click="activeSubTab = 'history'"
      >
        <PlumpIcon name="history" :size="14" :color="activeSubTab === 'history' ? '#ffffff' : 'var(--mint)'" />
        <span>下载记录</span>
        <span v-if="store.downloadHistory.length > 0" class="bms-subtab-count">
          {{ store.downloadHistory.length }}
        </span>
      </button>

      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeSubTab === 'prefs' }"
        :aria-selected="activeSubTab === 'prefs'"
        @click="activeSubTab = 'prefs'"
      >
        <PlumpIcon name="sliders" :size="14" :color="activeSubTab === 'prefs' ? '#ffffff' : 'var(--gold)'" />
        <span>下载偏好</span>
      </button>
    </div>

    <Transition
      :name="store.motionLevel === 'none' ? '' : 'bms-pane-fade'"
      mode="out-in"
    >
    <!-- ==================== 子页 1：下载队列 ==================== -->
    <div v-if="activeSubTab === 'queue'" key="queue" class="bms-panel-stack">
      <SettingsCard
        title="并发下载任务队列"
        :subtitle="
          activeQueueCount > 0
            ? `${activeQueueCount} 首进行中 · 当前并发 ${store.downloadConcurrency} 路`
            : '支持并发下载、实时测速、暂停恢复、置顶优先与真正取消（自动清理临时文件）'
        "
        icon="download"
        icon-color="var(--primary)"
      >
        <!-- 队列控制与状态筛选条 -->
        <div
          class="bmd-toolbar"
          data-setting-id="downloads-queue"
          :class="{ 'is-flash-highlight': highlightedId === 'downloads-queue' }"
        >
          <div class="bmd-filter-pills">
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: queueFilter === 'all' }"
              @click="queueFilter = 'all'"
            >
              全部 ({{ store.downloadQueue.length }})
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: queueFilter === 'active' }"
              @click="queueFilter = 'active'"
            >
              进行中 ({{ activeQueueCount }})
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: queueFilter === 'paused' }"
              @click="queueFilter = 'paused'"
            >
              已暂停 ({{ pausedQueueCount }})
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: queueFilter === 'done' }"
              @click="queueFilter = 'done'"
            >
              已完成 ({{ doneQueueCount }})
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: queueFilter === 'error' }"
              @click="queueFilter = 'error'"
            >
              失败/取消 ({{ failedQueueCount }})
            </button>
          </div>

          <div class="bmd-batch-actions">
            <button
              v-if="activeQueueCount > 0"
              type="button"
              class="bms-btn sm"
              @click="store.pauseAllDownloads()"
            >
              <PlumpIcon name="pause" :size="12" />
              <span>全部暂停</span>
            </button>
            <button
              v-if="pausedQueueCount > 0"
              type="button"
              class="bms-btn sm primary"
              @click="store.resumeAllDownloads()"
            >
              <PlumpIcon name="play" :size="12" color="#ffffff" />
              <span>全部恢复</span>
            </button>
            <button
              v-if="failedQueueCount > 0"
              type="button"
              class="bms-btn sm"
              @click="store.retryAllFailedDownloads()"
            >
              <PlumpIcon name="refresh" :size="12" />
              <span>重试失败</span>
            </button>
            <button
              v-if="doneQueueCount + failedQueueCount > 0"
              type="button"
              class="bms-btn sm ghost"
              @click="store.clearCompletedDownloads()"
            >
              <PlumpIcon name="trash" :size="12" />
              <span>清理已结束</span>
            </button>
          </div>
        </div>

        <!-- 队列任务列表（支持平滑重排让位） -->
        <TransitionGroup
          v-if="filteredQueue.length > 0"
          tag="div"
          name="bm-list-reflow"
          class="bmd-queue-list"
        >
          <div
            v-for="task in filteredQueue"
            :key="task.taskId"
            class="bmd-queue-card"
            :class="{
              'is-done': task.status === 'done',
              'is-error': task.status === 'error' || task.status === 'canceled',
            }"
          >
            <div class="bmd-q-head">
              <div class="bmd-q-cover">
                <img
                  v-if="task.song.coverUrl"
                  :src="task.song.coverUrl"
                  :alt="task.song.title"
                  loading="lazy"
                />
                <PlumpIcon v-else name="music" :size="18" color="var(--text-muted)" />
              </div>

              <div class="bmd-q-main">
                <div class="bmd-q-title-row">
                  <span class="bmd-q-title">{{ task.song.title }}</span>
                  <span class="bmd-q-artist">{{ task.song.artist }}</span>
                  <div class="bmd-q-badges">
                    <span class="bms-chip gold">{{ formatQueueQualityLabel(task) }}</span>
                    <span
                      class="bms-chip"
                      :class="
                        task.status === 'done'
                          ? 'mint'
                          : task.status === 'error' || task.status === 'canceled'
                          ? 'coral'
                          : 'muted'
                      "
                    >
                      {{ formatQueueStatusText(task) }}
                    </span>
                  </div>
                </div>

                <progress
                  v-if="
                    task.status === 'queued' ||
                    task.status === 'resolving' ||
                    task.status === 'downloading' ||
                    task.status === 'tagging'
                  "
                  class="bms-native-progress"
                  max="100"
                  :value="isQueueTaskIndeterminate(task) ? undefined : Math.round(task.percent || 0)"
                  :aria-label="`${task.song.title} 下载进度`"
                ></progress>

                <div class="bmd-q-foot">
                  <div class="bmd-q-telemetry" role="status" aria-live="polite">
                    <span>{{ task.message }}</span>
                    <span v-if="formatQueueBytes(task)" class="bmd-mono">
                      {{ formatQueueBytes(task) }}
                    </span>
                    <span
                      v-if="task.status === 'downloading' && formatSpeed(task.speedBytesPerSec)"
                      class="bmd-speed"
                    >
                      {{ formatSpeed(task.speedBytesPerSec) }}
                    </span>
                    <span
                      v-if="task.status === 'downloading' && formatEta(task.etaSeconds)"
                      class="bmd-mono"
                    >
                      {{ formatEta(task.etaSeconds) }}
                    </span>
                  </div>

                  <div class="bmd-q-actions">
                    <button
                      v-if="task.status === 'queued'"
                      type="button"
                      class="bms-btn sm ghost"
                      @click="store.prioritizeDownloadTask(task.taskId)"
                    >
                      <PlumpIcon name="top" :size="12" />
                      <span>置顶</span>
                    </button>
                    <button
                      v-if="
                        task.status === 'queued' ||
                        task.status === 'resolving' ||
                        task.status === 'downloading'
                      "
                      type="button"
                      class="bms-btn sm ghost"
                      @click="store.pauseDownloadTask(task.taskId)"
                    >
                      <PlumpIcon name="pause" :size="12" />
                      <span>暂停</span>
                    </button>
                    <button
                      v-if="task.status === 'paused'"
                      type="button"
                      class="bms-btn sm primary"
                      @click="store.resumeDownloadTask(task.taskId)"
                    >
                      <PlumpIcon name="play" :size="12" color="#ffffff" />
                      <span>继续</span>
                    </button>
                    <button
                      v-if="
                        task.status === 'queued' ||
                        task.status === 'paused' ||
                        task.status === 'resolving' ||
                        task.status === 'downloading'
                      "
                      type="button"
                      class="bms-btn sm danger"
                      @click="store.cancelDownloadTask(task.taskId)"
                    >
                      <span>取消</span>
                    </button>
                    <button
                      v-if="task.status === 'error' || task.status === 'canceled'"
                      type="button"
                      class="bms-btn sm primary"
                      @click="store.retryDownloadTask(task.taskId)"
                    >
                      <PlumpIcon name="refresh" :size="12" color="#ffffff" />
                      <span>重试</span>
                    </button>
                    <button
                      v-if="task.status === 'done' && task.savedAudioPath"
                      type="button"
                      class="bms-btn sm primary"
                      @click="
                        store.playDownloadedLocalFile(task.savedAudioPath, {
                          title: task.song.title,
                          artist: task.song.artist,
                        })
                      "
                    >
                      <PlumpIcon name="play" :size="12" color="#ffffff" />
                      <span>播放</span>
                    </button>
                    <button
                      v-if="task.status === 'done' && task.savedAudioPath"
                      type="button"
                      class="bms-btn sm"
                      @click="store.revealFileInExplorer(task.savedAudioPath)"
                    >
                      <PlumpIcon name="locate" :size="12" />
                      <span>定位文件</span>
                    </button>
                    <button
                      v-if="task.status === 'done' && task.savedLrcPath"
                      type="button"
                      class="bms-btn sm ghost"
                      @click="store.revealFileInExplorer(task.savedLrcPath)"
                    >
                      <PlumpIcon name="lyric" :size="12" />
                      <span>定位歌词</span>
                    </button>
                    <button
                      v-if="
                        task.status === 'done' ||
                        task.status === 'error' ||
                        task.status === 'canceled'
                      "
                      type="button"
                      class="bms-btn sm ghost"
                      aria-label="移出队列"
                      @click="store.removeDownloadQueueItem(task.taskId)"
                    >
                      <PlumpIcon name="close" :size="11" />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </TransitionGroup>

        <div v-else class="bmd-empty-box">
          <PlumpIcon name="download" :size="26" color="var(--text-muted)" />
          <p>当前筛选下暂无下载任务</p>
          <span>可在任意歌曲右键菜单或播放页「更多」中发起单曲与批量下载</span>
        </div>
      </SettingsCard>
    </div>

    <!-- ==================== 子页 2：下载记录 ==================== -->
    <div v-else-if="activeSubTab === 'history'" key="history" class="bms-panel-stack">
      <SettingsCard
        title="下载历史归档"
        :subtitle="`已持久化记录 ${store.downloadHistory.length} 首（上限 500 条） · 累计体积 ${historyTotalSizeText}`"
        icon="history"
        icon-color="var(--mint)"
      >
        <template #actions>
          <button
            type="button"
            class="bms-btn sm"
            @click="store.openDirectoryInExplorer(store.downloadMusicDir || 'downloads')"
          >
            <PlumpIcon name="folder" :size="13" />
            <span>打开下载目录</span>
          </button>
          <button
            v-if="store.downloadHistory.length > 0"
            type="button"
            class="bms-btn sm danger"
            @click="handleClearHistory"
          >
            <PlumpIcon name="trash" :size="12" />
            <span>清空记录</span>
          </button>
        </template>

        <div
          class="bmd-toolbar"
          data-setting-id="downloads-history"
          :class="{ 'is-flash-highlight': highlightedId === 'downloads-history' }"
        >
          <div class="bmd-hist-search">
            <PlumpIcon name="search" :size="14" color="var(--text-muted)" />
            <input
              v-model="historySearch"
              type="search"
              class="bmd-hist-input"
              placeholder="搜索已下载曲目、歌手、专辑或路径…"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
          <div class="bmd-filter-pills">
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: historyQualityFilter === 'all' }"
              @click="historyQualityFilter = 'all'"
            >
              全部
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: historyQualityFilter === 'hires' }"
              @click="historyQualityFilter = 'hires'"
            >
              Hi-Res
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: historyQualityFilter === 'sq' }"
              @click="historyQualityFilter = 'sq'"
            >
              SQ 无损
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: historyQualityFilter === 'hq' }"
              @click="historyQualityFilter = 'hq'"
            >
              HQ 320K
            </button>
            <button
              type="button"
              class="bmd-filter-btn"
              :class="{ active: historyQualityFilter === 'std' }"
              @click="historyQualityFilter = 'std'"
            >
              标准
            </button>
          </div>
        </div>

        <div v-if="filteredHistory.length > 0" class="bmd-history-list">
          <div
            v-for="item in filteredHistory"
            :key="item.id"
            class="bmd-history-row"
          >
            <div class="bmd-q-cover sm">
              <img
                v-if="item.coverUrl"
                :src="item.coverUrl"
                :alt="item.title"
                loading="lazy"
              />
              <PlumpIcon v-else name="music" :size="15" color="var(--text-muted)" />
            </div>

            <div class="bmd-hist-info">
              <div class="bmd-hist-title-line">
                <span class="bmd-hist-title">{{ item.title }}</span>
                <span class="bmd-hist-artist">{{ item.artist }}</span>
                <span class="bms-chip gold">{{ item.qualityLabel }}</span>
              </div>
              <div class="bmd-hist-sub">
                <span>{{ item.fileSizeText }}</span>
                <span>·</span>
                <span>{{ item.timestampText }}</span>
                <span v-if="item.savedPath">· {{ item.savedPath }}</span>
              </div>
            </div>

            <div class="bmd-hist-actions">
              <button
                v-if="item.savedPath"
                type="button"
                class="bms-btn sm primary"
                @click="
                  store.playDownloadedLocalFile(item.savedPath, {
                    title: item.title,
                    artist: item.artist,
                    album: item.album,
                    coverUrl: item.coverUrl,
                  })
                "
              >
                <PlumpIcon name="play" :size="12" color="#ffffff" />
                <span>播放</span>
              </button>
              <button
                v-if="item.savedPath"
                type="button"
                class="bms-btn sm"
                @click="store.revealFileInExplorer(item.savedPath)"
              >
                <PlumpIcon name="locate" :size="12" />
                <span>定位</span>
              </button>
              <button
                v-if="item.savedLrcPath"
                type="button"
                class="bms-btn sm ghost"
                @click="store.revealFileInExplorer(item.savedLrcPath)"
              >
                <PlumpIcon name="lyric" :size="12" />
                <span>歌词</span>
              </button>
              <button
                v-if="item.songSnapshot"
                type="button"
                class="bms-btn sm ghost"
                @click="store.redownloadHistoryItem(item)"
              >
                <PlumpIcon name="refresh" :size="12" />
                <span>重下</span>
              </button>
              <button
                type="button"
                class="bms-btn sm ghost"
                aria-label="删除此条下载记录"
                @click="store.removeDownloadHistoryItem(item.id)"
              >
                <PlumpIcon name="close" :size="11" />
              </button>
            </div>
          </div>
        </div>

        <div v-else class="bmd-empty-box">
          <PlumpIcon name="history" :size="26" color="var(--text-muted)" />
          <p>{{ store.downloadHistory.length > 0 ? "没有匹配的下载记录" : "暂无下载记录" }}</p>
          <span>下载完成的歌曲会自动在此留存记录，方便随时回播或定位文件</span>
        </div>
      </SettingsCard>
    </div>

    <!-- ==================== 子页 3：下载偏好 ==================== -->
    <div v-else key="prefs" class="bms-panel-stack">
      <SettingsCard
        title="音质、目录与归档规则"
        subtitle="自定义音频与独立歌词的保存位置、命名方式与目录结构"
        icon="folder"
        icon-color="var(--gold)"
      >
        <SettingsRow
          setting-id="downloads-prefs"
          :highlighted="highlightedId === 'downloads-prefs'"
          title="默认下载音质"
          description="优先下载所选音质；若当前歌曲无更高档位，自动向下选取可用最高音质"
        >
          <div class="bms-pill-group" role="radiogroup" aria-label="默认下载音质">
            <button
              v-for="opt in dlQualityOptions"
              :key="opt.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: normalizedDownloadQuality === opt.id }"
              :aria-checked="normalizedDownloadQuality === opt.id"
              @click="store.downloadQuality = opt.id"
            >
              {{ opt.label }}
            </button>
          </div>
        </SettingsRow>

        <SettingsRow
          title="最大并发下载任务数"
          description="同时下载的曲目数量（建议 2 路，兼顾速度与网关稳定性）"
        >
          <div class="bms-pill-group" role="radiogroup" aria-label="最大并发下载任务数">
            <button
              v-for="c in concurrencyOptions"
              :key="c"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.downloadConcurrency === c }"
              :aria-checked="store.downloadConcurrency === c"
              @click="store.downloadConcurrency = c"
            >
              {{ c }} 路并发
            </button>
          </div>
        </SettingsRow>

        <SettingsRow
          title="音乐文件保存目录"
          description="下载完成的 FLAC / MP3 音频保存位置"
          vertical
        >
          <div class="bms-path-bar">
            <span class="bms-path-pill">{{ store.downloadMusicDir }}</span>
            <button type="button" class="bms-btn" @click="store.browseDownloadMusicDir()">
              <PlumpIcon name="folder" :size="14" />
              <span>更改目录</span>
            </button>
            <button
              type="button"
              class="bms-btn ghost"
              @click="store.openDirectoryInExplorer(store.downloadMusicDir || 'downloads')"
            >
              <span>打开文件夹</span>
            </button>
          </div>
        </SettingsRow>

        <SettingsRow
          title="独立歌词 (.lrc) 保存目录"
          description="开启独立歌词导出时，同名 .lrc 文件写入此目录"
          vertical
        >
          <div class="bms-path-bar">
            <span class="bms-path-pill">{{ store.downloadLrcDir }}</span>
            <button type="button" class="bms-btn" @click="store.browseDownloadLrcDir()">
              <PlumpIcon name="folder" :size="14" />
              <span>更改目录</span>
            </button>
            <button
              type="button"
              class="bms-btn ghost"
              @click="store.openDirectoryInExplorer(store.downloadLrcDir || 'lyrics')"
            >
              <span>打开文件夹</span>
            </button>
          </div>
        </SettingsRow>

        <SettingsRow
          title="文件命名规则"
          :description="`示例输出：${namingPreviewExample}`"
        >
          <div class="bms-pill-group">
            <button
              v-for="opt in fileNamingOptions"
              :key="opt.id"
              type="button"
              class="bms-pill-opt"
              :class="{ active: store.downloadFileNaming === opt.id }"
              @click="store.downloadFileNaming = opt.id"
            >
              {{ opt.label }}
            </button>
          </div>
        </SettingsRow>

        <SettingsRow
          title="目录自动归档方式"
          description="同时作用于音频与独立歌词文件，自动创建歌手/专辑子目录"
        >
          <div class="bms-pill-group">
            <button
              v-for="opt in folderOrganizeOptions"
              :key="opt.id"
              type="button"
              class="bms-pill-opt"
              :class="{ active: store.downloadFolderOrganize === opt.id }"
              @click="store.downloadFolderOrganize = opt.id"
            >
              {{ opt.label }}
            </button>
          </div>
        </SettingsRow>
      </SettingsCard>

      <SettingsCard
        title="纯 Rust 二进制标签与自动化联动"
        subtitle="原生写入 FLAC Vorbis / MP3 ID3v2 高清封面、歌词并自动同步本地曲库"
        icon="sparkles"
        icon-color="var(--mint)"
      >
        <SettingsRow
          title="内嵌 1000P 高清专辑封面与元数据"
          description="自动写入歌名、歌手、专辑、发行日期与封面图"
        >
          <SettingsSwitch v-model="store.embedCoverAndTags" />
        </SettingsRow>

        <SettingsRow
          title="将时间轴歌词直接内嵌至音频标签"
          description="写入 FLAC LYRICS / MP3 USLT 帧，离线播放免外挂歌词文件"
        >
          <SettingsSwitch v-model="store.embedLyricsInAudio" />
        </SettingsRow>

        <SettingsRow
          title="额外导出独立同名 .lrc 文件"
          description="在歌词目录生成 UTF-8 标准 .lrc 文件"
        >
          <SettingsSwitch v-model="store.exportStandaloneLrc" />
        </SettingsRow>

        <SettingsRow
          title="合并双语翻译歌词"
          description="外语曲目存在官方译文时，按同时间戳合并导出"
        >
          <SettingsSwitch v-model="store.bilingualLrcExport" />
        </SettingsRow>

        <SettingsRow
          title="跳过目标目录中已存在的同名音频"
          description="批量下载时避免重复覆盖已有文件"
        >
          <SettingsSwitch v-model="store.skipExistingFiles" />
        </SettingsRow>

        <SettingsRow
          title="下载完成后自动静默刷新本地曲库"
          description="新下载音频立即同步至「本地音乐」列表"
        >
          <SettingsSwitch v-model="store.autoSyncLocalAfterDownload" />
        </SettingsRow>
      </SettingsCard>
    </div>
    </Transition>
  </div>
</template>

<style scoped>
.bmd-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  border-radius: var(--bm-r-md, 14px);
}

.bmd-toolbar.is-flash-highlight {
  box-shadow: 0 0 24px -2px color-mix(in srgb, var(--primary) 45%, transparent);
}

.bmd-filter-pills {
  position: relative;
  isolation: isolate;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bms-surface-well);
  flex-wrap: wrap;
}

.bmd-batch-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.bmd-filter-btn {
  position: relative;
  z-index: 1;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  padding: 5px 12px;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.18s ease, color 0.2s ease;
}

.bmd-filter-btn:hover:not(.active) {
  color: var(--text-primary);
  background: var(--bms-neutral-control, var(--bg-hover-square));
}

.bmd-filter-btn.active {
  background: var(--bms-focus-grad, var(--grad-kugou-primary));
  color: #ffffff;
}

.bmd-queue-list,
.bmd-history-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.bmd-queue-card {
  border-radius: var(--bm-r-lg, 18px);
  padding: 14px 16px;
  background: var(--bms-surface-row);
}

.bmd-queue-card.is-error {
  background: color-mix(in srgb, var(--coral) 11%, var(--bms-surface-row));
}

.bmd-q-head {
  display: flex;
  align-items: center;
  gap: 14px;
}

.bmd-q-cover {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  background: var(--bg-card);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.bmd-q-cover.sm {
  width: 40px;
  height: 40px;
  border-radius: 10px;
}

.bmd-q-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.bmd-q-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bmd-q-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bmd-q-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bmd-q-artist {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.bmd-q-badges {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.bmd-q-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
}

.bmd-q-telemetry {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.bmd-mono {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.bmd-speed {
  color: var(--primary-text, var(--primary));
  font-weight: var(--bm-fw-bold, 700);
  font-variant-numeric: tabular-nums;
}

.bmd-q-actions,
.bmd-hist-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  flex-shrink: 0;
}

.bmd-hist-search {
  flex: 1 1 240px;
  min-block-size: 38px;
  padding: 5px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bms-surface-well);
  display: flex;
  align-items: center;
  gap: 8px;
}

.bmd-hist-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
}

.bmd-history-row {
  display: flex;
  align-items: center;
  gap: 13px;
  padding: 12px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bms-surface-row);
  flex-wrap: wrap;
}

.bmd-hist-info {
  flex: 1;
  min-width: 220px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bmd-hist-title-line {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bmd-hist-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bmd-hist-artist {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.bmd-hist-sub {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bmd-empty-box {
  padding: 40px 20px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bms-surface-row);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  text-align: center;
  color: var(--text-secondary);
}

.bmd-empty-box p {
  margin: 0;
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bmd-empty-box span {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-muted);
}

/* L2 晨雾燕麦 / D2 丝绒星云特化 */
.scheme-watercolor_light.light-variant-nordic_oat .bmd-queue-card,
.scheme-watercolor_light.light-variant-nordic_oat .bmd-history-row,
.scheme-watercolor_light.light-variant-nordic_oat .bmd-empty-box,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bmd-queue-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bmd-history-row,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bmd-empty-box {
  background: var(--bms-surface-row);
}

.scheme-watercolor_light.light-variant-nordic_oat .bmd-hist-search,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bmd-hist-search {
  background: var(--bms-surface-well);
}
</style>
