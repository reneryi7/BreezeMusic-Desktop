<template>
  <footer
    class="fixed-bottom-player-bar"
    :class="{ 'player-bar-hidden': store.isNowPlayingOpen }"
  >
    <!-- 顶边 100% 全宽流光进度细轨（悬停/拖拽增粗 + 缓冲流光 + 跟随光标浮现轻量时间胶囊） -->
    <div
      class="pb-top-edge-timeline"
      :class="{ 'is-seeking': isSeekingTimeline, 'is-buffering': store.isBuffering }"
      @mousedown="handleSeekMouseDown"
      @mousemove="handleTimelineMouseMove"
    >
      <div class="pb-top-edge-rail">
        <div
          v-if="climaxSegmentStyle"
          class="pb-top-edge-climax-seg"
          :style="climaxSegmentStyle"
        ></div>
        <div class="pb-top-edge-fill-mask">
          <div
            ref="edgeFillRef"
            class="pb-top-edge-fill"
          ></div>
        </div>
        <div
          ref="edgeThumbRef"
          class="pb-top-edge-thumb-carrier"
        >
          <span class="pb-top-edge-thumb"></span>
        </div>
      </div>
      <div
        class="pb-hover-time-pill"
        :style="{ left: `${timelinePillPercent}%` }"
      >
        {{ formatDuration((timelinePillRawPercent / 100) * (store.durationMs / 1000)) }}
        / {{ formatDuration(store.durationMs / 1000) }}
      </div>
    </div>

    <!-- 左侧：专辑封面（FLIP 飞入起点锚点） + 歌名/VIP/红心 + 歌手/时长 -->
    <div
      class="pb-left-zone"
      style="cursor: pointer"
      @click="store.isNowPlayingOpen = !store.isNowPlayingOpen"
    >
      <div
        id="playerbar-flip-cover-anchor"
        class="pb-cover-frame"
        :class="{ 'is-np-open': store.isNowPlayingOpen }"
        data-tip="展开播放页"
        @click.stop="store.isNowPlayingOpen = !store.isNowPlayingOpen"
      >
        <img
          v-if="store.currentSong?.coverUrl"
          :src="store.currentSong.coverUrl"
          class="pb-cover-img"
          alt="cover"
        />
        <div v-else class="pb-cover-placeholder">
          <PlumpIcon name="disc" :size="24" />
        </div>
        <div class="pb-cover-expand-mask">
          <PlumpIcon name="maximize" :size="16" />
        </div>
      </div>

      <transition :name="trackMetaTransitionName" mode="out-in">
        <div
          :key="store.currentSong?.hash || store.currentSong?.id || '__idle__'"
          class="pb-song-meta"
        >
          <div class="pb-title-line">
            <span
              class="pb-song-title"
              @click.stop="store.isNowPlayingOpen = !store.isNowPlayingOpen"
            >
              {{ store.currentSong?.title || "微风音乐 · 概念声学" }}
            </span>
            <span
              v-if="store.currentSong?.isVip"
              class="pb-title-vip-badge"
            >
              VIP
            </span>
            <button
              v-if="store.currentSong"
              ref="likeBtnRef"
              type="button"
              class="pb-inline-like-btn"
              :class="{ liked: store.isSongLiked(store.currentSong) }"
              :data-tip="store.isSongLiked(store.currentSong) ? '取消喜欢' : '收藏到我喜欢'"
              @click.stop="handleLikeCurrentSong"
            >
              <PlumpIcon
                :name="store.isSongLiked(store.currentSong) ? 'heart-filled' : 'heart'"
                :size="15"
              />
            </button>
          </div>

          <div class="pb-artist-line">
            <span
              class="pb-song-artist"
              :class="{ 'is-clickable': Boolean(store.currentSong) }"
              :data-tip="store.currentSong ? '查看歌手详情' : undefined"
              @click.stop="store.currentSong && store.navigateToSongArtist(store.currentSong)"
            >
              {{ store.currentSong?.artist || "轻触发现开启无损声学之旅" }}
            </span>
            <template v-if="store.currentSong && store.durationMs > 0">
              <span class="pb-meta-sep" aria-hidden="true">·</span>
              <span class="pb-persistent-time">
                {{ formatDuration(displayElapsedSec) }} / {{ formatDuration(store.durationMs / 1000) }}
              </span>
            </template>
          </div>
        </div>
      </transition>
    </div>

    <!-- 中区：重绘放大舒展三键播控（上一首 / 播放暂停 / 下一首） -->
    <div class="pb-center-zone">
      <div class="pb-center-single-row">
        <button
          ref="prevBtnRef"
          type="button"
          class="pb-ctrl-btn"
          data-tip="上一曲 (Ctrl+Left)"
          aria-label="上一曲"
          @click="handlePrevClick"
        >
          <PlumpIcon name="prev" :size="22" color="currentColor" accent="var(--primary)" />
        </button>

        <button
          type="button"
          class="pb-play-hero-circle pb-hero-play-orb"
          :class="{ 'is-playing-live': store.isPlaying }"
          :data-tip="store.isPlaying ? '暂停 (Space)' : '播放 (Space)'"
          :aria-label="store.isPlaying ? '暂停' : '播放'"
          @click="store.togglePlayPause()"
        >
          <transition name="bm-icon-morph" mode="out-in">
            <PlumpIcon
              :key="store.isPlaying ? 'pause' : 'play'"
              :name="store.isPlaying ? 'pause' : 'play'"
              :size="24"
              color="#ffffff"
            />
          </transition>
        </button>

        <button
          ref="nextBtnRef"
          type="button"
          class="pb-ctrl-btn"
          data-tip="下一曲 (Ctrl+Right)"
          aria-label="下一曲"
          @click="handleNextClick"
        >
          <PlumpIcon name="next" :size="22" color="currentColor" accent="var(--primary)" />
        </button>
      </div>
    </div>

    <!-- 右侧：触发式大横条音量 / 音质 / 音效 / 播放顺序 / 桌面歌词 / 统一队列（移除心流舱） -->
    <div class="pb-right-zone">
      <!-- ① 精简音量触发器 + 正上方超宽 264px 大横条精调浮层 -->
      <div
        ref="volumeControlRef"
        class="pb-volume-control"
        data-no-tip="true"
        :class="{ 'is-open': isVolumePopupOpen }"
        @mouseenter="handleVolumeAreaMouseEnter"
        @mousemove="handleVolumeAreaMouseEnter"
        @mouseleave="handleVolumeAreaMouseLeave"
        @wheel.prevent="handleVolumeWheel"
      >
        <div
          class="pb-vol-trigger-pill"
          data-no-tip="true"
          @click.stop="toggleVolumePopover"
        >
          <button
            ref="muteBtnRef"
            type="button"
            class="pb-vol-mute-btn"
            data-no-tip="true"
            :aria-label="store.volume === 0 ? '取消静音' : '静音'"
            @click.stop="toggleMute"
          >
            <PlumpIcon :name="store.volume === 0 ? 'volume-mute' : 'volume'" :size="16" />
          </button>
          <button
            type="button"
            class="pb-volume-pct-readout"
            data-no-tip="true"
            :aria-expanded="isVolumePopupOpen"
            @click.stop="toggleVolumePopover"
          >
            {{ volumePercent }}%
          </button>
        </div>

        <transition name="fade-pop">
          <div
            v-if="isVolumePopupOpen"
            ref="volumePopupRef"
            class="pb-wide-volume-popover"
            data-no-tip="true"
            @click.stop
          >
            <div class="pb-wide-vol-header">
              <div class="pb-wide-vol-title">
                <PlumpIcon :name="store.volume === 0 ? 'volume-mute' : 'volume'" :size="15" />
                <span>{{ store.volume === 0 ? "已静音" : "精准音量调节" }}</span>
              </div>
              <span class="pb-wide-vol-value">{{ volumePercent }}%</span>
            </div>

            <div class="pb-wide-vol-bar-row">
              <button
                type="button"
                class="pb-vol-step-btn"
                data-no-tip="true"
                aria-label="音量减 1%"
                @mousedown.stop="startStepRepeat(-1)"
                @mouseup="stopStepRepeat"
                @mouseleave="stopStepRepeat"
              >
                <PlumpIcon name="minus" :size="12" />
              </button>

              <input
                :value="volumePercent"
                type="range"
                min="0"
                max="100"
                step="1"
                class="pb-wide-vol-slider"
                data-no-tip="true"
                :style="{ '--vol-pct': `${volumePercent}%` }"
                aria-label="主音量调节"
                :aria-valuetext="`${volumePercent}%`"
                @input="handleWideSliderInput"
              />

              <button
                type="button"
                class="pb-vol-step-btn"
                data-no-tip="true"
                aria-label="音量加 1%"
                @mousedown.stop="startStepRepeat(1)"
                @mouseup="stopStepRepeat"
                @mouseleave="stopStepRepeat"
              >
                <PlumpIcon name="plus" :size="12" />
              </button>
            </div>
          </div>
        </transition>
      </div>

      <!-- ② 音质快捷选择 -->
      <div ref="qualitySlotRef" class="pb-tool-btn-slot">
        <button
          type="button"
          class="pb-uniform-tool-btn pb-quality-uniform-btn"
          :class="[`tier-${qualityTierClass}`, { active: isQualityPopoverOpen }]"
          data-tip="切换在线音频规格"
          @click.stop="toggleQualityPopover"
        >
          <span class="pb-quality-compact-label">{{ qualityBadgeText }}</span>
          <PlumpIcon name="chevron-up" :size="11" />
        </button>

        <transition name="fade-pop">
          <div v-if="isQualityPopoverOpen" class="pb-quality-popover" @click.stop>
            <div class="pb-popover-head">
              <span>在线音频规格</span>
              <span
                class="pb-popover-head-tag"
                :class="{ 'is-gold': qualityTierClass === 'hires' }"
              >
                {{ qualityHeaderStatusText }}
              </span>
            </div>

            <div class="pb-popover-card-list">
              <button
                v-for="item in qualityMenuOptions"
                :key="item.id"
                type="button"
                class="pb-popover-item"
                :class="{
                  active: isQualityOptionActive(item.id),
                  'is-gold': item.isGold,
                }"
                @click="selectAudioQualityOption(item.id)"
              >
                <span class="pb-popover-item-main">
                  <span class="pb-popover-title-row">
                    <span>{{ item.title }}</span>
                    <span class="pb-popover-tier-tag" :class="{ gold: item.isGold }">
                      {{ item.badge }}
                    </span>
                    <span v-if="item.sizeText" class="pb-popover-size-tag">
                      {{ item.sizeText }}
                    </span>
                  </span>
                  <span class="pb-popover-sub">{{ item.spec }}</span>
                </span>
                <span v-if="isQualityOptionActive(item.id)" class="pb-popover-check">
                  <PlumpIcon name="check" :size="12" />
                </span>
              </button>
            </div>
          </div>
        </transition>
      </div>

      <!-- ③ 音效快捷矩阵（直连真实 DSP / 蝰蛇预设 / 记忆槽 / 经典 EQ） -->
      <div ref="fxSlotRef" class="pb-tool-btn-slot">
        <button
          type="button"
          class="pb-uniform-tool-btn"
          :class="{ active: isTunerActive || isFxPopoverOpen }"
          :data-tip="activeAcousticSummaryText"
          @click.stop="toggleFxPopover"
        >
          <PlumpIcon name="tuning" :size="17" />
          <span v-if="isTunerActive" class="pb-fx-jade-dot" aria-hidden="true"></span>
        </button>

        <transition name="fade-pop">
          <div v-if="isFxPopoverOpen" class="pb-fx-preset-popover" @click.stop>
            <div class="pb-popover-head">
              <span>声学音效矩阵</span>
              <div class="pb-popover-head-actions">
                <button
                  type="button"
                  class="pb-dsp-switch-pill"
                  :class="{ on: store.masterEffectEnabled }"
                  @click="store.toggleMasterEffect()"
                >
                  <span class="pb-dsp-dot"></span>
                  <span>{{ store.masterEffectEnabled ? "DSP 开启" : "DSP 旁路" }}</span>
                </button>
                <button
                  type="button"
                  class="pb-fx-reset-btn"
                  @click="resetAllAcousticFromPopover"
                >
                  原声复位
                </button>
              </div>
            </div>

            <div class="pb-fx-status-banner">
              <span>当前声学</span>
              <strong>{{ activeAcousticSummaryText }}</strong>
            </div>

            <div class="pb-popover-section-label">蝰蛇母带精选</div>
            <div class="pb-popover-card-list">
              <button
                v-for="preset in quickViperPresets"
                :key="preset.id"
                type="button"
                class="pb-popover-item"
                :class="{
                  active: store.masterEffectEnabled && store.activeViperEffectId === preset.id,
                  'is-gold': isGoldViperId(preset.id),
                }"
                @click="selectViperPresetFromPopover(preset.id)"
              >
                <span class="pb-popover-item-main">
                  <span class="pb-popover-title-row">
                    <span>{{ preset.label || preset.name }}</span>
                    <span
                      v-if="preset.category"
                      class="pb-popover-tier-tag"
                      :class="{ gold: isGoldViperId(preset.id) }"
                    >
                      {{ preset.category }}
                    </span>
                  </span>
                  <span class="pb-popover-sub">{{ preset.desc }}</span>
                </span>
                <span
                  v-if="store.masterEffectEnabled && store.activeViperEffectId === preset.id"
                  class="pb-popover-check"
                >
                  <PlumpIcon name="check" :size="12" />
                </span>
              </button>
            </div>

            <div class="pb-popover-section-label" style="margin-top: 10px">
              记忆槽与经典均衡
            </div>
            <div class="pb-fx-chips-row">
              <button
                v-for="slot in savedMemorySlots"
                :key="slot.slotId"
                type="button"
                class="pb-fx-mini-chip"
                @click="applyMemorySlotFromPopover(slot.slotId)"
              >
                {{ slot.name || `记忆槽 ${slot.slotId}` }}
              </button>
              <button
                v-for="eq in quickEqChips"
                :key="eq.id"
                type="button"
                class="pb-fx-mini-chip"
                :class="{ active: isEqChipActive(eq.id) }"
                @click="selectEqPresetFromPopover(eq.id)"
              >
                {{ eq.label }}
              </button>
            </div>

            <div class="pb-popover-footer-grid">
              <button
                type="button"
                class="pb-popover-footer-link"
                @click="openViperStudioFromPopover"
              >
                <span>蝰蛇声学中心</span>
                <PlumpIcon name="chevron-right" :size="13" />
              </button>
              <button
                type="button"
                class="pb-popover-footer-link secondary"
                @click="openTunerFromPopover"
              >
                <span>专业调音台</span>
                <PlumpIcon name="chevron-right" :size="13" />
              </button>
            </div>
          </div>
        </transition>
      </div>

      <!-- ④ 播放顺序 -->
      <div class="pb-tool-btn-slot">
        <button
          ref="playModeBtnRef"
          type="button"
          class="pb-uniform-tool-btn"
          :class="{ active: store.playMode !== 'sequence' }"
          :data-tip="playModeTitle"
          @click="handleCyclePlayMode"
        >
          <PlumpIcon :name="playModeIcon" :size="17" />
        </button>
      </div>

      <!-- ⑤ 桌面歌词 -->
      <div class="pb-tool-btn-slot">
        <button
          type="button"
          class="pb-uniform-tool-btn"
          :class="{ active: store.desktopLyricEnabled }"
          data-tip="桌面歌词"
          @click="toggleDesktopLyric"
        >
          <PlumpIcon name="lyric" :size="17" />
        </button>
      </div>

      <!-- ⑥ 统一队列按钮（内嵌计数胶囊，取消右上角翘出圆球） -->
      <div class="pb-tool-btn-slot">
        <button
          id="playerbar-queue-anchor"
          ref="queueToggleBtnRef"
          type="button"
          class="pb-uniform-tool-btn pb-queue-pill-btn"
          :class="{ active: store.isQueueOpen }"
          data-tip="播放队列"
          @click.stop="toggleQueuePopover"
        >
          <PlumpIcon name="queue" :size="17" />
          <span v-if="store.playQueue.length > 0" class="pb-queue-inline-count">
            <RollingCounter
              :value="store.playQueue.length"
              :display-text="store.playQueue.length > 99 ? '99+' : String(store.playQueue.length)"
            />
          </span>
        </button>
      </div>
    </div>
  </footer>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { formatDuration, normalizeConceptQuality } from "../api/kugou";
import { CLASSIC_EQ_PRESETS } from "../utils/audioDsp";
import { triggerOneShotClass } from "../utils/microMotionFx";
import { getMotionRuntimeProfile } from "../utils/motionRuntime";
import {
  isAppForegroundActive,
  subscribeAppPowerVisibility,
} from "../utils/windowPowerState";
import PlumpIcon from "./PlumpIcon.vue";
import RollingCounter from "./RollingCounter.vue";

const store = useAppStore();
const lastVolume = ref(0.8);
const queueToggleBtnRef = ref<HTMLElement | null>(null);
const volumeControlRef = ref<HTMLElement | null>(null);
const volumePopupRef = ref<HTMLElement | null>(null);
const qualitySlotRef = ref<HTMLElement | null>(null);
const fxSlotRef = ref<HTMLElement | null>(null);
const prevBtnRef = ref<HTMLElement | null>(null);
const nextBtnRef = ref<HTMLElement | null>(null);
const likeBtnRef = ref<HTMLElement | null>(null);
const muteBtnRef = ref<HTMLElement | null>(null);
const playModeBtnRef = ref<HTMLElement | null>(null);

const trackMetaTransitionName = computed(() => {
  if (getMotionRuntimeProfile().effectiveLevel === "none") return "";
  return store.lastTrackNavDir === "prev"
    ? "bm-track-slide-prev"
    : "bm-track-slide-next";
});

function handlePrevClick() {
  triggerOneShotClass(prevBtnRef.value, "bm-nudge-left", 380);
  void store.playPrev();
}

function handleNextClick() {
  triggerOneShotClass(nextBtnRef.value, "bm-nudge-right", 380);
  void store.playNext();
}

function handleLikeCurrentSong() {
  if (!store.currentSong) return;
  const willLike = !store.isSongLiked(store.currentSong);
  if (willLike) {
    triggerOneShotClass(likeBtnRef.value, "bm-heart-pop", 480);
  }
  void store.toggleFavoriteSong(store.currentSong);
}

function handleCyclePlayMode() {
  triggerOneShotClass(playModeBtnRef.value, "bm-mode-spin-pop", 380);
  store.cyclePlayMode();
}

const edgeFillRef = ref<HTMLElement | null>(null);
const edgeThumbRef = ref<HTMLElement | null>(null);
const hoverTimelinePercent = ref(0);
const isSeekingTimeline = ref(false);
const displayElapsedSec = ref(0);
let progressRafId: number | null = null;
let lastEdgePaintPerfMs = 0;
const isVolumePopupOpen = ref(false);
const isVolumePinned = ref(false);
const isHoveringVolume = ref(false);
const isQualityPopoverOpen = ref(false);
const isFxPopoverOpen = ref(false);

let closeDelayTimer: number | null = null;
let stepRepeatDelayTimer: number | null = null;
let stepRepeatIntervalTimer: number | null = null;

const volumePercent = computed(() => Math.round((store.volume || 0) * 100));

function clearVolumeTimers() {
  if (closeDelayTimer !== null) {
    window.clearTimeout(closeDelayTimer);
    closeDelayTimer = null;
  }
}

function stopStepRepeat() {
  if (stepRepeatDelayTimer !== null) {
    window.clearTimeout(stepRepeatDelayTimer);
    stepRepeatDelayTimer = null;
  }
  if (stepRepeatIntervalTimer !== null) {
    window.clearInterval(stepRepeatIntervalTimer);
    stepRepeatIntervalTimer = null;
  }
}

function scheduleVolumePopupClose(delayMs = 240) {
  clearVolumeTimers();
  closeDelayTimer = window.setTimeout(() => {
    if (!isHoveringVolume.value && !isVolumePinned.value) {
      isVolumePopupOpen.value = false;
    }
    closeDelayTimer = null;
  }, delayMs);
}

function handleVolumeAreaMouseEnter() {
  isHoveringVolume.value = true;
  clearVolumeTimers();
  isQualityPopoverOpen.value = false;
  isFxPopoverOpen.value = false;
  isVolumePopupOpen.value = true;
}

function handleVolumeAreaMouseLeave(e?: MouseEvent) {
  const nextTarget = e?.relatedTarget as Node | null;
  if (nextTarget && volumeControlRef.value?.contains(nextTarget)) {
    return;
  }
  isHoveringVolume.value = false;
  if (!isVolumePinned.value) {
    scheduleVolumePopupClose(260);
  }
}

function toggleVolumePopover() {
  isQualityPopoverOpen.value = false;
  isFxPopoverOpen.value = false;
  store.isQueueOpen = false;
  if (isVolumePopupOpen.value && isVolumePinned.value) {
    isVolumePinned.value = false;
    isVolumePopupOpen.value = false;
  } else {
    isVolumePinned.value = true;
    isVolumePopupOpen.value = true;
  }
}

function clampRatio(val: number) {
  return Math.min(1, Math.max(0, Number(val.toFixed(2))));
}

function adjustVolumeByPercent(deltaPct: number) {
  const nextPct = Math.max(0, Math.min(100, volumePercent.value + deltaPct));
  store.setVolume(clampRatio(nextPct / 100));
}

function startStepRepeat(deltaPct: number) {
  stopStepRepeat();
  adjustVolumeByPercent(deltaPct);
  stepRepeatDelayTimer = window.setTimeout(() => {
    stepRepeatIntervalTimer = window.setInterval(() => {
      adjustVolumeByPercent(deltaPct);
    }, 55);
  }, 320);
}

function handleWideSliderInput(e: Event) {
  const raw = Number((e.target as HTMLInputElement).value);
  if (!Number.isFinite(raw)) return;
  store.setVolume(clampRatio(raw / 100));
}

function handleVolumeWheel(e: WheelEvent) {
  clearVolumeTimers();
  isVolumePopupOpen.value = true;
  const deltaPct = e.deltaY < 0 ? 1 : -1;
  adjustVolumeByPercent(deltaPct);
  if (!isHoveringVolume.value && !isVolumePinned.value) {
    scheduleVolumePopupClose(1000);
  }
}

function toggleMute() {
  if (store.volume > 0) {
    lastVolume.value = store.volume;
    store.setVolume(0);
    triggerOneShotClass(muteBtnRef.value, "bm-mute-shake", 360);
  } else {
    store.setVolume(lastVolume.value || 0.8);
  }
}

const timelinePillRawPercent = computed(() => hoverTimelinePercent.value);

const timelinePillPercent = computed(() =>
  Math.min(96.5, Math.max(3.5, timelinePillRawPercent.value))
);

function handleTimelineMouseMove(e: MouseEvent) {
  const bar = e.currentTarget as HTMLElement;
  const rect = bar.getBoundingClientRect();
  if (rect.width <= 0) return;
  const pct = ((e.clientX - rect.left) / rect.width) * 100;
  hoverTimelinePercent.value = Math.min(100, Math.max(0, pct));
}

function handleOutsideMouseDown(e: MouseEvent) {
  const target = e.target as Node | null;
  if (!target) return;

  if (isQualityPopoverOpen.value && !qualitySlotRef.value?.contains(target)) {
    isQualityPopoverOpen.value = false;
  }

  if (isFxPopoverOpen.value && !fxSlotRef.value?.contains(target)) {
    isFxPopoverOpen.value = false;
  }

  if (isVolumePopupOpen.value && !volumeControlRef.value?.contains(target)) {
    isVolumePinned.value = false;
    isVolumePopupOpen.value = false;
  }
}

function syncEdgeTimelineDom(posMs: number) {
  const dur = store.durationMs || 0;
  const pct =
    dur <= 0
      ? 0
      : isSeekingTimeline.value
      ? Math.min(100, Math.max(0, hoverTimelinePercent.value))
      : Math.min(100, Math.max(0, (posMs / dur) * 100));
  const tr = `translate3d(${(pct - 100).toFixed(4)}%, 0, 0)`;
  if (edgeFillRef.value) edgeFillRef.value.style.transform = tr;
  if (edgeThumbRef.value) edgeThumbRef.value.style.transform = tr;
  const nextSec = Math.max(0, Math.floor((isSeekingTimeline.value ? (pct / 100) * dur : posMs) / 1000));
  if (displayElapsedSec.value !== nextSec) {
    displayElapsedSec.value = nextSec;
  }
}

function stopProgressClock() {
  if (progressRafId !== null) {
    cancelAnimationFrame(progressRafId);
    progressRafId = null;
  }
}

function startProgressClock() {
  if (
    progressRafId !== null ||
    !store.isPlaying ||
    store.isNowPlayingOpen ||
    !isAppForegroundActive()
  ) {
    return;
  }
  lastEdgePaintPerfMs = 0;
  const tick = (now: number) => {
    if (
      !store.isPlaying ||
      store.isNowPlayingOpen ||
      !isAppForegroundActive()
    ) {
      progressRafId = null;
      return;
    }
    // ~25fps (40ms) 直接写 DOM transform，避免 60Hz Vue 重渲染底栏
    if (now - lastEdgePaintPerfMs >= 40) {
      lastEdgePaintPerfMs = now;
      if (!isSeekingTimeline.value) {
        syncEdgeTimelineDom(store.getLivePlaybackPositionMs());
      }
    }
    progressRafId = requestAnimationFrame(tick);
  };
  progressRafId = requestAnimationFrame(tick);
}

watch(
  [() => store.isPlaying, () => store.isNowPlayingOpen],
  ([playing, npOpen]) => {
    syncEdgeTimelineDom(store.getLivePlaybackPositionMs());
    if (playing && !npOpen && isAppForegroundActive()) {
      startProgressClock();
    } else {
      stopProgressClock();
    }
  },
  { immediate: true }
);

watch(
  () => [store.currentPositionMs, store.currentSong?.hash, store.durationMs],
  () => {
    if (!store.isPlaying || store.isNowPlayingOpen || isSeekingTimeline.value) {
      syncEdgeTimelineDom(store.currentPositionMs || 0);
    }
  }
);

let unsubscribePowerVis: (() => void) | null = null;

onMounted(() => {
  window.addEventListener("mousedown", handleOutsideMouseDown, true);
  unsubscribePowerVis = subscribeAppPowerVisibility((active) => {
    if (!active) {
      stopProgressClock();
    } else if (store.isPlaying && !store.isNowPlayingOpen) {
      syncEdgeTimelineDom(store.getLivePlaybackPositionMs());
      startProgressClock();
    }
  });
  syncEdgeTimelineDom(store.getLivePlaybackPositionMs());
});

onBeforeUnmount(() => {
  stopProgressClock();
  clearVolumeTimers();
  stopStepRepeat();
  window.removeEventListener("mousedown", handleOutsideMouseDown, true);
  if (unsubscribePowerVis) {
    unsubscribePowerVis();
    unsubscribePowerVis = null;
  }
});

const climaxSegmentStyle = computed(() => {
  const song = store.currentSong;
  const cStart = song?.climaxStartMs || store.currentPlaybackInfo?.climaxStartMs || 0;
  const cEnd = song?.climaxEndMs || store.currentPlaybackInfo?.climaxEndMs || 0;
  if (!song || !store.durationMs || cEnd <= cStart) return null;
  const startPct = Math.max(0, Math.min(100, (cStart / store.durationMs) * 100));
  const widthPct = Math.max(
    2,
    Math.min(100 - startPct, ((cEnd - cStart) / store.durationMs) * 100)
  );
  return {
    left: `${startPct}%`,
    width: `${widthPct}%`,
  };
});

// ================= 真实音质数据（同步 currentSongAvailableQualities） =================
const activeQualityNormalized = computed(() => {
  const raw =
    store.currentPlaybackInfo?.actualQuality ||
    store.currentPlaybackInfo?.matchedQuality ||
    store.preferredQuality;
  return normalizeConceptQuality(raw);
});

const qualityBadgeText = computed(() => {
  if (store.currentPlaybackInfo?.qualityBadge) {
    return store.currentPlaybackInfo.qualityBadge;
  }
  switch (activeQualityNormalized.value) {
    case "high":
      return "Hi-Res";
    case "flac":
      return "SQ";
    case "320":
      return "HQ";
    default:
      return "128K";
  }
});

const qualityTierClass = computed(() => {
  const q = activeQualityNormalized.value;
  if (q === "high") return "hires";
  if (q === "flac") return "sq";
  if (q === "320") return "hq";
  return "std";
});

const qualityHeaderStatusText = computed(() => {
  if (!store.currentSong) return "默认首选偏好";
  const pref = normalizeConceptQuality(store.preferredQuality);
  const actual = activeQualityNormalized.value;
  if (store.currentPlaybackInfo && pref !== actual) {
    return `实际播放: ${qualityBadgeText.value}`;
  }
  return `当前: ${qualityBadgeText.value}`;
});

const qualityMenuOptions = computed(() => {
  const avail = store.currentSongAvailableQualities || [];
  if (store.currentSong && avail.length > 0) {
    return avail;
  }
  return [
    {
      id: "high" as const,
      title: "Hi-Res 高解析无损",
      badge: "Hi-Res",
      isGold: true,
      spec: "24-bit / 96kHz ~ 192kHz · 录音室原声母带",
      sizeText: "首选上限",
    },
    {
      id: "flac" as const,
      title: "SQ 超品无损音质",
      badge: "FLAC",
      isGold: false,
      spec: "16-bit / 44.1kHz ~ 48kHz · CD 级无损压缩",
      sizeText: "CD 无损",
    },
    {
      id: "320" as const,
      title: "HQ 高品音质",
      badge: "320K",
      isGold: false,
      spec: "320kbps CBR MP3 · 饱满高保真立体声",
      sizeText: "高品立体声",
    },
    {
      id: "128" as const,
      title: "标准流畅音质",
      badge: "128K",
      isGold: false,
      spec: "128kbps AAC/MP3 · 省流极速秒开",
      sizeText: "标准音质",
    },
  ];
});

function isQualityOptionActive(optId: string) {
  const normOpt = normalizeConceptQuality(optId);
  if (!store.currentSong) {
    return normalizeConceptQuality(store.preferredQuality) === normOpt;
  }
  return activeQualityNormalized.value === normOpt;
}

// ================= 真实音效数据（同步 ViperEffects / EQ / 记忆槽 / DSP） =================
const CORE_VIPER_IDS = [
  "viper_3d",
  "viper_bass",
  "viper_vocal",
  "viper_hifi",
  "viper_panorama_51",
];

const quickViperPresets = computed(() => {
  const all = store.viperEffects || [];
  const selected = [];
  const activeId = store.activeViperEffectId;
  if (activeId && activeId !== "off" && !CORE_VIPER_IDS.includes(activeId)) {
    const activeObj = all.find((item) => item.id === activeId);
    if (activeObj) selected.push(activeObj);
  }
  for (const id of CORE_VIPER_IDS) {
    const found = all.find((item) => item.id === id);
    if (found) selected.push(found);
  }
  return selected.slice(0, 5);
});

const savedMemorySlots = computed(() =>
  (store.peqMemorySlots || []).filter((slot) => slot.saved)
);

const CORE_EQ_CHIP_IDS = ["pop", "rock", "classical", "vocal"];
const quickEqChips = computed(() => {
  const ids = [...CORE_EQ_CHIP_IDS];
  const curEq = store.activeEqPreset;
  if (
    curEq &&
    curEq !== "flat" &&
    curEq !== "custom" &&
    !ids.includes(curEq) &&
    CLASSIC_EQ_PRESETS[curEq]
  ) {
    ids.splice(0, 0, curEq);
  }
  return ids
    .filter((id) => Boolean(CLASSIC_EQ_PRESETS[id]))
    .map((id) => ({
      id,
      label: CLASSIC_EQ_PRESETS[id].label,
    }));
});

function isGoldViperId(id: string) {
  return (
    id === "viper_black_diamond" ||
    id === "viper_clear_master" ||
    id === "viper_panorama_51"
  );
}

const isTunerActive = computed(() => {
  const viperOn =
    Boolean(store.activeViperEffectId) &&
    store.activeViperEffectId !== "off" &&
    store.activeViperEffectId !== "none";
  const eqModified = store.eqGains.some((g) => Math.abs(g) >= 0.05);
  return Boolean(
    store.masterEffectEnabled &&
      ((!store.peqBypass && (eqModified || store.activeEqPreset !== "flat")) ||
        viperOn ||
        store.acousticField.enabled)
  );
});

const activeAcousticSummaryText = computed(() => {
  if (!store.masterEffectEnabled) {
    return "DSP 引擎已旁路 · 原声直通";
  }
  const viper = (store.viperEffects || []).find(
    (v) => v.id === store.activeViperEffectId
  );
  if (viper) {
    return `蝰蛇生效 · ${viper.label || viper.name}`;
  }
  if (store.activeEqPreset && store.activeEqPreset !== "flat") {
    const eqMeta = CLASSIC_EQ_PRESETS[store.activeEqPreset];
    return eqMeta
      ? `均衡器 · ${eqMeta.label}`
      : `调音曲线 · ${store.activeEqPreset}`;
  }
  if (store.acousticField.enabled) {
    return "正在生效 · 自定义听感微调";
  }
  return "原声直通 (Bit-Perfect 0dB)";
});

function isEqChipActive(eqId: string): boolean {
  const viperOff =
    !store.activeViperEffectId ||
    store.activeViperEffectId === "off" ||
    store.activeViperEffectId === "none";
  if (eqId === "flat") {
    return (
      !store.masterEffectEnabled ||
      (viperOff &&
        !store.acousticField.enabled &&
        (store.activeEqPreset === "flat" || store.peqBypass))
    );
  }
  return (
    store.masterEffectEnabled &&
    viperOff &&
    !store.peqBypass &&
    store.activeEqPreset === eqId
  );
}

function selectViperPresetFromPopover(id: string) {
  if (store.masterEffectEnabled && store.activeViperEffectId === id) {
    store.applyViperEffect("off");
  } else {
    store.masterEffectEnabled = true;
    store.applyViperEffect(id);
  }
  isFxPopoverOpen.value = false;
}

function selectEqPresetFromPopover(eqId: string) {
  if (eqId === "flat") {
    store.resetAcousticAndEqToDefault();
  } else {
    if (store.activeViperEffectId && store.activeViperEffectId !== "off") {
      store.applyViperEffect("off");
    }
    store.masterEffectEnabled = true;
    store.peqBypass = false;
    store.setEqPreset(eqId);
  }
  isFxPopoverOpen.value = false;
}

function applyMemorySlotFromPopover(slotId: "A" | "B" | "C") {
  store.masterEffectEnabled = true;
  store.peqBypass = false;
  store.loadPeqFromSlot(slotId);
  isFxPopoverOpen.value = false;
}

function resetAllAcousticFromPopover() {
  store.resetAcousticAndEqToDefault();
  if (store.activeCloudStreamEffect) {
    store.switchCloudStreamEffect(store.activeCloudStreamEffect);
  }
}

function openViperStudioFromPopover() {
  isFxPopoverOpen.value = false;
  store.isViperStudioModalOpen = true;
}

function openTunerFromPopover() {
  isFxPopoverOpen.value = false;
  store.openSoundMixerFromPlayer();
}

// ================= 播放模式 / 桌词 / 弹窗切换 =================
const playModeIcon = computed(() => {
  switch (store.playMode) {
    case "shuffle":
      return "shuffle";
    case "loop_one":
    case "repeat_one":
      return "repeat-one";
    case "sequence":
      return "sequence";
    default:
      return "repeat";
  }
});

const playModeTitle = computed(() => {
  switch (store.playMode) {
    case "sequence":
      return "顺序播放";
    case "loop_all":
    case "repeat_all":
      return "列表循环";
    case "loop_one":
    case "repeat_one":
      return "单曲循环";
    case "shuffle":
      return "随机漫游";
    default:
      return "列表循环";
  }
});

function handleSeekMouseDown(e: MouseEvent) {
  const bar = e.currentTarget as HTMLElement;
  isSeekingTimeline.value = true;
  const updateSeek = (ev: MouseEvent) => {
    const rect = bar.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width));
    hoverTimelinePercent.value = ratio * 100;
    const nextMs = ratio * (store.durationMs || 0);
    syncEdgeTimelineDom(nextMs);
    store.seekToMs(nextMs);
  };
  updateSeek(e);
  const onMove = (ev: MouseEvent) => updateSeek(ev);
  const onUp = () => {
    isSeekingTimeline.value = false;
    syncEdgeTimelineDom(store.getLivePlaybackPositionMs());
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

function toggleDesktopLyric() {
  store.desktopLyricEnabled = !store.desktopLyricEnabled;
  store.notify(store.desktopLyricEnabled ? "已开启桌面悬浮逐字歌词" : "已关闭桌面悬浮歌词");
}

function toggleQualityPopover() {
  isFxPopoverOpen.value = false;
  isVolumePinned.value = false;
  isVolumePopupOpen.value = false;
  store.isQueueOpen = false;
  isQualityPopoverOpen.value = !isQualityPopoverOpen.value;
}

function selectAudioQualityOption(qId: string) {
  store.switchAudioQuality(qId as any);
  isQualityPopoverOpen.value = false;
}

function toggleFxPopover() {
  isQualityPopoverOpen.value = false;
  isVolumePinned.value = false;
  isVolumePopupOpen.value = false;
  store.isQueueOpen = false;
  isFxPopoverOpen.value = !isFxPopoverOpen.value;
}

function toggleQueuePopover() {
  isQualityPopoverOpen.value = false;
  isFxPopoverOpen.value = false;
  isVolumePinned.value = false;
  isVolumePopupOpen.value = false;
  store.queueAnchorRect = null;
  store.isQueueOpen = !store.isQueueOpen;
}
</script>

<style scoped>
.pb-cover-frame {
  transform: translate3d(0, 0, 0) rotate3d(0, 0, 1, 0deg);
  backface-visibility: hidden;
  will-change: opacity, transform;
}

.pb-cover-frame.is-np-open .pb-cover-expand-mask {
  opacity: 0 !important;
  transition: none !important;
}
</style>
