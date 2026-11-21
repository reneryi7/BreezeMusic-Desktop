import { ref, computed, reactive, type Ref } from "vue";
import { kugouClient } from "../../api/kugou";
import type {
  DiscoverSubView,
  FmChannel,
  MainNavTab,
  RankItem,
  Song,
} from "../../types";

export interface NewSongAreaOption {
  id: string;
  label: string;
  color: string;
}

export const NEW_SONG_AREAS: readonly NewSongAreaOption[] = [
  { id: "CHINESE", label: "华语", color: "#7C6AF6" },
  { id: "CANTONESE", label: "粤语", color: "#00c4a7" },
  { id: "WESTERN", label: "欧美", color: "#ff6b6b" },
  { id: "JPKR", label: "日韩", color: "#15c493" },
  { id: "ACG", label: "ACG", color: "#9688ff" },
  { id: "OST", label: "影视", color: "#f5a623" },
];

export interface DiscoverAndFmDeps {
  activeTab: Ref<MainNavTab>;
  isNowPlayingOpen: Ref<boolean>;
  playQueue: Ref<Song[]>;
  currentIndex: Ref<number>;
  durationMs: Ref<number>;
  notify: (msg: string) => void;
  playSongFromList: (song: Song, list?: Song[], queueTitle?: string) => Promise<void>;
  pushNavHistory?: () => void;
}

export function createDiscoverAndFmModule(deps: DiscoverAndFmDeps) {
  const {
    activeTab,
    isNowPlayingOpen,
    playQueue,
    currentIndex,
    durationMs,
    notify,
    playSongFromList,
    pushNavHistory,
  } = deps;

  // ================= 2. 发现页与调频台全量状态 =================
  const discoverSubView = ref<DiscoverSubView>("home");
  const discoverHeroExpanding = ref<"none" | "daily" | "fm">("none");
  const isGlobalQueueOpen = ref(false);

  const recommendSongs = ref<Song[]>([]);
  const isLoadingRecommend = ref(false);

  const personalFmSongs = ref<Song[]>([]);
  const personalFmIndex = ref(0);
  const isLoadingFm = ref(false);
  const isAppendingFmQueue = ref(false);
  const isPersonalFmDrawerOpen = ref(false);

  const currentFmSong = computed<Song | null>(
    () => personalFmSongs.value[personalFmIndex.value] || null
  );
  const nextFmSong = computed<Song | null>(
    () =>
      personalFmSongs.value[
        (personalFmIndex.value + 1) % Math.max(1, personalFmSongs.value.length)
      ] || null
  );

  const newSongPeriod = ref<string>("CHINESE");
  const newSongs = ref<Song[]>([]);
  const newSongsByPeriod = ref<Record<string, Song[]>>({});
  const isLoadingNewSongs = ref(false);
  const isRefreshingAllNewSongs = ref(false);

  const rankList = ref<RankItem[]>([]);
  const isLoadingRanks = ref(false);
  const isRankGridModalOpen = ref(false);
  const selectedRankCategory = ref<string>("全部");
  const activeRank = ref<RankItem | null>(null);
  const activeRankSongs = ref<Song[]>([]);
  const isLoadingRankSongs = ref(false);
  const isLoadingMoreRankSongs = ref(false);
  const rankSongsCache = new Map<number, Song[]>();
  let rankLoadEpoch = 0;

  const fmChannels = ref<FmChannel[]>([]);
  const selectedFmCategory = ref<string>("全部");
  const activeFmChannel = ref<FmChannel | null>(null);
  const fmStationSongs = ref<Song[]>([]);
  const isLoadingFmStation = ref(false);


  async function loadDailyRecommend(notifyUser = false) {
    if (isLoadingRecommend.value) return;
    isLoadingRecommend.value = true;
    try {
      recommendSongs.value = await kugouClient.getDailyRecommend();
      if (playQueue.value.length === 0 && recommendSongs.value.length > 0) {
        playQueue.value = [...recommendSongs.value];
        currentIndex.value = 0;
        const first = recommendSongs.value[0];
        durationMs.value = first.durationMs || 210000;
      }
      if (notifyUser) {
        notify(`已刷新每日推荐 · 共 ${recommendSongs.value.length} 首精选曲目 ✨`);
      }
    } catch (e) {
      console.warn("loadDailyRecommend error:", e);
      if (notifyUser) {
        notify("刷新每日推荐失败，请稍后重试");
      }
    } finally {
      isLoadingRecommend.value = false;
    }
  }

  const fmHistoryHashes = ref<string[]>(
    JSON.parse(localStorage.getItem("bm_fm_history_hashes") || "[]")
  );
  function recordFmHistoryHashes(songs: Song[]) {
    const incoming = songs
      .map((s) => (s.hash || "").toUpperCase())
      .filter((h) => h.length > 0);
    if (incoming.length === 0) return;
    const merged = Array.from(new Set([...incoming, ...fmHistoryHashes.value])).slice(0, 240);
    fmHistoryHashes.value = merged;
    localStorage.setItem("bm_fm_history_hashes", JSON.stringify(merged));
  }

  async function loadPersonalFm(append = false, notifyUser = true): Promise<Song[]> {
    isLoadingFm.value = true;
    try {
      const currentExclude = Array.from(
        new Set([
          ...personalFmSongs.value.map((s) => (s.hash || "").toUpperCase()),
          ...fmHistoryHashes.value.slice(0, 120),
        ])
      ).filter(Boolean);
      const songs = await kugouClient.getPersonalFm(currentExclude);
      if (songs.length > 0) {
        recordFmHistoryHashes(songs);
        if (append) {
          const seen = new Set(personalFmSongs.value.map((s) => (s.hash || "").toUpperCase()));
          const unique = songs.filter((s) => !seen.has((s.hash || "").toUpperCase()));
          const added = unique.length > 0 ? unique : songs;
          personalFmSongs.value = [...personalFmSongs.value, ...added];
          if (notifyUser) {
            notify(`已为猜你喜欢随心听无缝焕新 ${added.length} 首歌曲`);
          }
          return added;
        } else {
          personalFmSongs.value = songs;
          personalFmIndex.value = 0;
          return songs;
        }
      }
      return [];
    } catch (e) {
      console.warn("loadPersonalFm error:", e);
      return [];
    } finally {
      isLoadingFm.value = false;
    }
  }

  /**
   * 仅拉取新一批去重后的「猜你喜欢」推荐曲目供追加到播放队列使用：
   * - 使用独立加载态 `isAppendingFmQueue`（绝不触发换一批刷新图标旋转）
   * - 绝不修改或刷新 `personalFmSongs` 页面列表（页面始终保持原 24 首稳定展示）
   */
  async function fetchMorePersonalFmForQueue(extraExcludeHashes: string[] = []): Promise<Song[]> {
    if (isAppendingFmQueue.value) return [];
    isAppendingFmQueue.value = true;
    try {
      const currentExclude = Array.from(
        new Set([
          ...personalFmSongs.value.map((s) => (s.hash || "").toUpperCase()),
          ...playQueue.value.map((s) => (s.hash || "").toUpperCase()),
          ...extraExcludeHashes.map((h) => (h || "").toUpperCase()),
          ...fmHistoryHashes.value.slice(0, 120),
        ])
      ).filter(Boolean);
      const songs = await kugouClient.getPersonalFm(currentExclude);
      if (songs.length > 0) {
        recordFmHistoryHashes(songs);
        const existingQueueHashes = new Set(
          playQueue.value.map((s) => (s.hash || "").toUpperCase()).filter(Boolean)
        );
        const fresh = songs.filter(
          (s) => !existingQueueHashes.has((s.hash || "").toUpperCase())
        );
        return fresh.length > 0 ? fresh : songs;
      }
      return [];
    } catch (e) {
      console.warn("fetchMorePersonalFmForQueue error:", e);
      return [];
    } finally {
      isAppendingFmQueue.value = false;
    }
  }

  async function nextPersonalFmTrack(autoPlay = true) {
    if (personalFmSongs.value.length === 0) {
      await loadPersonalFm(false, false);
    }
    if (personalFmSongs.value.length === 0) return;

    if (personalFmIndex.value + 1 < personalFmSongs.value.length) {
      personalFmIndex.value += 1;
    } else {
      await loadPersonalFm(false, false);
    }
    if (autoPlay && currentFmSong.value) {
      playSongFromList(currentFmSong.value, personalFmSongs.value, "猜你喜欢");
    }
  }

  async function loadNewSongExpress(
    period = "CHINESE",
    options?: { force?: boolean; notifyUser?: boolean }
  ) {
    const targetPeriod = period.trim().toUpperCase() || "CHINESE";
    newSongPeriod.value = targetPeriod;

    const cached = newSongsByPeriod.value[targetPeriod];
    if (!options?.force && cached && cached.length > 0) {
      newSongs.value = cached;
      return;
    }

    isLoadingNewSongs.value = true;
    try {
      const list = await kugouClient.getNewSongExpress(targetPeriod);
      newSongsByPeriod.value = {
        ...newSongsByPeriod.value,
        [targetPeriod]: list,
      };
      if (newSongPeriod.value === targetPeriod) {
        newSongs.value = list;
      }
      resolveFmChannelCovers();
      if (options?.notifyUser) {
        const areaLabel =
          NEW_SONG_AREAS.find((a) => a.id === targetPeriod)?.label || targetPeriod;
        notify(`已刷新「${areaLabel}」新歌速递 · 共 ${list.length} 首最近一周新曲 ✨`);
      }
    } catch (e) {
      console.warn("loadNewSongExpress error:", e);
      if (options?.notifyUser) {
        notify("刷新新歌速递失败，请稍后重试");
      }
    } finally {
      if (!isRefreshingAllNewSongs.value) {
        isLoadingNewSongs.value = false;
      }
    }
  }

  async function refreshAllNewSongTags(notifyUser = false) {
    if (isRefreshingAllNewSongs.value) return;
    isRefreshingAllNewSongs.value = true;
    isLoadingNewSongs.value = true;
    try {
      let totalLoaded = 0;
      await Promise.allSettled(
        NEW_SONG_AREAS.map(async (area) => {
          const list = await kugouClient.getNewSongExpress(area.id);
          totalLoaded += list.length;
          newSongsByPeriod.value = {
            ...newSongsByPeriod.value,
            [area.id]: list,
          };
          if (newSongPeriod.value === area.id) {
            newSongs.value = list;
          }
        })
      );
      const currentCached = newSongsByPeriod.value[newSongPeriod.value];
      if (currentCached) {
        newSongs.value = currentCached;
      }
      resolveFmChannelCovers();
      if (notifyUser) {
        notify(`已刷新全部 ${NEW_SONG_AREAS.length} 个新歌分类 · 同步 ${totalLoaded} 首近一周新歌 ✨`);
      }
    } catch (e) {
      console.warn("refreshAllNewSongTags error:", e);
      if (notifyUser) {
        notify("刷新新歌速递失败，请稍后重试");
      }
    } finally {
      isRefreshingAllNewSongs.value = false;
      isLoadingNewSongs.value = false;
    }
  }

  async function loadRankList(notifyUser = false) {
    if (isLoadingRanks.value) return;
    isLoadingRanks.value = true;
    try {
      rankList.value = await kugouClient.getRankList();
      resolveFmChannelCovers();
      if (notifyUser) {
        notify(`已刷新巅峰排行榜 · 同步 ${rankList.value.length} 个官方榜单 ✨`);
      }
    } catch (e) {
      console.warn("loadRankList error:", e);
      if (notifyUser) {
        notify("刷新排行榜失败，请稍后重试");
      }
    } finally {
      isLoadingRanks.value = false;
    }
  }

  const isStageScrolledDown = ref<boolean>(false);

  function scrollStageToTop() {
    isStageScrolledDown.value = false;
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("breeze:defer-stage-scroll-top"));
    }
  }

  function syncRankPreviewAndTotal(rankId: number, songs: Song[]) {
    if (songs.length === 0) return;
    const previews = songs.slice(0, 3).map((s) =>
      s.artist && s.artist !== "未知歌手" ? `${s.title} - ${s.artist}` : s.title
    );
    rankList.value = rankList.value.map((r) => {
      if (r.rankId !== rankId) return r;
      return {
        ...r,
        totalSongs: Math.max(r.totalSongs || 0, songs.length),
        topSongsPreview: previews.length > 0 ? previews : r.topSongsPreview,
        songPreviews: previews.length > 0 ? previews : r.songPreviews,
      };
    });
    if (activeRank.value && activeRank.value.rankId === rankId) {
      activeRank.value = {
        ...activeRank.value,
        totalSongs: Math.max(activeRank.value.totalSongs || 0, songs.length),
      };
    }
  }

  async function openRankDetail(rank: RankItem, forceRefresh = false) {
    const myEpoch = ++rankLoadEpoch;
    if (
      activeTab.value !== "discover" ||
      discoverSubView.value !== "rank" ||
      activeRank.value?.rankId !== rank.rankId
    ) {
      pushNavHistory?.();
    }
    activeRank.value = rank;
    discoverSubView.value = "rank";
    activeTab.value = "discover";
    isNowPlayingOpen.value = false;
    isRankGridModalOpen.value = false;
    scrollStageToTop();

    const cached = !forceRefresh ? rankSongsCache.get(rank.rankId) : undefined;
    const needsFull500 =
      rank.rankId === 8888 ||
      (rank.name || rank.rankName || "").includes("500") ||
      (rank.totalSongs || 0) > 100;

    if (cached && cached.length > 0) {
      activeRankSongs.value = cached;
      isLoadingRankSongs.value = false;
      if (!needsFull500 || cached.length >= 200) {
        isLoadingMoreRankSongs.value = false;
        return;
      }
    } else {
      activeRankSongs.value = [];
      isLoadingRankSongs.value = true;
    }

    try {
      // 第一阶段：极速获取首屏第 1 页（100 首），实现瞬时首屏渲染
      if (!cached || cached.length === 0) {
        const firstPage = await kugouClient.getRankSongs(rank.rankId, 1, 100);
        if (myEpoch !== rankLoadEpoch) return;
        activeRankSongs.value = firstPage;
        rankSongsCache.set(rank.rankId, firstPage);
        syncRankPreviewAndTotal(rank.rankId, firstPage);
        isLoadingRankSongs.value = false;

        // 若第 1 页曲目已不足 85 首且并非多页大榜，说明本榜已全部加载完毕
        if (firstPage.length < 85 && !needsFull500) {
          isLoadingMoreRankSongs.value = false;
          return;
        }
      }

      // 第二阶段：拉取全量曲目（最高 500 首，如酷狗 TOP500 完整获取 500 首）
      isLoadingMoreRankSongs.value = true;
      const fullSongs = await kugouClient.getRankSongs(rank.rankId, 1, 500);
      if (myEpoch !== rankLoadEpoch) return;
      if (fullSongs.length >= activeRankSongs.value.length) {
        activeRankSongs.value = fullSongs;
        rankSongsCache.set(rank.rankId, fullSongs);
        syncRankPreviewAndTotal(rank.rankId, fullSongs);
      }
    } catch (e: any) {
      if (myEpoch === rankLoadEpoch && activeRankSongs.value.length === 0) {
        notify(`加载榜单曲目失败：${String(e)}`);
      }
    } finally {
      if (myEpoch === rankLoadEpoch) {
        isLoadingRankSongs.value = false;
        isLoadingMoreRankSongs.value = false;
      }
    }
  }

  async function refreshActiveRank() {
    if (!activeRank.value) return;
    rankSongsCache.delete(activeRank.value.rankId);
    await openRankDetail(activeRank.value, true);
    notify(`已同步「${activeRank.value.name || activeRank.value.rankName}」最新完整曲库（${activeRankSongs.value.length} 首）✨`);
  }

  function resolveFmChannelCovers() {
    // 官方电台均已由后端返回真实独立的 fmlogov2 高清方形封面，仅在极端空值时兜底
    const pool: string[] = [];
    for (const s of recommendSongs.value) {
      if (s.coverUrl && !pool.includes(s.coverUrl)) pool.push(s.coverUrl);
    }
    if (pool.length === 0) return;
    fmChannels.value = fmChannels.value.map((ch, idx) => {
      if (ch.coverUrl && !ch.coverUrl.includes("20210519")) return ch;
      return {
        ...ch,
        coverUrl: pool[idx % pool.length] || "",
      };
    });
  }

  async function refreshDiscoverAll() {
    notify("正在全面焕新发现页内容...");
    await Promise.all([
      loadDailyRecommend(false),
      loadPersonalFm(false),
      refreshAllNewSongTags(false),
      loadRankList(false),
      loadFmChannels(),
    ]);
    resolveFmChannelCovers();
    notify("发现页推荐已全面焕新 ✨");
  }

  function openDailyRecommendPage() {
    if (activeTab.value === "discover" && discoverSubView.value === "daily") return;
    pushNavHistory?.();
    discoverHeroExpanding.value = "none";
    activeTab.value = "discover";
    discoverSubView.value = "daily";
    scrollStageToTop();
  }

  function openGuessYouLikePage() {
    if (activeTab.value === "discover" && discoverSubView.value === "fm_detail") return;
    pushNavHistory?.();
    if (personalFmSongs.value.length === 0) {
      loadPersonalFm(false);
    }
    discoverHeroExpanding.value = "none";
    activeTab.value = "discover";
    discoverSubView.value = "fm_detail";
    scrollStageToTop();
  }

  function openNewSongsPage() {
    if (activeTab.value === "discover" && discoverSubView.value === "new_songs") return;
    pushNavHistory?.();
    discoverHeroExpanding.value = "none";
    activeTab.value = "discover";
    discoverSubView.value = "new_songs";
    scrollStageToTop();
  }

  function openRadioPlazaPage() {
    if (activeTab.value === "discover" && discoverSubView.value === "radio") return;
    pushNavHistory?.();
    discoverHeroExpanding.value = "none";
    activeTab.value = "discover";
    discoverSubView.value = "radio";
    if (!activeFmChannel.value && fmChannels.value.length > 0) {
      activeFmChannel.value = fmChannels.value[0];
      loadFmStationSongs(fmChannels.value[0].id, false);
    }
    scrollStageToTop();
  }

  async function playRankDirectly(rank: RankItem) {
    try {
      const cached = rankSongsCache.get(rank.rankId);
      if (cached && cached.length > 0) {
        activeRank.value = rank;
        activeRankSongs.value = cached;
        playSongFromList(cached[0], cached, rank.name || rank.rankName);
        return;
      }
      notify(`正在加载「${rank.name || rank.rankName}」曲目...`);
      const firstBatch = await kugouClient.getRankSongs(rank.rankId, 1, 100);
      if (firstBatch.length > 0) {
        activeRank.value = rank;
        activeRankSongs.value = firstBatch;
        rankSongsCache.set(rank.rankId, firstBatch);
        syncRankPreviewAndTotal(rank.rankId, firstBatch);
        playSongFromList(firstBatch[0], firstBatch, rank.name || rank.rankName);

        // 后台静默补齐完整榜单曲目（如 TOP500 全量 500 首）
        if (firstBatch.length >= 85 || rank.rankId === 8888 || (rank.totalSongs || 0) > 100) {
          kugouClient
            .getRankSongs(rank.rankId, 1, 500)
            .then((fullSongs) => {
              if (fullSongs.length >= firstBatch.length) {
                rankSongsCache.set(rank.rankId, fullSongs);
                syncRankPreviewAndTotal(rank.rankId, fullSongs);
                if (activeRank.value?.rankId === rank.rankId) {
                  activeRankSongs.value = fullSongs;
                }
              }
            })
            .catch(() => {});
        }
      } else {
        notify(`「${rank.name || rank.rankName}」暂无可播曲目`);
      }
    } catch (e: any) {
      notify(`加载榜单失败：${String(e)}`);
    }
  }

  async function loadFmChannels() {
    try {
      const list = await kugouClient.getFmChannels();
      fmChannels.value = list;
      resolveFmChannelCovers();
      // 不再在发现页冷启动时强行将第一个频道设为 activeFmChannel，消除首张电台卡片的“特例背景光晕”与红字状态；
      // 仅当用户已进入电台广场二级页且尚未选中频道时才默认定位首频道
      if (discoverSubView.value === "radio" && !activeFmChannel.value && list.length > 0) {
        activeFmChannel.value = list[0];
        loadFmStationSongs(list[0].id, false);
      }
    } catch (e) {
      console.warn("loadFmChannels error:", e);
    }
  }

  async function loadFmStationSongs(
    channelId: string,
    append = false,
    autoPlay = false
  ) {
    const found = fmChannels.value.find((c) => c.id === channelId);
    if (found) activeFmChannel.value = found;
    isLoadingFmStation.value = true;
    try {
      const batch = await kugouClient.getFmSongs(channelId);
      if (append) {
        const seen = new Set(fmStationSongs.value.map((s) => s.hash));
        const extra = batch.filter((s) => !seen.has(s.hash));
        fmStationSongs.value = [...fmStationSongs.value, ...extra];
        notify(
          `已追加 ${extra.length} 首「${activeFmChannel.value?.name || "电台"}」漫游新曲`
        );
      } else {
        fmStationSongs.value = batch;
        if (autoPlay && batch.length > 0) {
          playSongFromList(
            batch[0],
            batch,
            `FM · ${activeFmChannel.value?.name || "场景电台"}`
          );
        } else if (batch.length > 0) {
          notify(
            `已焕新「${activeFmChannel.value?.name || "电台"}」曲目（共 ${batch.length} 首）`
          );
        }
      }
    } catch (e: any) {
      notify(`调频电台加载失败：${String(e)}`);
    } finally {
      isLoadingFmStation.value = false;
    }
  }


  function clearDiscoverCaches() {
    rankSongsCache.clear();
    newSongsByPeriod.value = {};
  }

  return {
    clearDiscoverCaches,
    discoverSubView,
    discoverHeroExpanding,
    isGlobalQueueOpen,
    recommendSongs,
    isLoadingRecommend,
    personalFmSongs,
    personalFmIndex,
    isLoadingFm,
    isAppendingFmQueue,
    isPersonalFmDrawerOpen,
    currentFmSong,
    nextFmSong,
    newSongPeriod,
    newSongs,
    newSongsByPeriod,
    isLoadingNewSongs,
    isRefreshingAllNewSongs,
    rankList,
    isLoadingRanks,
    isRankGridModalOpen,
    selectedRankCategory,
    activeRank,
    activeRankSongs,
    isLoadingRankSongs,
    isLoadingMoreRankSongs,
    fmChannels,
    selectedFmCategory,
    activeFmChannel,
    fmStationSongs,
    isLoadingFmStation,
    isStageScrolledDown,
    scrollStageToTop,
    loadDailyRecommend,
    loadPersonalFm,
    fetchMorePersonalFmForQueue,
    nextPersonalFmTrack,
    loadNewSongExpress,
    refreshAllNewSongTags,
    loadRankList,
    openRankDetail,
    refreshActiveRank,
    resolveFmChannelCovers,
    refreshDiscoverAll,
    openDailyRecommendPage,
    openGuessYouLikePage,
    openNewSongsPage,
    openRadioPlazaPage,
    playRankDirectly,
    loadFmChannels,
    loadFmStationSongs,
  };
}
