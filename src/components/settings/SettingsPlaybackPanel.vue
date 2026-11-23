<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useAppStore } from "../../stores/appStore";
import type { ConceptAudioQualityValue, PlayMode } from "../../types";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();
const isRefreshingDevices = ref(false);

onMounted(() => {
  store.refreshAudioOutputDevices();
});

const qualityOptions: ReadonlyArray<{
  id: ConceptAudioQualityValue;
  badge: string;
  title: string;
  spec: string;
  tone: "gold" | "mint" | "primary" | "muted";
}> = [
  {
    id: "high",
    badge: "Hi-Res",
    title: "Hi-Res 高解析无损",
    spec: "最高 24-bit / 96–192kHz · 母带细节解析",
    tone: "gold",
  },
  {
    id: "flac",
    badge: "SQ",
    title: "SQ 超品无损 FLAC",
    spec: "16-bit / 44.1–48kHz · 录音室 CD 无损",
    tone: "mint",
  },
  {
    id: "320",
    badge: "HQ",
    title: "HQ 高品 320K",
    spec: "320kbps MP3 · 均衡饱满且加载迅捷",
    tone: "primary",
  },
  {
    id: "128",
    badge: "标准",
    title: "标准流畅 128K",
    spec: "128kbps · 极低带宽占用与秒开起播",
    tone: "muted",
  },
];

const playModeOptions: ReadonlyArray<{
  id: PlayMode;
  label: string;
  icon: string;
}> = [
  { id: "repeat_all", label: "列表循环", icon: "repeat" },
  { id: "repeat_one", label: "单曲循环", icon: "repeat-one" },
  { id: "shuffle", label: "随机洗牌", icon: "shuffle" },
  { id: "sequence", label: "顺序播放", icon: "sequence" },
];

const normalizedPrefQuality = computed<ConceptAudioQualityValue>(() => {
  const q = store.preferredQuality;
  if (q === "128" || q === "320" || q === "flac" || q === "high") return q;
  return "high";
});

const replayGainTelemetryText = computed(() => {
  if (!store.replayGainEnabled) return "已关闭 · 原声增益直通";
  const db = Number(store.effectiveReplayGainDb || 0);
  const sign = db >= 0 ? "+" : "";
  return `生效中 · 当前动态补偿 ${sign}${db.toFixed(1)} dB`;
});

const btDelayText = computed(() => {
  const ms = store.bluetoothDelayMs || 0;
  if (ms === 0) return "0 ms (无偏移)";
  return ms > 0 ? `+${ms} ms` : `${ms} ms`;
});

async function handleSelectQuality(q: ConceptAudioQualityValue) {
  await store.switchAudioQuality(q);
}

async function handleRefreshDevices() {
  if (isRefreshingDevices.value) return;
  isRefreshingDevices.value = true;
  try {
    await store.refreshAudioOutputDevices();
    store.notify(`已刷新音频输出设备列表（共 ${store.audioOutputDevices.length} 项）`);
  } finally {
    isRefreshingDevices.value = false;
  }
}

function handleDeviceChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value;
  store.selectAudioOutputDevice(val);
}

function setPlayMode(mode: PlayMode) {
  store.setPlayMode(mode, false);
}

const safeVolumePercent = computed({
  get: () => Math.round((store.startupSafeVolumeLimit || 0.5) * 100),
  set: (pct: number) => {
    store.startupSafeVolumeLimit = Number((Math.max(10, Math.min(90, pct)) / 100).toFixed(2));
  },
});

function stepBluetoothDelay(delta: number) {
  store.bluetoothDelayMs = Math.max(-500, Math.min(1000, store.bluetoothDelayMs + delta));
}

function goToTuner() {
  store.isNowPlayingOpen = false;
  store.activeTab = "tuner";
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 1. 在线默认音质 -->
    <SettingsCard
      title="默认在线流媒体音质"
      subtitle="酷狗概念版优先按以下目标音质解析直链，若单曲无该档位则自动向下无损兼容"
      icon="quality"
      icon-color="var(--gold)"
      setting-id="playback-quality"
    >
      <div
        class="bms-tile-grid cols-4"
        :class="{ 'is-flash-highlight': highlightedId === 'playback-quality' }"
        role="radiogroup"
        aria-label="默认在线流媒体音质"
      >
        <button
          v-for="q in qualityOptions"
          :key="q.id"
          type="button"
          role="radio"
          class="bms-tile-btn"
          :class="{ 'is-active': normalizedPrefQuality === q.id }"
          :aria-checked="normalizedPrefQuality === q.id"
          @click="handleSelectQuality(q.id)"
        >
          <div class="bms-q-head">
            <span class="bms-chip" :class="q.tone === 'primary' ? '' : q.tone">
              {{ q.badge }}
            </span>
            <span
              v-if="normalizedPrefQuality === q.id"
              class="bms-tile-check"
              aria-hidden="true"
            >
              <PlumpIcon name="check" :size="10" color="#ffffff" />
            </span>
          </div>
          <div class="bms-q-title">{{ q.title }}</div>
          <div class="bms-q-spec">{{ q.spec }}</div>
        </button>
      </div>
    </SettingsCard>

    <!-- 2. 音频输出设备（声卡 / 耳机） -->
    <SettingsCard
      title="音频输出通道"
      subtitle="基于 WebView2 硬件级 setSinkId 路由主音频轨、交叉淡变副轨与 WebAudio DSP 引擎"
      icon="headphones"
      icon-color="var(--cyan)"
    >
      <SettingsRow
        title="音频输出设备（声卡 / 外置 DAC / 耳机）"
        description="插拔 USB 声卡或蓝牙耳机后系统会自动监听变更，也可手动刷新枚举列表"
        icon="volume"
        icon-color="var(--cyan)"
        label-for="bms-audio-device-select"
        setting-id="playback-output-device"
        :highlighted="highlightedId === 'playback-output-device'"
      >
        <select
          id="bms-audio-device-select"
          name="audio-output-device"
          class="bms-select"
          :value="store.selectedAudioDeviceId"
          @change="handleDeviceChange"
        >
          <option
            v-for="dev in store.audioOutputDevices"
            :key="dev.deviceId"
            :value="dev.deviceId"
          >
            {{ dev.label }}
          </option>
        </select>
        <button
          type="button"
          class="bms-btn"
          :disabled="isRefreshingDevices"
          @click="handleRefreshDevices"
        >
          <PlumpIcon name="refresh" :size="14" />
          <span>刷新设备</span>
        </button>
      </SettingsRow>
    </SettingsCard>

    <!-- 3. 淡入淡出、响度均衡与蓝牙同步 -->
    <SettingsCard
      title="听感平滑与声学补偿"
      subtitle="双 Deck 重叠无缝衔接、跨曲响度拉平与无线蓝牙耳机声画延迟校准"
      icon="wave"
      icon-color="var(--mint)"
    >
      <SettingsRow
        title="跨曲无缝交叉淡入淡出 (Crossfade)"
        description="切歌时让下一曲提前平滑混入，设为 0 秒则保持曲目原样边界衔接"
        icon="wave"
        icon-color="var(--primary)"
        label-for="bms-crossfade-range"
        setting-id="playback-crossfade"
        :highlighted="highlightedId === 'playback-crossfade'"
      >
        <div class="bms-slider-wrap">
          <input
            id="bms-crossfade-range"
            v-model.number="store.crossfadeSeconds"
            type="range"
            min="0"
            max="8"
            step="1"
            class="bms-range"
          />
          <span class="bms-range-val">
            {{ store.crossfadeSeconds === 0 ? "关闭" : `${store.crossfadeSeconds} 秒` }}
          </span>
        </div>
      </SettingsRow>

      <SettingsRow
        title="播放 / 暂停防爆音平滑淡变"
        description="暂停与继续播放时施加毫秒级音量斜坡包络，避免高灵敏度耳机瞬态爆音"
        icon="play"
        icon-color="var(--mint)"
        label-for="bms-sw-play-fade"
        setting-id="playback-play-pause-fade"
        :highlighted="highlightedId === 'playback-play-pause-fade'"
      >
        <SettingsSwitch
          id="bms-sw-play-fade"
          v-model="store.playPauseFadeEnabled"
          aria-label="播放暂停防爆音平滑淡变"
        />
      </SettingsRow>

      <SettingsRow
        title="ReplayGain 跨曲响度均衡"
        :description="replayGainTelemetryText"
        icon="equalizer"
        icon-color="var(--cyan)"
        label-for="bms-sw-replaygain"
        setting-id="playback-replay-gain"
        :highlighted="highlightedId === 'playback-replay-gain'"
      >
        <SettingsSwitch
          id="bms-sw-replaygain"
          v-model="store.replayGainEnabled"
          aria-label="ReplayGain 跨曲响度均衡"
        />
      </SettingsRow>

      <SettingsRow
        title="夜间小音量等响度动态补偿"
        description="依据 Fletcher-Munson 人耳听感曲线，低音量聆听时自动补足低频下潜与高频空气感"
        icon="moon"
        icon-color="var(--violet)"
        label-for="bms-sw-night-loud"
        setting-id="playback-night-loudness"
        :highlighted="highlightedId === 'playback-night-loudness'"
      >
        <SettingsSwitch
          id="bms-sw-night-loud"
          v-model="store.nightLoudnessEnabled"
          aria-label="夜间小音量等响度动态补偿"
        />
      </SettingsRow>

      <SettingsRow
        title="蓝牙耳机音频延迟校准"
        description="使用蓝牙 TWS 耳机存在编码延迟时，同步偏移播放页与桌面逐字歌词时间轴"
        icon="clock"
        icon-color="var(--gold)"
        :badge="btDelayText"
        badge-tone="gold"
        setting-id="playback-bluetooth-delay"
        :highlighted="highlightedId === 'playback-bluetooth-delay'"
        vertical
      >
        <div class="bms-bt-delay-bar">
          <button type="button" class="bms-btn sm" @click="stepBluetoothDelay(-50)">-50ms</button>
          <input
            v-model.number="store.bluetoothDelayMs"
            type="range"
            min="-500"
            max="1000"
            step="25"
            aria-label="蓝牙耳机音频延迟校准毫秒数"
            class="bms-range"
          />
          <button type="button" class="bms-btn sm" @click="stepBluetoothDelay(50)">+50ms</button>
          <button
            type="button"
            class="bms-btn sm ghost"
            :disabled="store.bluetoothDelayMs === 0"
            @click="store.bluetoothDelayMs = 0"
          >
            归零
          </button>
        </div>
      </SettingsRow>
    </SettingsCard>

    <!-- 4. 播控策略与调音台联动 -->
    <SettingsCard
      title="播放策略与母带调音台联动"
      subtitle="配置队列循环方式、副歌速刷，并快速直达 64-bit 风格 WebAudio 声学调音台"
      icon="tuning"
      icon-color="var(--coral)"
    >
      <SettingsRow
        title="默认队列播放模式"
        description="真随机洗牌采用 Fisher-Yates 洗牌池，保障切回上一首按真实历史栈还原"
        icon="repeat"
        icon-color="var(--primary)"
        setting-id="playback-behavior"
        :highlighted="highlightedId === 'playback-behavior'"
      >
        <div class="bms-pill-group" role="radiogroup" aria-label="默认队列播放模式">
          <button
            v-for="m in playModeOptions"
            :key="m.id"
            type="button"
            role="radio"
            class="bms-pill-opt"
            :class="{ active: store.playMode === m.id }"
            :aria-checked="store.playMode === m.id"
            @click="setPlayMode(m.id)"
          >
            <PlumpIcon :name="m.icon" :size="13" color="currentColor" />
            <span>{{ m.label }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="启动时自动继续播放"
        description="软件启动时默认静默记忆上次播放队列与进度；开启后将在启动时自动从上次时间点继续出声播放"
        icon="play"
        icon-color="var(--primary)"
        :badge="store.autoPlayOnStartup ? '启动即续播' : '仅静默记忆'"
        :badge-tone="store.autoPlayOnStartup ? 'mint' : 'muted'"
        label-for="bms-sw-auto-play-startup"
        setting-id="playback-auto-play-startup"
        :highlighted="highlightedId === 'playback-auto-play-startup'"
      >
        <SettingsSwitch
          id="bms-sw-auto-play-startup"
          v-model="store.autoPlayOnStartup"
          aria-label="启动时自动继续播放"
        />
      </SettingsRow>

      <SettingsRow
        title="切歌时自动恢复 1.0× 标准倍速"
        description="开启后每次切换下一曲自动重置播放速率，避免加速听播客或练歌后忘记调回原速"
        icon="reset-rotate"
        icon-color="var(--cyan)"
        label-for="bms-sw-reset-speed"
      >
        <SettingsSwitch
          id="bms-sw-reset-speed"
          v-model="store.resetPlaybackRateOnTrackChange"
          aria-label="切歌时自动恢复标准倍速"
        />
      </SettingsRow>

      <SettingsRow
        title="启动安全音量上限（防炸耳保护）"
        :description="
          store.startupSafeVolumeEnabled
            ? `每次启动时若上次音量高于 ${safeVolumePercent}%，将自动温和回落至 ${safeVolumePercent}%`
            : '开启后可在软件启动时自动限制过高音量，保护佩戴耳机时的听力安全'
        "
        icon="shield"
        icon-color="var(--mint)"
        :badge="store.startupSafeVolumeEnabled ? `上限 ${safeVolumePercent}%` : '已关闭'"
        :badge-tone="store.startupSafeVolumeEnabled ? 'mint' : 'muted'"
        vertical
      >
        <div class="bms-safe-vol-bar">
          <SettingsSwitch
            id="bms-sw-safe-vol"
            v-model="store.startupSafeVolumeEnabled"
            aria-label="启动安全音量上限保护"
          />
          <div v-if="store.startupSafeVolumeEnabled" class="bms-slider-wrap">
            <input
              v-model.number="safeVolumePercent"
              type="range"
              min="10"
              max="90"
              step="5"
              class="bms-range"
              aria-label="启动安全音量上限百分比"
            />
            <span class="bms-range-val">{{ safeVolumePercent }}%</span>
          </div>
        </div>
      </SettingsRow>

      <SettingsRow
        title="副歌速刷模式 (Climax 30s)"
        description="切歌时自动跳至高潮副歌片段试听，适合在榜单或新歌库快速淘歌"
        icon="climax"
        icon-color="var(--coral)"
        label-for="bms-sw-climax"
      >
        <SettingsSwitch
          id="bms-sw-climax"
          v-model="store.climaxModeEnabled"
          aria-label="副歌速刷模式"
        />
      </SettingsRow>

      <SettingsRow
        title="母带声学调音台 & 蝰蛇音效引擎"
        :description="
          store.masterEffectEnabled
            ? '当前总开关已开启 · 含 10/15/31 段参量均衡器、Bauer 交叉馈送与卷积混响'
            : '当前处于原声直通态 · 可一键进入调音台精修频响曲线与声场'
        "
        icon="power-dsp"
        icon-color="var(--mint)"
        :badge="store.masterEffectEnabled ? 'DSP 已开启' : '原声直通'"
        :badge-tone="store.masterEffectEnabled ? 'mint' : 'muted'"
      >
        <SettingsSwitch
          :model-value="store.masterEffectEnabled"
          aria-label="母带 DSP 总开关"
          @update:model-value="store.toggleMasterEffect()"
        />
        <button type="button" class="bms-btn" @click="goToTuner">
          <span>前往调音台</span>
          <PlumpIcon name="chevron-right" :size="13" />
        </button>
      </SettingsRow>
    </SettingsCard>
  </div>
</template>

<style scoped>
.bms-q-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.bms-q-title {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bms-q-spec {
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: var(--bm-ui-line-height, 1.48);
  color: var(--text-secondary);
}

.bms-bt-delay-bar,
.bms-safe-vol-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  flex-wrap: wrap;
}
</style>
