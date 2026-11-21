import { computed, ref, type Ref } from "vue";
import { kugouClient } from "../../api/kugou";
import type { FmChannel, Song } from "../../types";
import {
  ambientSoundEngine,
  type AmbientChannelId,
} from "../../utils/ambientSoundEngine";

export type FocusModeType = "countdown" | "pomodoro" | "stopwatch";
export type FocusRunState = "idle" | "running" | "paused" | "completed";
export type PomodoroPhase = "focus" | "short_break" | "long_break";
export type FocusSceneTag = "STUDY" | "SLEEP" | "HEALING" | "CAFE" | "WORKOUT" | "DRIVE";

export interface FocusScenePreset {
  id: string;
  title: string;
  subtitle: string;
  mode: FocusModeType;
  minutes: number;
  tag: FocusSceneTag;
  musicChannelHint: string[];
  searchFallbackQuery: string;
  ambientMix: Partial<Record<AmbientChannelId, number>>;
}

export const FOCUS_SCENE_PRESETS: readonly FocusScenePreset[] = [
  {
    id: "pomodoro_25",
    title: "番茄工作法",
    subtitle: "25分钟深度专注 · 5分钟小憩律动",
    mode: "pomodoro",
    minutes: 25,
    tag: "STUDY",
    musicChannelHint: ["学习", "爵士", "lofi", "纯音乐"],
    searchFallbackQuery: "jazz lofi study",
    ambientMix: { rain: 0.45, vinyl: 0.35 },
  },
  {
    id: "rain_study",
    title: "雨夜书桌",
    subtitle: "窗外细雨 + 温暖唱针 · 阅读/编码沉浸",
    mode: "countdown",
    minutes: 45,
    tag: "STUDY",
    musicChannelHint: ["学习", "工作", "纯音乐", "钢琴"],
    searchFallbackQuery: "轻音乐 钢琴 学习",
    ambientMix: { rain: 0.68, vinyl: 0.32 },
  },
  {
    id: "retro_vinyl",
    title: "黑胶深夜阅读",
    subtitle: "暖黄壁炉微光 + 模拟碟片沙沙底噪",
    mode: "countdown",
    minutes: 30,
    tag: "CAFE",
    musicChannelHint: ["爵士", "下午茶", "慵懒", "咖啡"],
    searchFallbackQuery: "acoustic jazz chill",
    ambientMix: { vinyl: 0.56, campfire: 0.25 },
  },
  {
    id: "camp_night",
    title: "林间露营篝火",
    subtitle: "柴火轻爆与夜雨交织 · 舒缓内耗与焦虑",
    mode: "countdown",
    minutes: 45,
    tag: "HEALING",
    musicChannelHint: ["治愈", "民谣", "深夜", "晚安"],
    searchFallbackQuery: "民谣 吉他 治愈",
    ambientMix: { campfire: 0.64, rain: 0.22 },
  },
  {
    id: "deep_ocean",
    title: "深海呼吸潮汐",
    subtitle: "周期潮汐浪涌 · 放空思考与长时段创意",
    mode: "countdown",
    minutes: 60,
    tag: "SLEEP",
    musicChannelHint: ["助眠", "放松", "安静", "冥想"],
    searchFallbackQuery: "sleep ambient 冥想",
    ambientMix: { ocean: 0.66, rain: 0.18 },
  },
  {
    id: "cafe_flow",
    title: "午后咖啡馆心流",
    subtitle: "节拍轻盈不抢耳 · 适合设计/创作输出",
    mode: "countdown",
    minutes: 60,
    tag: "CAFE",
    musicChannelHint: ["咖啡", "r&b", "灵魂乐", "都市"],
    searchFallbackQuery: "r&b chill cafe",
    ambientMix: { vinyl: 0.42, rain: 0.28 },
  },
  {
    id: "night_sleep",
    title: "月光温和入眠",
    subtitle: "深海低频白噪 · 温和渐隐陪伴入睡",
    mode: "countdown",
    minutes: 30,
    tag: "SLEEP",
    musicChannelHint: ["睡前", "摇篮", "晚安", "纯音乐"],
    searchFallbackQuery: "助眠 轻音乐 睡眠",
    ambientMix: { ocean: 0.54, rain: 0.42 },
  },
  {
    id: "free_stopwatch",
    title: "自由正计时",
    subtitle: "无倒计时压迫 · 实时累计高能专注时长",
    mode: "stopwatch",
    minutes: 0,
    tag: "STUDY",
    musicChannelHint: ["专注", "工作", "纯音乐"],
    searchFallbackQuery: "专注 纯音乐",
    ambientMix: { rain: 0.34, vinyl: 0.30 },
  },
] as const;

const FOCUS_DATE_KEY = "bm_focus_date";
const FOCUS_TODAY_COUNT_KEY = "bm_focus_count";
const FOCUS_TODAY_MINS_KEY = "bm_focus_mins";
const FOCUS_ALL_COUNT_KEY = "bm_focus_total_sessions";
const FOCUS_ALL_MINS_KEY = "bm_focus_total_mins";
const FOCUS_AUTO_MUSIC_KEY = "bm_focus_auto_music_v1";
const FOCUS_AUTO_AMBIENT_KEY = "bm_focus_auto_ambient_v1";
const FOCUS_LAST_PRESET_KEY = "bm_focus_preset_v1";

function getLocalDateKey(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export interface FocusPodModuleDeps {
  fmChannels: Ref<FmChannel[]>;
  isPlaying: Ref<boolean>;
  notify: (msg: string) => void;
  playSongFromList: (song: Song, list?: Song[], queueTitle?: string) => Promise<void>;
}

/**
 * 桌面心流舱状态机（完整迁移 Android FocusPodManager.kt 并增强双栏控制与番茄自动循环）
 */
export function createFocusPodModule(deps: FocusPodModuleDeps) {
  const { fmChannels, isPlaying, notify, playSongFromList } = deps;

  const initialDate = getLocalDateKey();
  const savedDate = localStorage.getItem(FOCUS_DATE_KEY) || "";
  if (savedDate !== initialDate) {
    localStorage.setItem(FOCUS_DATE_KEY, initialDate);
    localStorage.setItem(FOCUS_TODAY_COUNT_KEY, "0");
    localStorage.setItem(FOCUS_TODAY_MINS_KEY, "0");
  }

  const focusTodayCompletedCount = ref<number>(
    savedDate === initialDate ? Number(localStorage.getItem(FOCUS_TODAY_COUNT_KEY) || 0) : 0
  );
  const focusTodayTotalMinutes = ref<number>(
    savedDate === initialDate ? Number(localStorage.getItem(FOCUS_TODAY_MINS_KEY) || 0) : 0
  );
  const focusAllTimeCompletedCount = ref<number>(
    Number(localStorage.getItem(FOCUS_ALL_COUNT_KEY) || 0)
  );
  const focusAllTimeTotalMinutes = ref<number>(
    Number(localStorage.getItem(FOCUS_ALL_MINS_KEY) || 0)
  );

  const savedPresetId = localStorage.getItem(FOCUS_LAST_PRESET_KEY) || "pomodoro_25";
  const initialPreset =
    FOCUS_SCENE_PRESETS.find((p) => p.id === savedPresetId) || FOCUS_SCENE_PRESETS[0];

  const activePresetId = ref<string>(initialPreset.id);
  const focusMode = ref<FocusModeType>(initialPreset.mode);
  const focusRunState = ref<FocusRunState>("idle");
  const focusTargetMinutes = ref<number>(initialPreset.minutes || 25);
  const focusRemainingSec = ref<number>(
    initialPreset.mode === "stopwatch" ? 0 : (initialPreset.minutes || 25) * 60
  );
  const focusElapsedSec = ref<number>(0);

  // 番茄钟阶段状态
  const pomodoroFocusMinutes = ref<number>(25);
  const pomodoroShortBreakMinutes = ref<number>(5);
  const pomodoroLongBreakMinutes = ref<number>(15);
  const pomodoroPhase = ref<PomodoroPhase>("focus");
  const pomodoroCompletedRounds = ref<number>(0);

  // 智能协同开关
  const autoPlaySceneMusic = ref<boolean>(
    localStorage.getItem(FOCUS_AUTO_MUSIC_KEY) !== "false"
  );
  const autoMixAmbient = ref<boolean>(
    localStorage.getItem(FOCUS_AUTO_AMBIENT_KEY) !== "false"
  );
  const isFetchingSceneMusic = ref<boolean>(false);

  // 兼容旧版 appStore 字段引用（双向桥接单例）
  const ambientSyncWithMusic = computed<boolean>({
    get: () => ambientSoundEngine.state.syncWithMusic,
    set: (val: boolean) => ambientSoundEngine.setSyncWithMusic(val),
  });

  const activePreset = computed<FocusScenePreset>(
    () =>
      FOCUS_SCENE_PRESETS.find((p) => p.id === activePresetId.value) ||
      FOCUS_SCENE_PRESETS[0]
  );

  const formattedClock = computed(() => {
    const sec =
      focusMode.value === "stopwatch"
        ? Math.max(0, Math.floor(focusElapsedSec.value))
        : Math.max(0, Math.floor(focusRemainingSec.value));
    const hrs = Math.floor(sec / 3600);
    const mins = Math.floor((sec % 3600) / 60);
    const rem = sec % 60;
    if (hrs > 0) {
      return `${hrs}:${String(mins).padStart(2, "0")}:${String(rem).padStart(2, "0")}`;
    }
    return `${String(mins).padStart(2, "0")}:${String(rem).padStart(2, "0")}`;
  });

  const progressRatio = computed(() => {
    if (focusMode.value === "stopwatch") {
      // 正计时按 60 分钟呼吸满圈循环
      return Math.min(1, ((focusElapsedSec.value % 3600) || 0) / 3600);
    }
    const totalSec = Math.max(1, getPhaseTotalSeconds());
    return Math.min(1, Math.max(0, 1 - focusRemainingSec.value / totalSec));
  });

  function getPhaseTotalSeconds(): number {
    if (focusMode.value === "pomodoro") {
      if (pomodoroPhase.value === "short_break") return pomodoroShortBreakMinutes.value * 60;
      if (pomodoroPhase.value === "long_break") return pomodoroLongBreakMinutes.value * 60;
      return pomodoroFocusMinutes.value * 60;
    }
    return Math.max(1, focusTargetMinutes.value) * 60;
  }

  function syncStatsForCurrentDay() {
    const today = getLocalDateKey();
    const curSaved = localStorage.getItem(FOCUS_DATE_KEY) || "";
    if (curSaved !== today) {
      focusTodayCompletedCount.value = 0;
      focusTodayTotalMinutes.value = 0;
      localStorage.setItem(FOCUS_DATE_KEY, today);
      localStorage.setItem(FOCUS_TODAY_COUNT_KEY, "0");
      localStorage.setItem(FOCUS_TODAY_MINS_KEY, "0");
    }
  }

  function persistStats() {
    syncStatsForCurrentDay();
    const today = getLocalDateKey();
    localStorage.setItem(FOCUS_DATE_KEY, today);
    localStorage.setItem(FOCUS_TODAY_COUNT_KEY, String(focusTodayCompletedCount.value));
    localStorage.setItem(FOCUS_TODAY_MINS_KEY, String(focusTodayTotalMinutes.value));
    localStorage.setItem(FOCUS_ALL_COUNT_KEY, String(focusAllTimeCompletedCount.value));
    localStorage.setItem(FOCUS_ALL_MINS_KEY, String(focusAllTimeTotalMinutes.value));
  }

  function addFocusedMinute(mins = 1) {
    if (mins <= 0) return;
    syncStatsForCurrentDay();
    focusTodayTotalMinutes.value += mins;
    focusAllTimeTotalMinutes.value += mins;
    persistStats();
  }

  function recordCompletedFocusSession(minutes: number) {
    syncStatsForCurrentDay();
    focusTodayCompletedCount.value += 1;
    focusAllTimeCompletedCount.value += 1;
    if (minutes > 0) {
      // 若尚未按分钟 tick 记完则兜底校准（通常 minuteAccumulator 已实时入账）
      const m = Math.max(0, Math.round(minutes));
      if (m > 0 && focusElapsedSec.value < 55) {
        focusTodayTotalMinutes.value += m;
        focusAllTimeTotalMinutes.value += m;
      }
    }
    persistStats();
  }

  let tickerId: number | null = null;
  let uncreditedFocusedSec = 0;

  function stopTicker() {
    if (tickerId !== null) {
      window.clearInterval(tickerId);
      tickerId = null;
    }
  }

  async function triggerSceneMusicPlayback(preset: FocusScenePreset) {
    if (!autoPlaySceneMusic.value || isFetchingSceneMusic.value) return;
    isFetchingSceneMusic.value = true;
    try {
      let songs: Song[] = [];
      let queueName = `心流 · ${preset.title}`;

      if (fmChannels.value.length > 0) {
        const matched = fmChannels.value.find((ch) => {
          const hay = `${ch.name} ${ch.category} ${ch.description || ""}`.toLowerCase();
          return preset.musicChannelHint.some((h) => hay.includes(h.toLowerCase()));
        });
        if (matched) {
          songs = await kugouClient.getFmSongs(matched.id).catch(() => []);
          if (songs.length > 0) {
            queueName = `心流调频 · ${matched.name}`;
          }
        }
      }

      if (songs.length === 0) {
        songs = await kugouClient.searchSongs(preset.searchFallbackQuery, 1, 24).catch(() => []);
      }

      if (songs.length > 0) {
        await playSongFromList(songs[0], songs, queueName);
      }
    } catch {
      // silent fallback
    } finally {
      isFetchingSceneMusic.value = false;
    }
  }

  function handlePhaseCompleted() {
    ambientSoundEngine.playGentleBellChime();

    if (focusMode.value === "pomodoro") {
      if (pomodoroPhase.value === "focus") {
        pomodoroCompletedRounds.value += 1;
        focusTodayCompletedCount.value += 1;
        focusAllTimeCompletedCount.value += 1;
        persistStats();

        const isLong = pomodoroCompletedRounds.value % 4 === 0;
        pomodoroPhase.value = isLong ? "long_break" : "short_break";
        const breakMins = isLong
          ? pomodoroLongBreakMinutes.value
          : pomodoroShortBreakMinutes.value;
        focusRemainingSec.value = breakMins * 60;
        focusElapsedSec.value = 0;
        uncreditedFocusedSec = 0;
        notify(
          isLong
            ? `🍅 完成第 ${pomodoroCompletedRounds.value} 轮番茄专注！进入 ${breakMins} 分钟长休`
            : `🍅 完成 1 轮番茄专注！进入 ${breakMins} 分钟小憩`
        );
      } else {
        pomodoroPhase.value = "focus";
        focusRemainingSec.value = pomodoroFocusMinutes.value * 60;
        focusElapsedSec.value = 0;
        uncreditedFocusedSec = 0;
        notify(`✨ 休息结束，自动开启新一轮 ${pomodoroFocusMinutes.value} 分钟番茄专注`);
      }
      return;
    }

    // 普通倒计时完成
    stopTicker();
    focusRunState.value = "completed";
    focusTodayCompletedCount.value += 1;
    focusAllTimeCompletedCount.value += 1;
    persistStats();
    notify(`🎯 已完成「${activePreset.value.title}」心流专注周期！`);
  }

  function startTicker() {
    stopTicker();
    tickerId = window.setInterval(() => {
      if (focusRunState.value !== "running") return;

      const countsTowardsFocusStats =
        focusMode.value !== "pomodoro" || pomodoroPhase.value === "focus";

      if (countsTowardsFocusStats) {
        uncreditedFocusedSec += 1;
        if (uncreditedFocusedSec >= 60) {
          uncreditedFocusedSec -= 60;
          addFocusedMinute(1);
        }
      }

      if (focusMode.value === "stopwatch") {
        focusElapsedSec.value += 1;
        return;
      }

      focusElapsedSec.value += 1;
      if (focusRemainingSec.value > 1) {
        focusRemainingSec.value -= 1;
      } else {
        focusRemainingSec.value = 0;
        handlePhaseCompleted();
      }
    }, 1000);
  }

  function applyScenePreset(presetId: string, options?: { autoStart?: boolean }) {
    const found = FOCUS_SCENE_PRESETS.find((p) => p.id === presetId);
    if (!found) return;
    activePresetId.value = found.id;
    localStorage.setItem(FOCUS_LAST_PRESET_KEY, found.id);

    stopTicker();
    focusRunState.value = "idle";
    focusMode.value = found.mode;
    pomodoroPhase.value = "focus";
    pomodoroCompletedRounds.value = 0;
    focusTargetMinutes.value = found.minutes || 25;
    focusElapsedSec.value = 0;
    uncreditedFocusedSec = 0;
    focusRemainingSec.value =
      found.mode === "stopwatch" ? 0 : (found.minutes || 25) * 60;

    if (autoMixAmbient.value) {
      ambientSoundEngine.applySceneMix(found.ambientMix);
    }

    if (options?.autoStart) {
      startFocusSession();
    } else if (autoPlaySceneMusic.value) {
      triggerSceneMusicPlayback(found);
    }
  }

  function switchFocusMode(nextMode: FocusModeType) {
    if (focusMode.value === nextMode && focusRunState.value === "idle") return;
    stopTicker();
    focusRunState.value = "idle";
    focusMode.value = nextMode;
    pomodoroPhase.value = "focus";
    focusElapsedSec.value = 0;
    uncreditedFocusedSec = 0;
    if (nextMode === "pomodoro") {
      focusRemainingSec.value = pomodoroFocusMinutes.value * 60;
    } else if (nextMode === "stopwatch") {
      focusRemainingSec.value = 0;
    } else {
      const m = focusTargetMinutes.value > 0 ? focusTargetMinutes.value : 45;
      focusTargetMinutes.value = m;
      focusRemainingSec.value = m * 60;
    }
  }

  function setCustomCountdownMinutes(minutes: number) {
    const safe = Math.min(180, Math.max(5, Math.round(minutes)));
    focusTargetMinutes.value = safe;
    if (focusMode.value === "stopwatch") {
      focusMode.value = "countdown";
    }
    if (focusRunState.value === "idle" || focusRunState.value === "completed") {
      focusRemainingSec.value = safe * 60;
      focusElapsedSec.value = 0;
      uncreditedFocusedSec = 0;
      focusRunState.value = "idle";
    }
  }

  function startFocusSession() {
    if (focusRunState.value === "running") return;
    if (focusRunState.value === "completed") {
      focusElapsedSec.value = 0;
      uncreditedFocusedSec = 0;
      focusRemainingSec.value =
        focusMode.value === "stopwatch" ? 0 : getPhaseTotalSeconds();
    }
    focusRunState.value = "running";
    if (autoMixAmbient.value && ambientSoundEngine.activeCount === 0) {
      ambientSoundEngine.applySceneMix(activePreset.value.ambientMix);
    } else if (ambientSoundEngine.activeCount > 0) {
      ambientSoundEngine.setMasterEnabled(true);
    }
    if (autoPlaySceneMusic.value && !isPlaying.value) {
      triggerSceneMusicPlayback(activePreset.value);
    }
    startTicker();
  }

  function pauseFocusSession() {
    if (focusRunState.value !== "running") return;
    focusRunState.value = "paused";
    stopTicker();
  }

  function resumeFocusSession() {
    if (focusRunState.value !== "paused") return;
    focusRunState.value = "running";
    startTicker();
  }

  function toggleFocusRunPause() {
    if (focusRunState.value === "running") {
      pauseFocusSession();
    } else if (focusRunState.value === "paused") {
      resumeFocusSession();
    } else {
      startFocusSession();
    }
  }

  function stopAndResetFocusSession(completeStopwatch = true) {
    stopTicker();
    if (
      completeStopwatch &&
      focusMode.value === "stopwatch" &&
      focusElapsedSec.value >= 60
    ) {
      focusTodayCompletedCount.value += 1;
      focusAllTimeCompletedCount.value += 1;
      persistStats();
      notify(
        `✅ 正计时结束，本次共专注 ${Math.floor(focusElapsedSec.value / 60)} 分钟`
      );
    }
    focusRunState.value = "idle";
    pomodoroPhase.value = "focus";
    focusElapsedSec.value = 0;
    uncreditedFocusedSec = 0;
    focusRemainingSec.value =
      focusMode.value === "stopwatch" ? 0 : getPhaseTotalSeconds();
  }

  function skipPomodoroPhase() {
    if (focusMode.value !== "pomodoro") return;
    handlePhaseCompleted();
  }

  function setAutoPlaySceneMusic(val: boolean) {
    autoPlaySceneMusic.value = Boolean(val);
    localStorage.setItem(FOCUS_AUTO_MUSIC_KEY, String(autoPlaySceneMusic.value));
  }

  function setAutoMixAmbient(val: boolean) {
    autoMixAmbient.value = Boolean(val);
    localStorage.setItem(FOCUS_AUTO_AMBIENT_KEY, String(autoMixAmbient.value));
  }

  return {
    focusTodayCompletedCount,
    focusTodayTotalMinutes,
    focusAllTimeCompletedCount,
    focusAllTimeTotalMinutes,
    ambientSyncWithMusic,
    recordCompletedFocusSession,
    activePresetId,
    activePreset,
    focusMode,
    focusRunState,
    focusTargetMinutes,
    focusRemainingSec,
    focusElapsedSec,
    pomodoroFocusMinutes,
    pomodoroShortBreakMinutes,
    pomodoroLongBreakMinutes,
    pomodoroPhase,
    pomodoroCompletedRounds,
    autoPlaySceneMusic,
    autoMixAmbient,
    isFetchingSceneMusic,
    formattedClock,
    progressRatio,
    applyScenePreset,
    switchFocusMode,
    setCustomCountdownMinutes,
    startFocusSession,
    pauseFocusSession,
    resumeFocusSession,
    toggleFocusRunPause,
    stopAndResetFocusSession,
    skipPomodoroPhase,
    setAutoPlaySceneMusic,
    setAutoMixAmbient,
    triggerSceneMusicPlayback,
  };
}
