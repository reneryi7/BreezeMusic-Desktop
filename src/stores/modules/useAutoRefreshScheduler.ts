import { ref, type ComputedRef, type Ref } from "vue";

export interface AutoRefreshSchedulerDeps {
  isLoggedIn: ComputedRef<boolean>;
  isStandaloneSubWindow: boolean;
  refreshAccountProfileAndVipSilent: (isStartup?: boolean) => Promise<void>;
  loadDailyRecommend: (notifyUser?: boolean) => Promise<void>;
  loadPersonalFm: (append?: boolean, notifyUser?: boolean) => Promise<unknown>;
  loadNewSongExpress: (period?: string, options?: { force?: boolean; notifyUser?: boolean }) => Promise<void>;
  refreshAllNewSongTags: (notifyUser?: boolean) => Promise<void>;
  loadRankList: (notifyUser?: boolean) => Promise<void>;
  loadFmChannels: () => Promise<void>;
  loadUserPlaylists: () => Promise<void>;
  loadRecentHistory: () => Promise<void>;
  loadFollowedArtists: () => Promise<void>;
  loadPurchasedAssets: () => Promise<void>;
  loadUserCloudDisk: (force?: boolean, silent?: boolean) => Promise<void>;
  loadSearchDiscovery: (force?: boolean) => Promise<void>;
  scanLocalMusicLibrary: (customDir?: string, silent?: boolean) => Promise<void>;
  shouldAutoScanLocalMusic?: () => boolean;
  activeNewSongPeriod: Ref<string>;
}

const ONE_HOUR_MS = 60 * 60 * 1000;
const CLOCK_TICK_INTERVAL_MS = 60 * 1000;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function yieldToIdle(timeoutMs = 800): Promise<void> {
  return new Promise((resolve) => {
    if (typeof window !== "undefined" && typeof (window as any).requestIdleCallback === "function") {
      (window as any).requestIdleCallback(() => resolve(), { timeout: timeoutMs });
    } else {
      window.setTimeout(resolve, 60);
    }
  });
}

function isUserMidBatchEdit(): boolean {
  if (typeof document === "undefined") return false;
  return Boolean(document.querySelector(".batch-floating-action-bar"));
}

/**
 * 全局分阶段启动全量刷新 + 每小时后台静默刷新调度器 + 响应式分钟时钟
 * - 启动时分三波错峰加载（T+0 首屏关键 -> T+450 个人曲库 -> T+1200 低优探索与增量本地扫描），避免瞬间风暴卡顿
 * - 后续每隔 1 小时（含休眠唤醒补偿）按低并发错峰节流静默刷新全模块，零骨架闪烁、零打断播放、零弹窗打扰
 */
export function createAutoRefreshScheduler(deps: AutoRefreshSchedulerDeps) {
  const nowClockTick = ref<number>(Date.now());
  const lastFullRefreshAt = ref<number>(0);
  const isBackgroundRefreshing = ref<boolean>(false);

  let hourlyTimerId: number | null = null;
  let clockTimerId: number | null = null;
  let wakeupTimerId: number | null = null;
  let startupStage2Timer: number | null = null;
  let startupStage3Timer: number | null = null;
  let refreshEpoch = 0;
  let visibilityBound = false;

  function tickClockNow() {
    nowClockTick.value = Date.now();
  }

  async function runStaggeredTasks(
    tasks: Array<() => Promise<unknown>>,
    epoch: number,
    spacingMs = 280
  ) {
    for (const task of tasks) {
      if (epoch !== refreshEpoch) return;
      await yieldToIdle(600);
      if (epoch !== refreshEpoch) return;
      try {
        await task();
      } catch {
        // 后台静默刷新单任务失败绝不阻断其余模块，亦不打扰用户
      }
      if (epoch !== refreshEpoch) return;
      if (spacingMs > 0) {
        await sleep(spacingMs);
      }
    }
  }

  /**
   * 启动时分阶段全量刷新：立即渲染首屏关键数据，再错峰预热个人资产、云盘、搜索发现与本地缓存扫描
   */
  function runStartupStaggeredRefresh() {
    if (deps.isStandaloneSubWindow) return;
    const epoch = ++refreshEpoch;
    lastFullRefreshAt.value = Date.now();

    // 第 1 波 (T+0ms)：首屏发现页最核心数据 + 登录态账户/VIP 信息
    void Promise.allSettled([
      deps.isLoggedIn.value ? deps.refreshAccountProfileAndVipSilent(true) : Promise.resolve(),
      deps.loadDailyRecommend(false),
      deps.loadRankList(false),
      deps.loadNewSongExpress(deps.activeNewSongPeriod.value, { force: true, notifyUser: false }),
    ]);

    // 第 2 波 (T+450ms)：个人曲库与资产（歌单、我喜欢、最近播放、关注歌手、已购、云盘）
    if (startupStage2Timer !== null) window.clearTimeout(startupStage2Timer);
    startupStage2Timer = window.setTimeout(() => {
      startupStage2Timer = null;
      if (epoch !== refreshEpoch) return;
      const stage2Tasks: Array<() => Promise<unknown>> = [
        () => deps.loadRecentHistory(),
      ];
      if (deps.isLoggedIn.value) {
        stage2Tasks.push(
          () => deps.loadUserPlaylists(),
          () => deps.loadFollowedArtists(),
          () => deps.loadPurchasedAssets(),
          () => deps.loadUserCloudDisk(true, true)
        );
      }
      void runStaggeredTasks(stage2Tasks, epoch, 140);
    }, 450);

    // 第 3 波 (T+1250ms)：低优后台预热（私人漫游 FM、全语种新歌、电台广场、搜索热榜、本地曲库增量扫描）
    if (startupStage3Timer !== null) window.clearTimeout(startupStage3Timer);
    startupStage3Timer = window.setTimeout(() => {
      startupStage3Timer = null;
      if (epoch !== refreshEpoch) return;
      const stage3Tasks: Array<() => Promise<unknown>> = [
        () => deps.loadPersonalFm(false, false),
        () => deps.loadFmChannels(),
        () => deps.refreshAllNewSongTags(false),
        () => deps.loadSearchDiscovery(false),
      ];
      if (!deps.shouldAutoScanLocalMusic || deps.shouldAutoScanLocalMusic()) {
        stage3Tasks.push(() => deps.scanLocalMusicLibrary(undefined, true));
      }
      void runStaggeredTasks(stage3Tasks, epoch, 220);
    }, 1250);
  }

  /**
   * 周期性（每小时）后台全量静默刷新：低并发错峰推进，绝不抢占音频线程与主线程渲染帧
   */
  async function runHourlySilentRefresh() {
    if (deps.isStandaloneSubWindow) return;
    if (typeof document !== "undefined" && document.visibilityState === "hidden") return;
    if (isBackgroundRefreshing.value) return;
    // 若用户正在执行批量多选编辑，延后本次刷新避免列表状态抖动
    if (isUserMidBatchEdit()) {
      window.setTimeout(() => {
        if (!isUserMidBatchEdit() && Date.now() - lastFullRefreshAt.value >= ONE_HOUR_MS - 30_000) {
          void runHourlySilentRefresh();
        }
      }, 60_000);
      return;
    }

    const epoch = refreshEpoch;
    isBackgroundRefreshing.value = true;
    lastFullRefreshAt.value = Date.now();
    tickClockNow();

    try {
      const tasks: Array<() => Promise<unknown>> = [];
      if (deps.isLoggedIn.value) {
        tasks.push(() => deps.refreshAccountProfileAndVipSilent(false));
      }
      tasks.push(
        () => deps.loadDailyRecommend(false),
        () => deps.loadRankList(false),
        () => deps.refreshAllNewSongTags(false),
        () => deps.loadFmChannels(),
        () => deps.loadSearchDiscovery(true),
        () => deps.loadRecentHistory()
      );
      if (deps.isLoggedIn.value) {
        tasks.push(
          () => deps.loadUserPlaylists(),
          () => deps.loadFollowedArtists(),
          () => deps.loadPurchasedAssets(),
          () => deps.loadUserCloudDisk(true, true)
        );
      }
      if (!deps.shouldAutoScanLocalMusic || deps.shouldAutoScanLocalMusic()) {
        tasks.push(() => deps.scanLocalMusicLibrary(undefined, true));
      }

      await runStaggeredTasks(tasks, epoch, 300);
    } finally {
      isBackgroundRefreshing.value = false;
    }
  }

  function startClockTimer() {
    if (typeof window === "undefined") return;
    if (clockTimerId === null) {
      clockTimerId = window.setInterval(tickClockNow, CLOCK_TICK_INTERVAL_MS);
    }
  }

  function stopClockTimer() {
    if (clockTimerId !== null) {
      window.clearInterval(clockTimerId);
      clockTimerId = null;
    }
  }

  function handleVisibilityChange() {
    if (typeof document === "undefined") return;
    if (document.visibilityState === "hidden") {
      stopClockTimer();
      return;
    }
    if (document.visibilityState !== "visible") return;
    tickClockNow();
    startClockTimer();
    const elapsed = Date.now() - lastFullRefreshAt.value;
    if (lastFullRefreshAt.value > 0 && elapsed >= ONE_HOUR_MS) {
      if (wakeupTimerId !== null) window.clearTimeout(wakeupTimerId);
      // 休眠唤醒后等待 2 秒让系统网卡与音频设备恢复就绪，再静默补刷
      wakeupTimerId = window.setTimeout(() => {
        wakeupTimerId = null;
        void runHourlySilentRefresh();
      }, 2000);
    }
  }

  function startScheduler() {
    if (deps.isStandaloneSubWindow || typeof window === "undefined") return;

    if (typeof document === "undefined" || document.visibilityState === "visible") {
      startClockTimer();
    }
    if (hourlyTimerId === null) {
      hourlyTimerId = window.setInterval(() => {
        void runHourlySilentRefresh();
      }, ONE_HOUR_MS);
    }
    if (!visibilityBound && typeof document !== "undefined") {
      document.addEventListener("visibilitychange", handleVisibilityChange);
      visibilityBound = true;
    }
  }

  function resetCycleForAuthChange() {
    refreshEpoch += 1;
    lastFullRefreshAt.value = Date.now();
    if (hourlyTimerId !== null) {
      window.clearInterval(hourlyTimerId);
      hourlyTimerId = window.setInterval(() => {
        void runHourlySilentRefresh();
      }, ONE_HOUR_MS);
    }
  }

  function stopScheduler() {
    refreshEpoch += 1;
    if (hourlyTimerId !== null) {
      window.clearInterval(hourlyTimerId);
      hourlyTimerId = null;
    }
    if (clockTimerId !== null) {
      window.clearInterval(clockTimerId);
      clockTimerId = null;
    }
    if (wakeupTimerId !== null) {
      window.clearTimeout(wakeupTimerId);
      wakeupTimerId = null;
    }
    if (startupStage2Timer !== null) {
      window.clearTimeout(startupStage2Timer);
      startupStage2Timer = null;
    }
    if (startupStage3Timer !== null) {
      window.clearTimeout(startupStage3Timer);
      startupStage3Timer = null;
    }
    if (visibilityBound && typeof document !== "undefined") {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      visibilityBound = false;
    }
  }

  return {
    nowClockTick,
    lastFullRefreshAt,
    isBackgroundRefreshing,
    startScheduler,
    stopScheduler,
    runStartupStaggeredRefresh,
    runHourlySilentRefresh,
    resetCycleForAuthChange,
  };
}
