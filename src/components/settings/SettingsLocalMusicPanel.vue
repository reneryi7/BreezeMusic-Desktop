<script setup lang="ts">
import { computed } from "vue";
import { useAppStore } from "../../stores/appStore";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();

const minDurationOptions = [
  { value: 0, label: "不限制" },
  { value: 15, label: "< 15 秒" },
  { value: 30, label: "< 30 秒" },
  { value: 60, label: "< 60 秒" },
];

const minFileSizeOptions = [
  { value: 0, label: "不限制" },
  { value: 100, label: "< 100 KB" },
  { value: 500, label: "< 500 KB" },
  { value: 1024, label: "< 1 MB" },
];

const currentDir = computed(() =>
  (store.localMusicDir || store.downloadMusicDir || "./downloads").trim()
);

const managedDirs = computed<string[]>(() => {
  const list = [...store.localMusicSavedDirs];
  const cur = currentDir.value;
  if (cur) {
    const curNorm = cur.replace(/[\\/]+$/, "").toLowerCase();
    if (!list.some((d) => d.replace(/[\\/]+$/, "").toLowerCase() === curNorm)) {
      list.unshift(cur);
    }
  }
  return list;
});

const losslessCount = computed(
  () =>
    store.localMusicSongs.filter(
      (s) => s.isHiRes || s.hasHiRes || s.isLossless || s.hasFlac
    ).length
);

function isDirActive(dir: string): boolean {
  return (
    dir.replace(/[\\/]+$/, "").toLowerCase() ===
    currentDir.value.replace(/[\\/]+$/, "").toLowerCase()
  );
}

async function handleSwitchDir(dir: string) {
  await store.scanLocalMusicLibrary(dir);
}

function updateRuleAndHint(patch: Parameters<typeof store.updateLocalMusicSettings>[0]) {
  store.updateLocalMusicSettings(patch);
  if (store.localMusicSongs.length > 0) {
    void store.scanLocalMusicLibrary(undefined, true);
  }
}

function goLocalMusicView() {
  store.openLocalMusicPage();
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 顶部本地曲库概览条 -->
    <div class="bml-hero-summary">
      <div class="bml-metric">
        <span class="bml-metric-val">{{ store.localMusicSongs.length }}</span>
        <span class="bml-metric-label">已索引本地曲目</span>
      </div>
      <div class="bml-metric">
        <span class="bml-metric-val">{{ losslessCount }}</span>
        <span class="bml-metric-label">无损 / Hi-Res</span>
      </div>
      <div class="bml-metric">
        <span class="bml-metric-val">{{ managedDirs.length }}</span>
        <span class="bml-metric-label">已管理目录</span>
      </div>
      <div class="bml-hero-actions">
        <button
          type="button"
          class="bms-btn primary"
          :disabled="store.isScanningLocalMusic"
          @click="store.scanLocalMusicLibrary()"
        >
          <PlumpIcon name="refresh" :size="14" color="#ffffff" />
          <span>{{ store.isScanningLocalMusic ? "正在扫描…" : "立即扫描" }}</span>
        </button>
        <button type="button" class="bms-btn" @click="goLocalMusicView">
          <PlumpIcon name="music" :size="14" />
          <span>前往本地音乐</span>
        </button>
      </div>
    </div>

    <!-- 卡片 1：本地音乐文件夹多目录管理 -->
    <SettingsCard
      icon="folder"
      icon-color="var(--mint)"
      title="扫描目录管理"
      subtitle="支持添加多个本地音乐目录并在目录间快速切换，与「本地音乐」页面实时同步"
    >
      <template #actions>
        <button type="button" class="bms-btn sm primary" @click="store.pickAndScanLocalMusicDir()">
          <PlumpIcon name="folder" :size="13" color="#ffffff" />
          <span>添加扫描目录</span>
        </button>
      </template>

      <div
        class="bml-dir-list"
        data-setting-id="local-music-dirs"
        :class="{ 'is-flash-highlight': highlightedId === 'local-music-dirs' }"
      >
        <div
          v-for="dir in managedDirs"
          :key="dir"
          class="bml-dir-item"
          :class="{ 'is-active': isDirActive(dir) }"
        >
          <div class="bml-dir-info">
            <PlumpIcon
              name="folder"
              :size="16"
              :color="isDirActive(dir) ? 'var(--primary)' : 'var(--text-muted)'"
            />
            <div class="bml-dir-text">
              <div class="bml-dir-path-line">
                <span class="bml-dir-path">{{ dir }}</span>
                <span v-if="isDirActive(dir)" class="bms-chip mint">当前扫描</span>
              </div>
            </div>
          </div>

          <div class="bml-dir-actions">
            <button
              v-if="!isDirActive(dir)"
              type="button"
              class="bms-btn sm"
              :disabled="store.isScanningLocalMusic"
              @click="handleSwitchDir(dir)"
            >
              切换扫描
            </button>
            <button
              type="button"
              class="bms-btn sm ghost"
              @click="store.openDirectoryInExplorer(dir)"
            >
              打开
            </button>
            <button
              v-if="!isDirActive(dir)"
              type="button"
              class="bms-btn sm danger"
              aria-label="移除该扫描目录"
              @click="store.removeLocalMusicSavedDir(dir)"
            >
              移除
            </button>
          </div>
        </div>
      </div>
    </SettingsCard>

    <!-- 卡片 2：扫描过滤与元数据解析规则 -->
    <SettingsCard
      icon="filter"
      icon-color="var(--cyan)"
      title="扫描过滤与元数据规则"
      subtitle="过滤提示音、语音备忘录与重复文件，精细控制标签读取优先级"
    >
      <SettingsRow
        setting-id="local-music-rules"
        :highlighted="highlightedId === 'local-music-rules'"
        title="启动时自动后台扫描本地曲库"
        description="冷启动进入应用后静默刷新已选目录，保持本地曲库随时最新"
      >
        <SettingsSwitch
          :model-value="store.localMusicSettings.autoScanOnStartup"
          aria-label="启动时自动后台扫描本地曲库"
          @update:model-value="
            (v) => store.updateLocalMusicSettings({ autoScanOnStartup: v })
          "
        />
      </SettingsRow>

      <SettingsRow
        title="递归扫描子文件夹"
        description="开启后将自动深入专辑与歌手子目录检索全部音频文件"
      >
        <SettingsSwitch
          :model-value="store.localMusicSettings.scanSubfolders"
          aria-label="递归扫描子文件夹"
          @update:model-value="(v) => updateRuleAndHint({ scanSubfolders: v })"
        />
      </SettingsRow>

      <SettingsRow
        title="忽略过短音频片段"
        description="自动过滤铃声、音效与提示音等时长过短的文件"
      >
        <div class="bms-pill-group">
          <button
            v-for="opt in minDurationOptions"
            :key="opt.value"
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.localMusicSettings.minDurationSec === opt.value }"
            @click="updateRuleAndHint({ minDurationSec: opt.value })"
          >
            {{ opt.label }}
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="忽略过小体积文件"
        description="跳过损坏空文件或极小体积录音片段"
      >
        <div class="bms-pill-group">
          <button
            v-for="opt in minFileSizeOptions"
            :key="opt.value"
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.localMusicSettings.minFileSizeKb === opt.value }"
            @click="updateRuleAndHint({ minFileSizeKb: opt.value })"
          >
            {{ opt.label }}
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="自动过滤重复音轨"
        description="按歌名、歌手与近似时长智能去重，避免相同曲目反复出现"
      >
        <SettingsSwitch
          :model-value="store.localMusicSettings.deduplicate"
          aria-label="自动过滤重复音轨"
          @update:model-value="(v) => updateRuleAndHint({ deduplicate: v })"
        />
      </SettingsRow>

      <SettingsRow
        title="元数据识别优先级"
        description="决定优先读取音频内嵌 ID3/Vorbis 标签还是直接按文件名解析"
      >
        <div class="bms-pill-group">
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.localMusicSettings.preferEmbeddedTags }"
            @click="updateRuleAndHint({ preferEmbeddedTags: true })"
          >
            内嵌标签优先
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: !store.localMusicSettings.preferEmbeddedTags }"
            @click="updateRuleAndHint({ preferEmbeddedTags: false })"
          >
            文件名解析优先
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="重建元数据与内嵌封面缓存"
        description="若在外部工具修改过本地音频标签或封面，可一键强制重扫刷新缓存"
      >
        <button
          type="button"
          class="bms-btn"
          :disabled="store.isScanningLocalMusic"
          @click="store.rebuildLocalMusicCache()"
        >
          <PlumpIcon name="reset-rotate" :size="14" />
          <span>重建缓存</span>
        </button>
      </SettingsRow>
    </SettingsCard>
  </div>
</template>

<style scoped>
.bml-hero-summary {
  border-radius: 20px;
  padding: 18px 22px;
  background: var(--bms-surface-panel);
  box-shadow: var(--bms-panel-shadow);
  backdrop-filter: blur(22px) saturate(155%);
  -webkit-backdrop-filter: blur(22px) saturate(155%);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  flex-wrap: wrap;
}

.bml-metric {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 96px;
}

.bml-metric-val {
  font-size: var(--bm-fs-title, 20px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.bml-metric-label {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.bml-hero-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.bml-dir-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  border-radius: var(--bm-r-lg, 18px);
}

.bml-dir-list.is-flash-highlight {
  box-shadow: 0 0 28px -2px color-mix(in srgb, var(--primary) 50%, transparent);
}

.bml-dir-item {
  border-radius: var(--bm-r-lg, 18px);
  padding: 13px 16px;
  background: var(--bms-surface-row);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.bml-dir-item.is-active {
  background: color-mix(in srgb, var(--primary) 12%, var(--bms-surface-row));
}

.bml-dir-info {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 11px;
}

.bml-dir-text {
  min-width: 0;
  flex: 1;
}

.bml-dir-path-line {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bml-dir-path {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  color: var(--text-primary);
  word-break: break-all;
}

.bml-dir-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

/* L2 晨雾燕麦 / D2 丝绒星云特化 */
.scheme-watercolor_light.light-variant-nordic_oat .bml-hero-summary,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bml-hero-summary {
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}
</style>
