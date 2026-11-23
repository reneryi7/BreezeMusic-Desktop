<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useAppStore } from "../../stores/appStore";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";

defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();
const showPlainIds = ref(false);
const isRefreshingStats = ref(false);

onMounted(() => {
  store.refreshCacheStats();
});

const totalCacheMb = computed(() => {
  const sum =
    (store.cacheStats.audioStreamMb || 0) +
    (store.cacheStats.artworkImageMb || 0) +
    (store.cacheStats.playlistSnapshotMb || 0) +
    (store.cacheStats.lyricAndUrlMb || 0);
  return Number(sum.toFixed(2));
});

function formatMb(mb: number): string {
  if (!Number.isFinite(mb) || mb <= 0) return "0 MB";
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  return `${mb.toFixed(mb >= 100 ? 0 : 1)} MB`;
}

const cacheCategories = computed(() => [
  {
    key: "audio" as const,
    label: "音频流媒体缓存",
    desc: "已播歌曲的本地无损/高品分片缓存，下次重听秒开且下载可零耗流复用",
    mb: store.cacheStats.audioStreamMb || 0,
    color: "var(--primary)",
    icon: "headphones",
  },
  {
    key: "image" as const,
    label: "高清封面图片缓存",
    desc: "专辑与歌单高清封面本地缓存文件",
    mb: store.cacheStats.artworkImageMb || 0,
    color: "var(--coral)",
    icon: "image",
  },
  {
    key: "snapshot" as const,
    label: "歌单与元数据快照",
    desc: "收藏歌单缓存与本地配置快照（绝不清理账号登录态与播放历史）",
    mb: store.cacheStats.playlistSnapshotMb || 0,
    color: "var(--mint)",
    icon: "library",
  },
  {
    key: "lyric" as const,
    label: "歌词与直链缓存",
    desc: "已解密 KRC/LRC 歌词及推荐去重记忆",
    mb: store.cacheStats.lyricAndUrlMb || 0,
    color: "var(--gold)",
    icon: "lyric",
  },
]);

const quotaOptions: ReadonlyArray<{ gb: number; label: string }> = [
  { gb: 1, label: "1 GB" },
  { gb: 2, label: "2 GB" },
  { gb: 4, label: "4 GB" },
  { gb: 8, label: "8 GB" },
  { gb: 0, label: "无限制" },
];

const barSegments = computed(() => {
  const total = totalCacheMb.value;
  if (total <= 0) return [];
  return cacheCategories.value
    .filter((c) => c.mb > 0)
    .map((c) => ({
      key: c.key,
      color: c.color,
      pct: Math.max(4, (c.mb / total) * 100),
    }));
});

const deviceDfid = computed(
  () => store.gatewayDiag?.dfid || store.vipStatus?.dfid || "待诊断获取"
);
const deviceGuid = computed(
  () => store.gatewayDiag?.guid || store.vipStatus?.guid || "待诊断获取"
);
const deviceMid = computed(() => store.gatewayDiag?.mid || "待诊断获取");

function maskId(val: string): string {
  if (!val || val === "待诊断获取") return val;
  if (showPlainIds.value || val.length <= 10) return val;
  return `${val.slice(0, 6)}••••••${val.slice(-4)}`;
}

async function copyId(label: string, val: string) {
  if (!val || val === "待诊断获取") return;
  try {
    await navigator.clipboard.writeText(val);
    store.notify(`已复制 ${label} 至剪贴板`);
  } catch {
    store.notify(`${label}: ${val}`);
  }
}

async function handleRefreshStats() {
  if (isRefreshingStats.value) return;
  isRefreshingStats.value = true;
  try {
    await store.refreshCacheStats();
    store.notify("已刷新实时磁盘缓存占用");
  } finally {
    isRefreshingStats.value = false;
  }
}

async function handleClearCategory(
  key: "audio" | "image" | "snapshot" | "lyric" | "all",
  title: string
) {
  const ok = await store.askConfirm({
    title: `清理${title}`,
    message:
      key === "all"
        ? "将清理全部音频流、封面、歌单快照与歌词磁盘缓存（完整保留账号登录态与历史播放记录），是否继续？"
        : `确认清理「${title}」？不会影响您的登录会话、歌单资产与已下载歌曲。`,
    confirmText: "确认清理",
    cancelText: "取消",
    tone: "danger",
  });
  if (ok) {
    await store.clearCacheCategory(key);
  }
}

async function handleResignDfid() {
  const ok = await store.askConfirm({
    title: "重新签发 24 位 DFID 设备标识",
    message:
      "若遇到网关频繁超时、无损解析异常或风控提示，可尝试重新向酷狗概念版签发新的设备标识。此操作不影响账号密码，确认继续？",
    confirmText: "重新签发",
    cancelText: "取消",
    tone: "danger",
  });
  if (ok) {
    await store.resignDeviceAndRefresh();
  }
}

async function handleClearSearchHistory() {
  const ok = await store.askConfirm({
    title: "清空搜索历史",
    message: "清空全部本地搜索记录？",
    confirmText: "清空",
    cancelText: "取消",
    tone: "danger",
  });
  if (ok) {
    store.clearSearchHistory();
    store.notify("已清空本地搜索历史");
  }
}

async function handleResetVisualDefaults() {
  const ok = await store.askConfirm({
    title: "恢复视觉与歌词出厂默认",
    message: "将播放页歌词、桌面歌词排版以及播放舞台视觉偏好恢复为默认设置（不退出账号，不清理本地歌曲）。",
    confirmText: "恢复默认",
    cancelText: "取消",
  });
  if (ok) {
    store.resetLyricSettingsToDefault();
    store.resetDesktopLyricSettingsToDefault();
    store.coverColorStageMode = "follow";
    store.playerBgStyle = "fluid_aurora";
    store.playerStageStyle = "vinyl_turntable";
    store.notify("已恢复播放页与歌词默认视觉设置");
  }
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 1. 四维磁盘缓存可视化仪表盘 -->
    <SettingsCard
      title="四维磁盘缓存仪表盘"
      subtitle="数据自动存放在安装目录（只读目录自动回退 AppData），不乱占系统盘空间"
      icon="disc"
      icon-color="var(--cyan)"
    >
      <template #headerRight>
        <div class="bms-head-actions">
          <button
            type="button"
            class="bms-btn sm"
            :disabled="isRefreshingStats"
            @click="handleRefreshStats"
          >
            <PlumpIcon name="refresh" :size="12" />
            <span>刷新统计</span>
          </button>
          <button
            type="button"
            class="bms-btn sm danger"
            :disabled="totalCacheMb <= 0"
            @click="handleClearCategory('all', '全部缓存')"
          >
            <PlumpIcon name="trash" :size="12" />
            <span>一键全清</span>
          </button>
        </div>
      </template>

      <div
        class="bms-cache-overview"
        :class="{ 'is-flash-highlight': highlightedId === 'storage-cache-dashboard' }"
        data-setting-id="storage-cache-dashboard"
      >
        <div class="bms-cache-total-row">
          <div class="bms-cache-total-copy">
            <span class="bms-cache-total-label">当前磁盘总缓存占用</span>
            <span class="bms-cache-total-num">{{ formatMb(totalCacheMb) }}</span>
          </div>
          <span class="bms-chip muted">
            配额上限：{{ store.cacheStats.maxQuotaGb === 0 ? "无限制" : `${store.cacheStats.maxQuotaGb} GB` }}
          </span>
        </div>

        <div class="bms-cache-bar" role="img" :aria-label="`总缓存占用 ${formatMb(totalCacheMb)}`">
          <span
            v-for="seg in barSegments"
            :key="seg.key"
            class="bms-cache-bar-seg"
            :style="{ width: `${seg.pct}%`, background: seg.color }"
          ></span>
        </div>

        <div class="bms-tile-grid">
          <div
            v-for="cat in cacheCategories"
            :key="cat.key"
            class="bms-cache-card"
          >
            <div class="bms-cache-card-left">
              <span class="bms-cache-dot" :style="{ background: cat.color }"></span>
              <div class="bms-cache-card-copy">
                <div class="bms-cache-card-title">{{ cat.label }}</div>
                <div class="bms-cache-card-val">{{ formatMb(cat.mb) }}</div>
                <div class="bms-cache-card-desc">{{ cat.desc }}</div>
              </div>
            </div>
            <button
              type="button"
              class="bms-btn sm ghost"
              :disabled="cat.mb <= 0"
              @click="handleClearCategory(cat.key, cat.label)"
            >
              清理
            </button>
          </div>
        </div>
      </div>

      <SettingsRow
        title="音频流磁盘缓存配额上限"
        description="超出配额时 Rust 音频代理自动按最久未访问 (LRU) 淘汰旧缓存至 90% 安全水位线"
        icon="sliders"
        icon-color="var(--primary)"
        setting-id="storage-cache-quota"
        :highlighted="highlightedId === 'storage-cache-quota'"
      >
        <div class="bms-pill-group" role="radiogroup" aria-label="音频流磁盘缓存配额上限">
          <button
            v-for="opt in quotaOptions"
            :key="opt.gb"
            type="button"
            role="radio"
            class="bms-pill-opt"
            :class="{ active: store.cacheStats.maxQuotaGb === opt.gb }"
            :aria-checked="store.cacheStats.maxQuotaGb === opt.gb"
            @click="store.setCacheQuotaGb(opt.gb)"
          >
            {{ opt.label }}
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="缓存目录位置"
        description="打开本地磁盘缓存目录查看或核对存储文件"
        icon="folder"
        icon-color="var(--cyan)"
        vertical
      >
        <div class="bms-path-bar">
          <span class="bms-path-pill" :data-tip="store.cacheStats.cacheDirPath">
            {{ store.cacheStats.cacheDirPath }}
          </span>
          <button
            type="button"
            class="bms-btn"
            @click="store.openDirectoryInExplorer('cache')"
          >
            <PlumpIcon name="folder" :size="14" />
            <span>在资源管理器中打开</span>
          </button>
        </div>
      </SettingsRow>
    </SettingsCard>

    <!-- 2. 音源网关诊断与设备身份 -->
    <SettingsCard
      title="酷狗概念版网关诊断与设备身份"
      subtitle="检测 appid=3116 协议网关连通延迟，排查解析失败或重新签发 24 位 DFID 设备标识"
      icon="shield"
      icon-color="var(--mint)"
    >
      <template #headerRight>
        <div class="bms-head-actions">
          <button
            type="button"
            class="bms-btn sm ghost"
            @click="showPlainIds = !showPlainIds"
          >
            <span>{{ showPlainIds ? "隐藏指纹" : "明文查看" }}</span>
          </button>
          <button
            type="button"
            class="bms-btn sm primary"
            :disabled="store.isDiagnosingGateway"
            @click="store.runGatewayDiagnostic()"
          >
            <PlumpIcon name="refresh" :size="12" color="#ffffff" />
            <span>{{ store.isDiagnosingGateway ? "诊断中..." : "一键网关测速" }}</span>
          </button>
        </div>
      </template>

      <div
        class="bms-diag-panel"
        :class="{ 'is-flash-highlight': highlightedId === 'storage-gateway-diag' }"
        data-setting-id="storage-gateway-diag"
      >
        <div class="bms-diag-status-row">
          <div class="bms-diag-pill" :class="store.gatewayDiag ? (store.gatewayDiag.ok ? 'is-ok' : 'is-err') : ''">
            <span class="bms-diag-dot"></span>
            <span>
              {{
                store.gatewayDiag
                  ? `${store.gatewayDiag.statusText} (${store.gatewayDiag.latencyMs} ms)`
                  : "点击右上角「一键网关测速」检测实时链路延迟"
              }}
            </span>
          </div>
          <button
            type="button"
            class="bms-btn sm danger"
            :disabled="store.isDiagnosingGateway"
            @click="handleResignDfid"
          >
            <PlumpIcon name="reset-rotate" :size="12" />
            <span>重签 DFID 设备标识</span>
          </button>
        </div>

        <div class="bms-id-grid">
          <button
            type="button"
            class="bms-id-chip"
            @click="copyId('DFID', deviceDfid)"
          >
            <span class="bms-id-k">DFID</span>
            <span class="bms-id-v">{{ maskId(deviceDfid) }}</span>
            <PlumpIcon name="copy" :size="12" color="var(--text-muted)" />
          </button>
          <button
            type="button"
            class="bms-id-chip"
            @click="copyId('GUID', deviceGuid)"
          >
            <span class="bms-id-k">GUID</span>
            <span class="bms-id-v">{{ maskId(deviceGuid) }}</span>
            <PlumpIcon name="copy" :size="12" color="var(--text-muted)" />
          </button>
          <button
            type="button"
            class="bms-id-chip"
            @click="copyId('MID', deviceMid)"
          >
            <span class="bms-id-k">MID</span>
            <span class="bms-id-v">{{ maskId(deviceMid) }}</span>
            <PlumpIcon name="copy" :size="12" color="var(--text-muted)" />
          </button>
        </div>
      </div>
    </SettingsCard>

    <!-- 3. 历史与偏好数据维护 -->
    <SettingsCard
      title="偏好与历史数据维护"
      subtitle="单项清理历史或重置视觉排版偏好，均不会退出您的登录账号"
      icon="reset-rotate"
      icon-color="var(--coral)"
    >
      <SettingsRow
        title="快速维护工具"
        description="按需清理本地搜索记录或恢复播放页与桌面歌词默认外观"
        icon="trash"
        icon-color="var(--coral)"
        setting-id="storage-data-maintenance"
        :highlighted="highlightedId === 'storage-data-maintenance'"
      >
        <button
          type="button"
          class="bms-btn sm"
          :disabled="store.searchHistory.length === 0"
          @click="handleClearSearchHistory"
        >
          清空搜索历史 ({{ store.searchHistory.length }})
        </button>
        <button
          type="button"
          class="bms-btn sm ghost"
          @click="handleResetVisualDefaults"
        >
          恢复视觉与歌词默认
        </button>
      </SettingsRow>
    </SettingsCard>
  </div>
</template>

<style scoped>
.bms-head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bms-cache-overview {
  display: flex;
  flex-direction: column;
  gap: 14px;
  border-radius: var(--bm-r-lg, 18px);
  transition: box-shadow 0.25s ease;
}

.bms-cache-overview.is-flash-highlight,
.bms-diag-panel.is-flash-highlight {
  box-shadow: 0 0 28px -2px color-mix(in srgb, var(--primary) 48%, transparent);
}

.bms-cache-total-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.bms-cache-total-copy {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.bms-cache-total-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--text-secondary);
}

.bms-cache-total-num {
  font-size: var(--bm-fs-title, 20px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.bms-cache-bar {
  width: 100%;
  height: 10px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bms-surface-well);
  overflow: hidden;
  display: flex;
  gap: 2px;
}

.bms-cache-bar-seg {
  height: 100%;
  border-radius: var(--bm-r-pill, 999px);
  transition: width 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}

.bms-cache-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 13px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bms-surface-row);
}

.bms-cache-card-left {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 0;
}

.bms-cache-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  margin-top: 5px;
  flex-shrink: 0;
}

.bms-cache-card-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.bms-cache-card-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--text-secondary);
}

.bms-cache-card-val {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.bms-cache-card-desc {
  font-size: var(--bm-fs-micro, 11px);
  line-height: 1.45;
  color: var(--text-muted);
}

.bms-diag-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  border-radius: var(--bm-r-lg, 18px);
}

.bms-diag-status-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding: 12px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bms-surface-row);
}

.bms-diag-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--text-secondary);
}

.bms-diag-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-muted);
}

.bms-diag-pill.is-ok .bms-diag-dot {
  background: var(--mint);
  box-shadow: 0 0 10px var(--mint);
}

.bms-diag-pill.is-ok {
  color: var(--text-primary);
}

.bms-diag-pill.is-err .bms-diag-dot {
  background: var(--coral);
}

.bms-id-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

@media (max-width: 820px) {
  .bms-id-grid {
    grid-template-columns: 1fr;
  }
}

.bms-id-chip {
  border: none;
  padding: 11px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bms-surface-row);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  cursor: pointer;
  transition: background 0.2s ease;
}

.bms-id-chip:hover {
  background: var(--bms-surface-row-hover);
}

.bms-id-chip:focus-visible {
  outline: 2px solid var(--primary-text, var(--primary));
  outline-offset: 2px;
}

.bms-id-k {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 900;
  color: var(--primary-text, var(--primary));
  letter-spacing: 0.05em;
}

.bms-id-v {
  flex: 1;
  text-align: left;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-cache-card,
.scheme-watercolor_light.light-variant-nordic_oat .bms-diag-status-row,
.scheme-watercolor_light.light-variant-nordic_oat .bms-id-chip,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-cache-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-diag-status-row,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-id-chip {
  background: var(--bms-surface-row);
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-cache-bar {
  background: var(--bms-surface-well);
}
</style>
