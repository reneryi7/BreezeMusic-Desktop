<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import {
  FOCUS_SCENE_PRESETS,
  type FocusModeType,
  type FocusScenePreset,
} from "../stores/modules/useFocusPodModule";
import { AMBIENT_CHANNELS, type AmbientChannelId } from "../utils/ambientSoundEngine";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();

const customMinutesInput = ref<number>(store.focusTargetMinutes || 45);

watch(
  () => store.focusTargetMinutes,
  (val) => {
    if (val > 0) customMinutesInput.value = val;
  }
);

const ringRadius = 104;
const ringCircumference = 2 * Math.PI * ringRadius;

const ringDashoffset = computed(() => {
  if (store.focusMode === "stopwatch") {
    const cycle = (store.focusElapsedSec % 60) / 60;
    return ringCircumference * (1 - Math.max(0.08, cycle));
  }
  const ratio = Math.min(1, Math.max(0, store.focusProgressRatio));
  return ringCircumference * (1 - ratio);
});

const statusBadgeText = computed(() => {
  if (store.focusRunState === "completed") return "本轮心流已完成";
  if (store.focusRunState === "paused") return "已暂停专注";
  if (store.focusMode === "pomodoro") {
    if (store.pomodoroPhase === "short_break") return "番茄短休恢复";
    if (store.pomodoroPhase === "long_break") return "番茄深度长休";
    return store.focusRunState === "running" ? "番茄深度专注中" : "番茄专注就绪";
  }
  if (store.focusMode === "stopwatch") {
    return store.focusRunState === "running" ? "自由计时进行中" : "自由正计时就绪";
  }
  return store.focusRunState === "running" ? "沉浸倒计时中" : "倒计时就绪";
});

const primaryActionLabel = computed(() => {
  if (store.focusRunState === "running") return "暂停";
  if (store.focusRunState === "paused") return "继续专注";
  if (store.focusRunState === "completed") return "开启新一轮";
  return "开始专注";
});

const activeAmbientCount = computed(
  () =>
    AMBIENT_CHANNELS.filter(
      (ch) => store.ambientEngine.state.channels[ch.id]?.enabled
    ).length
);

function getSceneIcon(presetId: string): string {
  switch (presetId) {
    case "pomodoro_25":
      return "timer";
    case "rain_study":
      return "cloud";
    case "retro_vinyl":
      return "vinyl";
    case "camp_night":
      return "flame";
    case "deep_ocean":
      return "wave";
    case "cafe_flow":
      return "headphones";
    case "night_sleep":
      return "moon";
    case "free_stopwatch":
      return "clock";
    default:
      return "sparkles";
  }
}

function getAmbientIcon(id: AmbientChannelId): string {
  switch (id) {
    case "rain":
      return "cloud";
    case "vinyl":
      return "vinyl";
    case "campfire":
      return "flame";
    case "ocean":
      return "wave";
  }
}

function getTagLabel(tag: FocusScenePreset["tag"]): string {
  switch (tag) {
    case "STUDY":
      return "学习";
    case "CAFE":
      return "灵感";
    case "HEALING":
      return "疗愈";
    case "SLEEP":
      return "助眠";
    case "WORKOUT":
      return "律动";
    case "DRIVE":
      return "巡航";
  }
}

function applyCustomMinutes() {
  const num = Number(customMinutesInput.value);
  if (!Number.isFinite(num)) return;
  const clamped = Math.min(180, Math.max(5, Math.round(num)));
  customMinutesInput.value = clamped;
  store.setCustomCountdownMinutes(clamped);
}

function stepPomodoroFocus(delta: number) {
  const next = Math.min(60, Math.max(10, store.pomodoroFocusMinutes + delta));
  store.pomodoroFocusMinutes = next;
  if (store.focusRunState === "idle" && store.pomodoroPhase === "focus") {
    store.focusRemainingSec = next * 60;
  }
}

function stepPomodoroBreak(delta: number) {
  const next = Math.min(20, Math.max(3, store.pomodoroShortBreakMinutes + delta));
  store.pomodoroShortBreakMinutes = next;
  if (store.focusRunState === "idle" && store.pomodoroPhase === "short_break") {
    store.focusRemainingSec = next * 60;
  }
}

function handleModeClick(mode: FocusModeType) {
  store.switchFocusMode(mode);
}

function handleSceneSelect(preset: FocusScenePreset) {
  store.applyFocusScenePreset(preset.id);
}

function handlePlaySceneMusicNow(preset: FocusScenePreset) {
  store.triggerSceneMusicPlayback(preset);
}

function handleKeydown(ev: KeyboardEvent) {
  if (ev.key === "Escape" && store.isFocusPodOpen) {
    store.isFocusPodOpen = false;
  }
}

watch(
  () => store.isFocusPodOpen,
  (open) => {
    if (typeof window === "undefined") return;
    if (open) {
      window.addEventListener("keydown", handleKeydown);
    } else {
      window.removeEventListener("keydown", handleKeydown);
    }
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("keydown", handleKeydown);
  }
});
</script>

<template>
  <transition name="fade">
    <div
      v-if="store.isFocusPodOpen"
      class="fp-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label="桌面心流舱 Focus Studio"
      @click.self="store.isFocusPodOpen = false"
    >
      <div class="fp-studio-shell">
        <!-- 顶部工作室抬头 -->
        <header class="fp-header">
          <div class="fp-brand">
            <div class="fp-brand-badge">
              <span
                class="fp-live-dot"
                :class="{
                  running: store.focusRunState === 'running',
                  sound: activeAmbientCount > 0 && store.ambientEngine.state.masterEnabled,
                }"
              ></span>
              <span>FOCUS STUDIO · 桌面心流舱</span>
            </div>
            <h2 class="fp-brand-title">
              {{ store.activeFocusPreset?.title || "沉浸专注舱" }}
            </h2>
          </div>

          <div class="fp-header-right">
            <div
              v-if="store.currentSong"
              class="fp-now-playing-pill"
              title="当前联动曲目（点击跳转播放页）"
              @click="
                store.isFocusPodOpen = false;
                store.isNowPlayingOpen = true;
              "
            >
              <img
                v-if="store.currentSong.coverUrl"
                :src="store.currentSong.coverUrl"
                class="fp-np-cover"
                alt=""
              />
              <span v-else class="fp-np-cover fp-np-fallback">
                <PlumpIcon name="music" :size="13" />
              </span>
              <span class="fp-np-copy">
                <span class="fp-np-title">{{ store.currentSong.title }}</span>
                <span class="fp-np-artist">{{ store.currentSong.artist }}</span>
              </span>
              <button
                type="button"
                class="fp-np-toggle"
                :title="store.isPlaying ? '暂停音乐' : '恢复播放'"
                @click.stop="store.togglePlayPause()"
              >
                <PlumpIcon :name="store.isPlaying ? 'pause' : 'play'" :size="12" />
              </button>
            </div>

            <button
              type="button"
              class="fp-close-btn"
              title="收起心流舱（计时器与白噪音继续在后台运行）"
              aria-label="关闭心流舱"
              @click="store.isFocusPodOpen = false"
            >
              <PlumpIcon name="close" :size="16" />
            </button>
          </div>
        </header>

        <!-- 双栏主体 -->
        <div class="fp-columns">
          <!-- 左栏：定时仪与番茄循环控制台 -->
          <section class="fp-timer-pane">
            <!-- 三模式切换器 -->
            <div class="fp-mode-tabs" role="tablist" aria-label="计时模式">
              <button
                v-for="modeItem in [
                  { id: 'pomodoro', label: '番茄钟' },
                  { id: 'countdown', label: '倒计时' },
                  { id: 'stopwatch', label: '正计时' },
                ]"
                :key="modeItem.id"
                type="button"
                role="tab"
                :aria-selected="store.focusMode === modeItem.id"
                class="fp-mode-pill"
                :class="{ active: store.focusMode === modeItem.id }"
                @click="handleModeClick(modeItem.id as FocusModeType)"
              >
                {{ modeItem.label }}
              </button>
            </div>

            <!-- 环形进度表盘 -->
            <div
              class="fp-orb-stage"
              :class="{
                'is-running': store.focusRunState === 'running',
                'is-break': store.focusMode === 'pomodoro' && store.pomodoroPhase !== 'focus',
              }"
            >
              <div
                class="fp-orb-ambient-glow"
                :class="{ 'focus-breathe-aura': store.focusRunState === 'running' }"
              ></div>
              <svg class="fp-orb-svg" viewBox="0 0 240 240">
                <circle class="fp-orb-track" cx="120" cy="120" :r="ringRadius" />
                <circle
                  class="fp-orb-progress"
                  cx="120"
                  cy="120"
                  :r="ringRadius"
                  :stroke-dasharray="ringCircumference"
                  :stroke-dashoffset="ringDashoffset"
                />
              </svg>

              <div class="fp-orb-center">
                <span class="fp-status-tag">{{ statusBadgeText }}</span>
                <div class="fp-clock-display">{{ store.focusFormattedClock }}</div>
                <span class="fp-clock-sub">
                  <template v-if="store.focusMode === 'stopwatch'">
                    自由累计心流 · 每满 1 分钟写入统计
                  </template>
                  <template v-else-if="store.focusMode === 'pomodoro'">
                    {{ store.pomodoroFocusMinutes }} 分专注 / {{ store.pomodoroShortBreakMinutes }} 分小憩
                  </template>
                  <template v-else>
                    设定目标 {{ store.focusTargetMinutes }} 分钟
                  </template>
                </span>
              </div>
            </div>

            <!-- 模式专属控制：番茄轮次 / 自定义时长 -->
            <div v-if="store.focusMode === 'pomodoro'" class="fp-mode-detail-card">
              <div class="fp-pomo-rounds-row">
                <div class="fp-pomo-dots">
                  <span
                    v-for="idx in 4"
                    :key="idx"
                    class="fp-pomo-dot"
                    :class="{
                      filled:
                        idx <=
                        (store.pomodoroCompletedRounds % 4 === 0 && store.pomodoroCompletedRounds > 0 && store.pomodoroPhase === 'long_break'
                          ? 4
                          : store.pomodoroCompletedRounds % 4),
                    }"
                  ></span>
                  <span class="fp-pomo-round-text">
                    已完成 {{ store.pomodoroCompletedRounds }} 轮
                  </span>
                </div>

                <button
                  type="button"
                  class="fp-micro-pill"
                  @click="store.skipPomodoroPhase()"
                >
                  跳过当前阶段
                </button>
              </div>

              <div class="fp-pomo-steppers">
                <div class="fp-stepper-item">
                  <span>专注</span>
                  <div class="fp-stepper-ctrl">
                    <button type="button" @click="stepPomodoroFocus(-5)">-</button>
                    <strong>{{ store.pomodoroFocusMinutes }}m</strong>
                    <button type="button" @click="stepPomodoroFocus(5)">+</button>
                  </div>
                </div>
                <div class="fp-stepper-item">
                  <span>短休</span>
                  <div class="fp-stepper-ctrl">
                    <button type="button" @click="stepPomodoroBreak(-1)">-</button>
                    <strong>{{ store.pomodoroShortBreakMinutes }}m</strong>
                    <button type="button" @click="stepPomodoroBreak(1)">+</button>
                  </div>
                </div>
              </div>
            </div>

            <div v-else-if="store.focusMode === 'countdown'" class="fp-mode-detail-card">
              <div class="fp-duration-presets">
                <button
                  v-for="mins in [15, 25, 45, 60, 90]"
                  :key="mins"
                  type="button"
                  class="fp-dur-pill"
                  :class="{ active: store.focusTargetMinutes === mins }"
                  @click="store.setCustomCountdownMinutes(mins)"
                >
                  {{ mins }} 分
                </button>
              </div>

              <div class="fp-custom-dur-row">
                <span class="fp-custom-label">自定义时长 (5–180 分)</span>
                <div class="fp-custom-input-wrap">
                  <input
                    v-model.number="customMinutesInput"
                    type="number"
                    min="5"
                    max="180"
                    class="fp-custom-input"
                    @keyup.enter="applyCustomMinutes"
                  />
                  <button
                    type="button"
                    class="fp-micro-pill"
                    @click="applyCustomMinutes"
                  >
                    应用
                  </button>
                </div>
              </div>
            </div>

            <div v-else class="fp-mode-detail-card fp-stopwatch-note">
              <PlumpIcon name="clock" :size="16" color="var(--primary)" />
              <span>正计时无时间压力；累计满 60 秒后点击下方「结束并记录」即可归档本轮专注。</span>
            </div>

            <!-- 底部主控按钮组 -->
            <div class="fp-transport-row">
              <button
                type="button"
                class="fp-transport-primary"
                @click="store.toggleFocusRunPause()"
              >
                <transition name="bm-icon-morph" mode="out-in">
                  <PlumpIcon
                    :key="store.focusRunState === 'running' ? 'pause' : 'play'"
                    :name="store.focusRunState === 'running' ? 'pause' : 'play'"
                    :size="17"
                    color="#fff"
                  />
                </transition>
                <span>{{ primaryActionLabel }}</span>
              </button>

              <button
                type="button"
                class="fp-transport-secondary"
                @click="store.stopAndResetFocusSession(true)"
              >
                <PlumpIcon name="reset-rotate" :size="15" />
                <span>
                  {{
                    store.focusMode === "stopwatch" && store.focusElapsedSec >= 60
                      ? "结束并记录"
                      : "重置计时"
                  }}
                </span>
              </button>
            </div>
          </section>

          <!-- 右栏：8大场景预设 + 4通道WebAudio白噪音 + 心流数据统计 -->
          <section class="fp-studio-pane">
            <!-- 场景矩阵 -->
            <div class="fp-block">
              <div class="fp-block-head">
                <div class="fp-block-title-group">
                  <h3 class="fp-block-title">沉浸场景预设</h3>
                  <span class="fp-block-sub">一键联动电台音轨与白噪音配方</span>
                </div>

                <div class="fp-auto-toggles">
                  <button
                    type="button"
                    class="fp-switch-pill"
                    :class="{ active: store.autoPlaySceneMusic }"
                    @click="store.setAutoPlaySceneMusic(!store.autoPlaySceneMusic)"
                  >
                    <PlumpIcon name="music" :size="12" />
                    <span>切换场景播音乐</span>
                  </button>
                  <button
                    type="button"
                    class="fp-switch-pill"
                    :class="{ active: store.autoMixAmbient }"
                    @click="store.setAutoMixAmbient(!store.autoMixAmbient)"
                  >
                    <PlumpIcon name="sliders" :size="12" />
                    <span>自动配比环境音</span>
                  </button>
                </div>
              </div>

              <div class="fp-scene-grid">
                <div
                  v-for="preset in FOCUS_SCENE_PRESETS"
                  :key="preset.id"
                  class="fp-scene-card"
                  :class="{ active: store.focusPresetId === preset.id }"
                  @click="handleSceneSelect(preset)"
                >
                  <div class="fp-scene-top">
                    <span class="fp-scene-icon">
                      <PlumpIcon
                        :name="getSceneIcon(preset.id)"
                        :size="16"
                        :color="store.focusPresetId === preset.id ? 'var(--primary)' : 'currentColor'"
                      />
                    </span>
                    <div class="fp-scene-badges">
                      <span class="fp-scene-tag">{{ getTagLabel(preset.tag) }}</span>
                      <span class="fp-scene-dur">
                        {{ preset.mode === "stopwatch" ? "自由" : `${preset.minutes}m` }}
                      </span>
                    </div>
                  </div>

                  <div class="fp-scene-name">{{ preset.title }}</div>
                  <div class="fp-scene-desc" :title="preset.subtitle">
                    {{ preset.subtitle }}
                  </div>

                  <button
                    type="button"
                    class="fp-scene-music-btn"
                    title="立刻匹配并播放此场景电台/音乐"
                    @click.stop="handlePlaySceneMusicNow(preset)"
                  >
                    <PlumpIcon
                      :name="
                        store.isFetchingSceneMusic && store.focusPresetId === preset.id
                          ? 'refresh'
                          : 'play'
                      "
                      :size="11"
                      :class="{
                        'fp-spin':
                          store.isFetchingSceneMusic && store.focusPresetId === preset.id,
                      }"
                    />
                    <span>配乐</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- 4通道白噪音独立混音台 -->
            <div class="fp-block">
              <div class="fp-block-head">
                <div class="fp-block-title-group">
                  <h3 class="fp-block-title">Web Audio 四通道环境混音台</h3>
                  <span class="fp-block-sub">关闭心流舱窗口后仍可在后台独立发声</span>
                </div>

                <div class="fp-auto-toggles">
                  <button
                    type="button"
                    class="fp-switch-pill"
                    :class="{ active: store.ambientEngine.state.syncWithMusic }"
                    title="仅当播放器处于播放态时同步发出环境伴音"
                    @click="
                      store.ambientEngine.setSyncWithMusic(
                        !store.ambientEngine.state.syncWithMusic
                      )
                    "
                  >
                    <span>跟随音乐伴奏</span>
                  </button>

                  <button
                    type="button"
                    class="fp-switch-pill"
                    :class="{ active: store.ambientEngine.state.masterEnabled }"
                    @click="
                      store.ambientEngine.setMasterEnabled(
                        !store.ambientEngine.state.masterEnabled
                      )
                    "
                  >
                    <span>
                      {{ store.ambientEngine.state.masterEnabled ? "总声道开启" : "总声道静音" }}
                    </span>
                  </button>

                  <button
                    type="button"
                    class="fp-micro-pill"
                    :disabled="activeAmbientCount === 0"
                    @click="store.ambientEngine.silenceAll()"
                  >
                    全部关闭
                  </button>
                </div>
              </div>

              <div class="fp-ambient-grid">
                <div
                  v-for="ch in AMBIENT_CHANNELS"
                  :key="ch.id"
                  class="fp-ambient-card"
                  :class="{ active: store.ambientEngine.state.channels[ch.id]?.enabled }"
                  :style="{ '--ch-accent': ch.accentToken }"
                  @click="store.ambientEngine.toggleChannel(ch.id)"
                >
                  <div class="fp-amb-top">
                    <div class="fp-amb-icon">
                      <PlumpIcon
                        :name="getAmbientIcon(ch.id)"
                        :size="17"
                        :color="
                          store.ambientEngine.state.channels[ch.id]?.enabled
                            ? ch.accentToken
                            : 'currentColor'
                        "
                      />
                    </div>
                    <div class="fp-amb-copy">
                      <div class="fp-amb-name">{{ ch.name }}</div>
                      <div class="fp-amb-sub">{{ ch.subtitle }}</div>
                    </div>
                    <span class="fp-amb-pct">
                      {{
                        store.ambientEngine.state.channels[ch.id]?.enabled
                          ? `${Math.round(
                              (store.ambientEngine.state.channels[ch.id]?.volume || 0) * 100
                            )}%`
                          : "关闭"
                      }}
                    </span>
                  </div>

                  <div class="fp-amb-slider-row" @click.stop>
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.02"
                      class="fp-amb-slider"
                      :aria-label="`${ch.name}音量`"
                      :value="store.ambientEngine.state.channels[ch.id]?.volume ?? ch.defaultVolume"
                      :style="{
                        '--amb-fill': `${Math.round(
                          (store.ambientEngine.state.channels[ch.id]?.volume ??
                            ch.defaultVolume) * 100
                        )}%`,
                      }"
                      @input="
                        store.ambientEngine.setChannelVolume(
                          ch.id,
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                  </div>
                </div>
              </div>
            </div>

            <!-- 底部专注里程碑统计条 -->
            <div class="fp-stats-strip">
              <div class="fp-stat-item">
                <span class="fp-stat-val">{{ store.focusTodayCompletedCount }}</span>
                <span class="fp-stat-lbl">今日完成轮次</span>
              </div>
              <div class="fp-stat-item">
                <span class="fp-stat-val">{{ store.focusTodayTotalMinutes }} 分钟</span>
                <span class="fp-stat-lbl">今日专注时长</span>
              </div>
              <div class="fp-stat-item">
                <span class="fp-stat-val">{{ store.focusAllTimeCompletedCount }}</span>
                <span class="fp-stat-lbl">累计专注轮次</span>
              </div>
              <div class="fp-stat-item">
                <span class="fp-stat-val">{{ store.focusAllTimeTotalMinutes }} 分钟</span>
                <span class="fp-stat-lbl">累计专注总时长</span>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.fp-backdrop {
  position: fixed;
  inset: 0;
  z-index: 420;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

.fp-studio-shell {
  width: min(1060px, 95vw);
  max-height: min(820px, 92vh);
  border-radius: var(--bm-r-xl, 24px);
  padding: 18px 22px;
  background: linear-gradient(
    155deg,
    color-mix(in srgb, var(--primary) 11%, rgba(22, 23, 31, 0.94)),
    rgba(16, 17, 24, 0.96)
  );
  box-shadow: 0 32px 72px -18px rgba(0, 0, 0, 0.72);
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: hidden;
  color: var(--text-primary);
}

.fp-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
}

.fp-brand {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.fp-brand-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--primary);
}

.fp-live-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.28);
}

.fp-live-dot.sound {
  background: var(--cyan, var(--primary));
}

.fp-live-dot.running {
  background: var(--mint, var(--primary));
  box-shadow: 0 0 12px var(--mint, var(--primary));
}

.fp-brand-title {
  margin: 0;
  font-size: var(--bm-fs-title, 20px);
  font-weight: 800;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fp-header-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.fp-now-playing-pill {
  display: flex;
  align-items: center;
  gap: 9px;
  height: 40px;
  padding: 4px 8px 4px 5px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.07);
  max-width: 260px;
  cursor: pointer;
}

.fp-np-cover {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  object-fit: cover;
  flex-shrink: 0;
}

.fp-np-fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--primary) 24%, transparent);
  color: var(--primary);
}

.fp-np-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.2;
}

.fp-np-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fp-np-artist {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fp-np-toggle {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: none;
  background: var(--primary);
  color: #fff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}

.fp-close-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  border: none;
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}

.fp-close-btn:hover {
  background: rgba(255, 255, 255, 0.16);
  color: var(--text-primary);
}

.fp-columns {
  display: grid;
  grid-template-columns: minmax(292px, 330px) minmax(0, 1fr);
  gap: 16px;
  min-height: 0;
  overflow-y: auto;
  padding-right: 2px;
}

@media (max-width: 860px) {
  .fp-columns {
    grid-template-columns: 1fr;
  }
}

.fp-timer-pane {
  border-radius: var(--bm-r-lg, 18px);
  padding: 14px 16px;
  background: rgba(255, 255, 255, 0.035);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.fp-mode-tabs {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
  width: 100%;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.05);
}

.fp-mode-pill {
  height: 30px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}

.fp-mode-pill.active {
  background: var(--primary);
  color: #fff;
}

.fp-orb-stage {
  position: relative;
  width: 196px;
  height: 196px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.fp-orb-ambient-glow {
  position: absolute;
  inset: 18px;
  border-radius: 50%;
  background: radial-gradient(
    circle,
    color-mix(in srgb, var(--primary) 26%, transparent),
    transparent 70%
  );
  filter: blur(16px);
  opacity: 0.7;
  transition: opacity 0.3s ease, transform 0.3s ease;
}

.fp-orb-stage.is-running .fp-orb-ambient-glow {
  opacity: 1;
  animation: fpOrbPulse 3.6s ease-in-out infinite;
}

.fp-orb-stage.is-break .fp-orb-progress {
  stroke: var(--mint, var(--primary));
}

.fp-orb-svg {
  width: 196px;
  height: 196px;
  transform: rotate(-90deg);
}

.fp-orb-track {
  fill: none;
  stroke: rgba(255, 255, 255, 0.08);
  stroke-width: 8;
}

.fp-orb-progress {
  fill: none;
  stroke: var(--primary);
  stroke-width: 8;
  stroke-linecap: round;
  transition: stroke-dashoffset 0.35s linear, stroke 0.25s ease;
}

.fp-orb-center {
  position: absolute;
  inset: 22px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 4px;
}

.fp-status-tag {
  padding: 2px 9px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.07);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--text-secondary);
}

.fp-clock-display {
  font-size: clamp(32px, 3vw, 38px);
  font-weight: 800;
  letter-spacing: -0.02em;
  line-height: 1.08;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.fp-clock-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
}

.fp-mode-detail-card {
  width: 100%;
  border-radius: var(--bm-r-md, 14px);
  padding: 10px 11px;
  background: rgba(255, 255, 255, 0.045);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.fp-pomo-rounds-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.fp-pomo-dots {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.fp-pomo-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.16);
}

.fp-pomo-dot.filled {
  background: var(--primary);
}

.fp-pomo-round-text {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  margin-left: 2px;
}

.fp-pomo-steppers {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.fp-stepper-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: var(--bm-r-sm, 10px);
  background: rgba(255, 255, 255, 0.04);
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.fp-stepper-ctrl {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.fp-stepper-ctrl button {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: none;
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  cursor: pointer;
}

.fp-stepper-ctrl strong {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.fp-duration-presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.fp-dur-pill {
  flex: 1 1 auto;
  height: 28px;
  padding: 0 8px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  cursor: pointer;
}

.fp-dur-pill.active {
  background: var(--primary);
  color: #fff;
}

.fp-custom-dur-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.fp-custom-label {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
}

.fp-custom-input-wrap {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.fp-custom-input {
  width: 62px;
  height: 28px;
  padding: 0 8px;
  border: none;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  text-align: center;
  outline: none;
}

.fp-stopwatch-note {
  flex-direction: row;
  align-items: flex-start;
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: 1.45;
  color: var(--text-secondary);
}

.fp-transport-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
}

.fp-transport-primary {
  flex: 1.3;
  height: 38px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: var(--primary);
  color: #fff;
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  cursor: pointer;
  box-shadow: 0 12px 24px -10px color-mix(in srgb, var(--primary) 70%, transparent);
}

.fp-transport-secondary {
  flex: 1;
  height: 38px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  cursor: pointer;
}

.fp-studio-pane {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}

.fp-block {
  border-radius: var(--bm-r-lg, 18px);
  padding: 12px 14px;
  background: rgba(255, 255, 255, 0.035);
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.fp-block-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.fp-block-title-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.fp-block-title {
  margin: 0;
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  color: var(--text-primary);
}

.fp-block-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
}

.fp-auto-toggles {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.fp-switch-pill,
.fp-micro-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 11px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.fp-switch-pill.active {
  background: color-mix(in srgb, var(--primary) 24%, transparent);
  color: var(--primary);
}

.fp-micro-pill:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.fp-scene-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

@media (max-width: 980px) {
  .fp-scene-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

.fp-scene-card {
  position: relative;
  border-radius: var(--bm-r-md, 14px);
  padding: 9px 10px 30px;
  background: rgba(255, 255, 255, 0.045);
  display: flex;
  flex-direction: column;
  gap: 3px;
  cursor: pointer;
  min-width: 0;
  transition: transform 0.16s ease, background 0.16s ease;
}

.fp-scene-card:hover {
  background: rgba(255, 255, 255, 0.085);
  transform: translateY(-1px);
}

.fp-scene-card.active {
  background: color-mix(in srgb, var(--primary) 20%, rgba(255, 255, 255, 0.06));
}

.fp-scene-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.fp-scene-icon {
  width: 28px;
  height: 28px;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(255, 255, 255, 0.07);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-primary);
}

.fp-scene-badges {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.fp-scene-tag,
.fp-scene-dur {
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(255, 255, 255, 0.06);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  color: var(--text-secondary);
}

.fp-scene-name {
  margin-top: 2px;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fp-scene-desc {
  font-size: var(--bm-fs-micro, 11px);
  line-height: 1.38;
  color: var(--text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.fp-scene-music-btn {
  position: absolute;
  left: 9px;
  bottom: 7px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  padding: 0 8px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
}

.fp-scene-music-btn:hover {
  background: var(--primary);
  color: #fff;
}

.fp-ambient-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

@media (max-width: 640px) {
  .fp-ambient-grid {
    grid-template-columns: 1fr;
  }
}

.fp-ambient-card {
  border-radius: var(--bm-r-md, 14px);
  padding: 9px 12px;
  background: rgba(255, 255, 255, 0.045);
  display: flex;
  flex-direction: column;
  gap: 7px;
  cursor: pointer;
  transition: background 0.16s ease;
}

.fp-ambient-card.active {
  background: color-mix(in srgb, var(--ch-accent, var(--primary)) 16%, rgba(255, 255, 255, 0.05));
}

.fp-amb-top {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.fp-amb-icon {
  width: 34px;
  height: 34px;
  border-radius: var(--bm-r-sm, 10px);
  background: rgba(255, 255, 255, 0.07);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.fp-amb-copy {
  flex: 1;
  min-width: 0;
}

.fp-amb-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--text-primary);
}

.fp-amb-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fp-amb-pct {
  padding: 3px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(255, 255, 255, 0.07);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.fp-ambient-card.active .fp-amb-pct {
  color: var(--ch-accent, var(--primary));
}

.fp-amb-slider-row {
  display: flex;
  align-items: center;
}

.fp-amb-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 6px;
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(
    90deg,
    var(--ch-accent, var(--primary)) var(--amb-fill, 50%),
    rgba(255, 255, 255, 0.12) var(--amb-fill, 50%)
  );
  outline: none;
  cursor: pointer;
}

.fp-amb-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
  cursor: pointer;
}

.fp-stats-strip {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.fp-stat-item {
  border-radius: var(--bm-r-md, 14px);
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.035);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.fp-stat-val {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.fp-stat-lbl {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-secondary);
}

.fp-spin {
  animation: fpSpin 1s linear infinite;
}

@keyframes fpSpin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes fpOrbPulse {
  0%,
  100% {
    transform: scale(0.98);
    opacity: 0.72;
  }
  50% {
    transform: scale(1.04);
    opacity: 1;
  }
}

.fp-mode-pill:focus-visible,
.fp-close-btn:focus-visible,
.fp-np-toggle:focus-visible,
.fp-transport-primary:focus-visible,
.fp-transport-secondary:focus-visible,
.fp-switch-pill:focus-visible,
.fp-micro-pill:focus-visible,
.fp-dur-pill:focus-visible,
.fp-scene-music-btn:focus-visible,
.fp-amb-slider:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

/* D2 暗紫星云 */
.scheme-abyssal_dark.dark-variant-velvet_nebula .fp-studio-shell,
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .fp-studio-shell {
  background: linear-gradient(
    155deg,
    color-mix(in srgb, var(--primary) 13%, rgba(30, 22, 38, 0.96)),
    rgba(20, 15, 26, 0.97)
  );
}

/* L1 晨雾水彩浅色 */
.scheme-watercolor_light .fp-backdrop,
[data-theme="watercolor_light"] .fp-backdrop {
  background: transparent;
}

.scheme-watercolor_light .fp-studio-shell,
[data-theme="watercolor_light"] .fp-studio-shell {
  background: linear-gradient(
    155deg,
    color-mix(in srgb, var(--primary) 7%, rgba(252, 249, 244, 0.96)),
    rgba(245, 241, 234, 0.97)
  );
  box-shadow:
    0 28px 72px rgba(42, 34, 24, 0.22),
    0 8px 24px rgba(42, 34, 24, 0.12);
}

.scheme-watercolor_light .fp-timer-pane,
.scheme-watercolor_light .fp-block,
.scheme-watercolor_light .fp-stats-strip .fp-stat-item,
[data-theme="watercolor_light"] .fp-timer-pane,
[data-theme="watercolor_light"] .fp-block,
[data-theme="watercolor_light"] .fp-stats-strip .fp-stat-item {
  background: rgba(255, 255, 255, 0.66);
}

.scheme-watercolor_light .fp-mode-detail-card,
.scheme-watercolor_light .fp-scene-card,
.scheme-watercolor_light .fp-ambient-card,
.scheme-watercolor_light .fp-now-playing-pill,
[data-theme="watercolor_light"] .fp-mode-detail-card,
[data-theme="watercolor_light"] .fp-scene-card,
[data-theme="watercolor_light"] .fp-ambient-card,
[data-theme="watercolor_light"] .fp-now-playing-pill {
  background: rgba(255, 255, 255, 0.78);
}

.scheme-watercolor_light .fp-scene-card.active,
[data-theme="watercolor_light"] .fp-scene-card.active {
  background: color-mix(in srgb, var(--primary) 14%, rgba(255, 255, 255, 0.9));
}

.scheme-watercolor_light .fp-ambient-card.active,
[data-theme="watercolor_light"] .fp-ambient-card.active {
  background: color-mix(in srgb, var(--ch-accent, var(--primary)) 13%, rgba(255, 255, 255, 0.9));
}

.scheme-watercolor_light .fp-mode-tabs,
.scheme-watercolor_light .fp-switch-pill:not(.active),
.scheme-watercolor_light .fp-micro-pill,
.scheme-watercolor_light .fp-dur-pill:not(.active),
.scheme-watercolor_light .fp-transport-secondary,
.scheme-watercolor_light .fp-close-btn,
.scheme-watercolor_light .fp-custom-input,
.scheme-watercolor_light .fp-status-tag,
.scheme-watercolor_light .fp-scene-icon,
.scheme-watercolor_light .fp-scene-tag,
.scheme-watercolor_light .fp-scene-dur,
.scheme-watercolor_light .fp-scene-music-btn:not(:hover),
.scheme-watercolor_light .fp-amb-icon,
.scheme-watercolor_light .fp-amb-pct,
.scheme-watercolor_light .fp-stepper-item,
.scheme-watercolor_light .fp-stepper-ctrl button,
.scheme-watercolor_light .fp-pomo-dot:not(.filled),
[data-theme="watercolor_light"] .fp-mode-tabs,
[data-theme="watercolor_light"] .fp-switch-pill:not(.active),
[data-theme="watercolor_light"] .fp-micro-pill,
[data-theme="watercolor_light"] .fp-dur-pill:not(.active),
[data-theme="watercolor_light"] .fp-transport-secondary,
[data-theme="watercolor_light"] .fp-close-btn,
[data-theme="watercolor_light"] .fp-custom-input,
[data-theme="watercolor_light"] .fp-status-tag,
[data-theme="watercolor_light"] .fp-scene-icon,
[data-theme="watercolor_light"] .fp-scene-tag,
[data-theme="watercolor_light"] .fp-scene-dur,
[data-theme="watercolor_light"] .fp-scene-music-btn:not(:hover),
[data-theme="watercolor_light"] .fp-amb-icon,
[data-theme="watercolor_light"] .fp-amb-pct,
[data-theme="watercolor_light"] .fp-stepper-item,
[data-theme="watercolor_light"] .fp-stepper-ctrl button,
[data-theme="watercolor_light"] .fp-pomo-dot:not(.filled) {
  background: rgba(34, 30, 26, 0.08);
}

.scheme-watercolor_light .fp-orb-track,
[data-theme="watercolor_light"] .fp-orb-track {
  stroke: rgba(34, 30, 26, 0.1);
}

.scheme-watercolor_light .fp-live-dot:not(.running):not(.sound),
[data-theme="watercolor_light"] .fp-live-dot:not(.running):not(.sound) {
  background: rgba(34, 30, 26, 0.25);
}

.scheme-watercolor_light .fp-amb-slider,
[data-theme="watercolor_light"] .fp-amb-slider {
  background: linear-gradient(
    90deg,
    var(--ch-accent, var(--primary)) var(--amb-fill, 50%),
    rgba(34, 30, 26, 0.12) var(--amb-fill, 50%)
  );
}

/* L2 北欧燕麦暖纸 */
.scheme-watercolor_light.light-variant-nordic_oat .fp-studio-shell,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-studio-shell {
  background: linear-gradient(
    155deg,
    color-mix(in srgb, var(--primary) 6%, #FCF9F2),
    #EDE7DB
  );
}

.scheme-watercolor_light.light-variant-nordic_oat .fp-timer-pane,
.scheme-watercolor_light.light-variant-nordic_oat .fp-block,
.scheme-watercolor_light.light-variant-nordic_oat .fp-stats-strip .fp-stat-item,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-timer-pane,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-block,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-stats-strip .fp-stat-item {
  background: #F5F0E6;
}

.scheme-watercolor_light.light-variant-nordic_oat .fp-mode-detail-card,
.scheme-watercolor_light.light-variant-nordic_oat .fp-scene-card,
.scheme-watercolor_light.light-variant-nordic_oat .fp-ambient-card,
.scheme-watercolor_light.light-variant-nordic_oat .fp-now-playing-pill,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-mode-detail-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-scene-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-ambient-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .fp-now-playing-pill {
  background: #FCF9F2;
}
</style>
