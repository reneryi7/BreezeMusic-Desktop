import { ref, computed, type ComputedRef, type Ref } from "vue";
import {
  isSameTrackIdentity,
  kugouClient,
  splitSongAuthors,
} from "../../api/kugou";
import { triggerHeartBurst } from "../../utils/microMotionFx";
import type {
  AccountProfile,
  AlbumItem,
  DiscoverSubView,
  FollowedArtist,
  KugouSession,
  LibraryAssetTab,
  LibraryDetailMode,
  LocalMusicSettings,
  MainNavTab,
  NavigationHistoryEntry,
  Playlist,
  PurchasedAlbumItem,
  PurchasedSongItem,
  SearchDiscoveryData,
  Song,
  SongAuthor,
} from "../../types";

export interface LibraryAndDetailDeps {
  activeTab: Ref<MainNavTab>;
  discoverSubView: Ref<DiscoverSubView>;
  isNowPlayingOpen: Ref<boolean>;
  isSearchPaletteOpen: Ref<boolean>;
  isLoginModalOpen: Ref<boolean>;
  isRankGridModalOpen: Ref<boolean>;
  isMultiArtistModalOpen: Ref<boolean>;
  multiArtistCandidates: Ref<SongAuthor[]>;
  multiArtistSongTitle: Ref<string>;
  addToPlaylistModalSong: Ref<Song | null>;
  navHistoryStack: Ref<NavigationHistoryEntry[]>;
  isLoggedIn: ComputedRef<boolean>;
  session: Ref<KugouSession | null>;
  profile: Ref<AccountProfile | null>;
  searchKeyword: Ref<string>;
  searchViewMode: Ref<"landing" | "results">;
  searchSuggestions: Ref<string[]>;
  searchResultsArtists: Ref<FollowedArtist[]>;
  searchDiscovery: Ref<SearchDiscoveryData>;
  notify: (msg: string) => void;
  scrollStageToTop: () => void;
  closeSongContextMenu?: () => void;
}

export function createLibraryAndDetailModule(deps: LibraryAndDetailDeps) {
  const {
    activeTab,
    discoverSubView,
    isNowPlayingOpen,
    isSearchPaletteOpen,
    isLoginModalOpen,
    isRankGridModalOpen,
    isMultiArtistModalOpen,
    multiArtistCandidates,
    multiArtistSongTitle,
    addToPlaylistModalSong,
    navHistoryStack,
    isLoggedIn,
    session,
    profile,
    searchKeyword,
    searchViewMode,
    searchSuggestions,
    searchResultsArtists,
    searchDiscovery,
    notify,
    scrollStageToTop,
    closeSongContextMenu,
  } = deps;

  // ================= 3. 我的音乐库（我喜欢/最近/自建/收藏/关注/已购） =================
  const userPlaylists = ref<Playlist[]>([]);
  const selectedPlaylist = ref<Playlist | null>(null);
  const playlistSongs = ref<Song[]>([]);
  const likedSongs = ref<Song[]>([]);
  const recentHistorySongs = ref<Song[]>([]);
  const followedArtists = ref<FollowedArtist[]>([]);
  const purchasedAlbums = ref<PurchasedAlbumItem[]>([]);
  const purchasedSongs = ref<PurchasedSongItem[]>([]);
  const collectedRemotePlaylists = ref<Playlist[]>([]);

  const libraryAssetTab = ref<LibraryAssetTab>("created");
  const libraryDetailMode = ref<LibraryDetailMode>("liked");
  const isLibraryGridView = ref<boolean>(
    localStorage.getItem("bm_lib_grid") !== "false"
  );
  const isLoadingLibrary = ref(false);

  // 歌手详情与专辑详情内嵌工作台状态
  const activeArtistDetail = ref<FollowedArtist | null>(null);
  const activeArtistSongs = ref<Song[]>([]);
  const activeArtistAlbums = ref<AlbumItem[]>([]);
  const artistDetailSubTab = ref<"songs" | "albums">("songs");
  const artistSongsSort = ref<"hot" | "new">("hot");
  const isLoadingArtist = ref(false);

  const activeAlbumDetail = ref<AlbumItem | null>(null);
  const activeAlbumSongs = ref<Song[]>([]);
  const isLoadingAlbum = ref(false);

  function safeParseStringArray(key: string): string[] {
    try {
      const raw = localStorage.getItem(key);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed)
        ? parsed.filter((v): v is string => typeof v === "string")
        : [];
    } catch {
      return [];
    }
  }

  const createdOrderIds = ref<string[]>(safeParseStringArray("bm_order_created"));
  const collectedOrderIds = ref<string[]>(safeParseStringArray("bm_order_collected"));
  const artistsOrderIds = ref<string[]>(safeParseStringArray("bm_order_artists"));
  const purchasedOrderIds = ref<string[]>(safeParseStringArray("bm_order_purchased"));

  function isDefaultCollectPlaylist(p: Playlist): boolean {
    const t = (p.title || p.name || "").trim();
    return t === "默认收藏";
  }

  function applySavedOrder<T>(
    items: T[],
    orderIds: string[],
    getKey: (item: T) => string
  ): T[] {
    if (!orderIds || orderIds.length === 0) return items;
    const orderMap = new Map<string, number>();
    orderIds.forEach((id, idx) => orderMap.set(id, idx));
    return [...items].sort((a, b) => {
      const ia = orderMap.get(getKey(a));
      const ib = orderMap.get(getKey(b));
      if (ia !== undefined && ib !== undefined) return ia - ib;
      if (ia !== undefined) return -1;
      if (ib !== undefined) return 1;
      return 0;
    });
  }

  const createdPlaylists = computed(() => {
    const raw = userPlaylists.value.filter(
      (p) => p.isCreatedByMe && !p.isLiked && !isDefaultCollectPlaylist(p)
    );
    return applySavedOrder(raw, createdOrderIds.value, (p) =>
      String(p.id || p.listId || p.title)
    );
  });
  const collectedPlaylists = computed(() => {
    const cloud = userPlaylists.value.filter(
      (p) => p.isFavorite && !p.isCreatedByMe && !isDefaultCollectPlaylist(p)
    );
    const merged = [...cloud];
    for (const rp of collectedRemotePlaylists.value) {
      if (
        !isDefaultCollectPlaylist(rp) &&
        !merged.some((m) => m.id === rp.id || m.title === rp.title)
      ) {
        merged.push(rp);
      }
    }
    return applySavedOrder(merged, collectedOrderIds.value, (p) =>
      String(p.id || p.globalCollectionId || p.title)
    );
  });
  const likedPlaylistMeta = computed(() =>
    userPlaylists.value.find((p) => p.isLiked || p.title === "我喜欢")
  );


  let sessionPlayedSongs: Song[] = [];

  function isSameHistoryTrack(a: Song, b: Song): boolean {
    const ha = (a.hash || "").trim().toLowerCase();
    const hb = (b.hash || "").trim().toLowerCase();
    if (ha && hb && ha === hb) return true;
    const aAid = Number(a.audioId) || 0;
    const bAid = Number(b.audioId) || 0;
    if (aAid > 0 && aAid === bAid) return true;
    const aAlbumAid = Number(a.albumAudioId) || 0;
    const bAlbumAid = Number(b.albumAudioId) || 0;
    if (aAlbumAid > 0 && aAlbumAid === bAlbumAid) return true;
    const ma = (a.mixSongId || "").trim();
    const mb = (b.mixSongId || "").trim();
    if (ma && ma !== "0" && ma === mb) return true;
    const ia = (a.id || "").trim();
    const ib = (b.id || "").trim();
    if (ia && ib && ia === ib) return true;
    return false;
  }

  function mergeHistoryTrack(base: Song, incoming: Song): Song {
    return {
      ...base,
      ...incoming,
      coverUrl: incoming.coverUrl || base.coverUrl || "",
      album: incoming.album || base.album || "",
      albumId: incoming.albumId || base.albumId || "",
      artistId: incoming.artistId || base.artistId || "",
    };
  }

  function syncRecentHistoryPlaylistView() {
    if (selectedPlaylist.value?.id === "__recent_history__") {
      playlistSongs.value = [...recentHistorySongs.value];
      selectedPlaylist.value.trackCount = recentHistorySongs.value.length;
      if (recentHistorySongs.value[0]?.coverUrl) {
        selectedPlaylist.value.coverUrl = recentHistorySongs.value[0].coverUrl;
      }
    }
  }

  function recordSongToRecentHistory(song: Song) {
    if (!song || (!song.hash && !song.id && !song.title)) return;
    const prevInAll = recentHistorySongs.value.find((s) =>
      isSameHistoryTrack(s, song)
    );
    const merged = prevInAll ? mergeHistoryTrack(prevInAll, song) : { ...song };
    sessionPlayedSongs = [
      merged,
      ...sessionPlayedSongs.filter((s) => !isSameHistoryTrack(s, merged)),
    ].slice(0, 200);
    recentHistorySongs.value = [
      merged,
      ...recentHistorySongs.value.filter((s) => !isSameHistoryTrack(s, merged)),
    ].slice(0, 200);
    syncRecentHistoryPlaylistView();
  }

  function invalidatePlaylistSwrCacheFor(pl?: Playlist | null, listId?: number) {
    if (!pl && !listId) {
      playlistSwrCache.clear();
      return;
    }
    const ids = new Set<string>();
    if (pl?.globalCollectionId) ids.add(pl.globalCollectionId);
    if (pl?.id) ids.add(pl.id);
    if (pl?.listCreateGid) ids.add(pl.listCreateGid);
    if (pl?.specialId) ids.add(pl.specialId);
    if (listId) {
      const matched = userPlaylists.value.find((p) => p.listId === listId);
      if (matched?.globalCollectionId) ids.add(matched.globalCollectionId);
      if (matched?.id) ids.add(matched.id);
    }
    for (const k of ids) {
      playlistSwrCache.delete(k);
    }
  }

  function loadRecentHistoryMergeOrder(sessionList: Song[], cloudList: Song[]): Song[] {
    const merged: Song[] = [];
    // 以云端最新拉取的富化元数据为优先，保留本会话新增记录，避免旧会话快照覆盖云端最新富化信息
    for (const item of [...sessionList, ...cloudList]) {
      const idx = merged.findIndex((m) => isSameHistoryTrack(m, item));
      if (idx >= 0) {
        // 后到的是云端新富化数据时，应让云端新数据作为 incoming 补全旧会话快照
        merged[idx] = mergeHistoryTrack(merged[idx], item);
      } else if (merged.length < 200) {
        merged.push(item);
      }
    }
    return merged;
  }

  async function loadRecentHistory() {
    try {
      const list = await kugouClient.getRecentHistory();
      if (list.length > 0 || recentHistorySongs.value.length === 0) {
        recentHistorySongs.value = loadRecentHistoryMergeOrder(
          sessionPlayedSongs,
          list
        );
        syncRecentHistoryPlaylistView();
      }
    } catch (e) {
      console.warn("loadRecentHistory error:", e);
    }
  }

  async function loadUserPlaylists() {
    if (!isLoggedIn.value) return;
    isLoadingLibrary.value = true;
    try {
      const rawList = await kugouClient.getUserPlaylists();
      const list = rawList.filter((p) => !isDefaultCollectPlaylist(p));
      userPlaylists.value = list;

      const likedPl = list.find((p) => p.isLiked || p.title === "我喜欢");
      if (likedPl) {
        kugouClient
          .getPlaylistSongs(likedPl.globalCollectionId)
          .then((songs) => {
            likedSongs.value = songs;
            if (likedPl.trackCount <= 0 && songs.length > 0) {
              likedPl.trackCount = songs.length;
            }
          })
          .catch(() => {});
      }

      if (list.length > 0 && !selectedPlaylist.value) {
        const preferred =
          list.find((p) => p.isCreatedByMe && !p.isLiked && p.trackCount > 0) ||
          list.find((p) => p.isLiked && p.trackCount > 0) ||
          list.find((p) => p.trackCount > 0) ||
          list[0];
        await selectPlaylist(preferred, false);
      }
    } catch (e: any) {
      notify(`获取歌单失败：${String(e)}`);
    } finally {
      isLoadingLibrary.value = false;
    }
  }

  // 独立详情页 SWR 内存秒开缓存 (Playlist / Artist / Album) 与竞态序号保护
  const playlistSwrCache = new Map<string, { playlist: Playlist; songs: Song[] }>();
  const artistSwrCache = new Map<
    string,
    { detail: FollowedArtist; songs: Song[]; albums: AlbumItem[] }
  >();
  const albumSwrCache = new Map<string, { detail: AlbumItem; songs: Song[] }>();
  let activePlaylistReqSeq = 0;
  let activeArtistReqSeq = 0;
  let activeAlbumReqSeq = 0;

  function getStageScrollTop(): number {
    if (typeof document === "undefined") return 0;
    const stage = document.querySelector(".skyline-stage-viewport") as HTMLElement | null;
    return stage ? stage.scrollTop : 0;
  }

  function restoreStageScrollTop(scrollTop = 0) {
    if (typeof document === "undefined") return;
    const apply = () => {
      const stage = document.querySelector(".skyline-stage-viewport") as HTMLElement | null;
      if (stage) stage.scrollTop = scrollTop;
    };
    apply();
    if (typeof requestAnimationFrame !== "undefined") {
      requestAnimationFrame(apply);
    }
    setTimeout(apply, 28);
  }

  function pushNavHistory() {
    const entry: NavigationHistoryEntry = {
      tab: activeTab.value,
      discoverSubView: discoverSubView.value,
      libraryAssetTab: libraryAssetTab.value,
      libraryDetailMode: libraryDetailMode.value,
      artistId: activeArtistDetail.value?.artistId,
      albumId: activeAlbumDetail.value?.albumId,
      playlistId: selectedPlaylist.value?.id || selectedPlaylist.value?.globalCollectionId,
      searchViewMode: searchViewMode.value,
      searchKeyword: searchKeyword.value,
      returnToNowPlaying: isNowPlayingOpen.value,
      scrollTop: getStageScrollTop(),
    };
    const top = navHistoryStack.value[navHistoryStack.value.length - 1];
    if (
      !top ||
      top.tab !== entry.tab ||
      top.discoverSubView !== entry.discoverSubView ||
      top.artistId !== entry.artistId ||
      top.albumId !== entry.albumId ||
      top.playlistId !== entry.playlistId ||
      top.searchViewMode !== entry.searchViewMode ||
      top.searchKeyword !== entry.searchKeyword ||
      top.returnToNowPlaying !== entry.returnToNowPlaying
    ) {
      navHistoryStack.value.push(entry);
      if (navHistoryStack.value.length > 24) {
        navHistoryStack.value.shift();
      }
    }
  }

  function clearNavHistory() {
    navHistoryStack.value = [];
  }

  function navigateBack() {
    closeSongContextMenu?.();
    if (isRankGridModalOpen.value) {
      isRankGridModalOpen.value = false;
      return;
    }
    if (isNowPlayingOpen.value) {
      isNowPlayingOpen.value = false;
      return;
    }
    const popped = navHistoryStack.value.pop();
    if (popped) {
      if (typeof window !== "undefined") {
        window.dispatchEvent(
          new CustomEvent("breeze:nav-back-restore", {
            detail: { scrollTop: popped.scrollTop || 0 },
          })
        );
      }
      if (popped.returnToNowPlaying) {
        activeTab.value = popped.tab;
        if (popped.discoverSubView) discoverSubView.value = popped.discoverSubView;
        isNowPlayingOpen.value = true;
        return;
      }
      activeTab.value = popped.tab;
      if (popped.discoverSubView) discoverSubView.value = popped.discoverSubView;
      if (popped.libraryAssetTab) libraryAssetTab.value = popped.libraryAssetTab;
      if (popped.libraryDetailMode) libraryDetailMode.value = popped.libraryDetailMode;
      if (popped.tab === "artist_detail" && popped.artistId) {
        const cachedArtist = artistSwrCache.get(popped.artistId);
        if (cachedArtist) {
          activeArtistDetail.value = cachedArtist.detail;
          activeArtistSongs.value = cachedArtist.songs;
          activeArtistAlbums.value = cachedArtist.albums;
        }
      } else if (popped.tab === "album_detail" && popped.albumId) {
        const cachedAlbum = albumSwrCache.get(popped.albumId);
        if (cachedAlbum) {
          activeAlbumDetail.value = cachedAlbum.detail;
          activeAlbumSongs.value = cachedAlbum.songs;
        }
      } else if (popped.tab === "playlist_detail" && popped.playlistId) {
        const cachedPl = playlistSwrCache.get(popped.playlistId);
        if (cachedPl) {
          selectedPlaylist.value = cachedPl.playlist;
          playlistSongs.value = cachedPl.songs;
        }
      } else if (popped.tab === "search") {
        const targetMode = popped.searchViewMode || "landing";
        searchViewMode.value = targetMode;
        if (targetMode === "landing") {
          searchKeyword.value = "";
          searchSuggestions.value = [];
        } else if (popped.searchKeyword !== undefined) {
          searchKeyword.value = popped.searchKeyword;
        }
      }
      restoreStageScrollTop(popped.scrollTop || 0);
      return;
    }
    if (activeTab.value === "discover" && discoverSubView.value !== "home") {
      discoverSubView.value = "home";
      scrollStageToTop();
      return;
    }
    if (activeTab.value === "search" && searchViewMode.value === "results") {
      searchViewMode.value = "landing";
      searchKeyword.value = "";
      searchSuggestions.value = [];
      scrollStageToTop();
      return;
    }
    if (
      activeTab.value === "cloud_drive" ||
      activeTab.value === "local_music"
    ) {
      activeTab.value = "library";
      scrollStageToTop();
      return;
    }
    if (
      activeTab.value === "artist_detail" ||
      activeTab.value === "album_detail" ||
      activeTab.value === "playlist_detail" ||
      activeTab.value === "search"
    ) {
      activeTab.value = "discover";
      discoverSubView.value = "home";
      scrollStageToTop();
    }
  }

  function openCloudDrivePage() {
    pushNavHistory();
    activeTab.value = "cloud_drive";
    scrollStageToTop();
    if (cloudDiskSongs.value.length === 0 && isLoggedIn.value) {
      loadUserCloudDisk(false);
    }
  }

  function openLocalMusicPage() {
    pushNavHistory();
    activeTab.value = "local_music";
    scrollStageToTop();
    if (localMusicSongs.value.length === 0) {
      scanLocalMusicLibrary();
    }
  }

  function openSoundMixerFromPlayer() {
    pushNavHistory();
    isNowPlayingOpen.value = false;
    activeTab.value = "tuner";
    scrollStageToTop();
  }

  async function selectPlaylist(pl: Playlist, openStandalonePage = true) {
    const reqSeq = ++activePlaylistReqSeq;
    if (openStandalonePage) {
      pushNavHistory();
      activeTab.value = "playlist_detail";
      isNowPlayingOpen.value = false;
      isSearchPaletteOpen.value = false;
      scrollStageToTop();
    }
    const candidateIds: string[] = [];
    const pushCand = (val?: string | number | null) => {
      const s = String(val ?? "").trim();
      if (s && s !== "0" && !candidateIds.includes(s)) {
        candidateIds.push(s);
      }
    };
    if (pl.isFavorite) {
      pushCand(pl.listCreateGid);
      if (pl.listCreateUserid && pl.listCreateListid && String(pl.listCreateListid) !== "0") {
        pushCand(`collection_3_${pl.listCreateUserid}_${pl.listCreateListid}_0`);
      }
    }
    pushCand(pl.globalCollectionId);
    pushCand(pl.id);
    pushCand(pl.listCreateGid);
    pushCand(pl.specialId);
    pushCand(pl.listCreateListid);
    if (pl.listId && pl.listId > 0) {
      pushCand(pl.listId);
    }
    const gid = candidateIds[0] || "";
    const cached = gid ? playlistSwrCache.get(gid) : undefined;
    if (cached && cached.songs.length > 0) {
      selectedPlaylist.value = { ...cached.playlist, ...pl };
      playlistSongs.value = cached.songs;
      isLoadingLibrary.value = false;
    } else {
      selectedPlaylist.value = pl;
      playlistSongs.value = [];
      isLoadingLibrary.value = true;
    }
    libraryDetailMode.value = "playlist";
    try {
      const metaPromise =
        !pl.isCreatedByMe && gid
          ? kugouClient.getPlaylistInfo(gid).catch(() => null)
          : Promise.resolve(null);

      let songs = gid ? await kugouClient.getPlaylistSongs(gid) : [];
      if (reqSeq !== activePlaylistReqSeq) return;
      if (songs.length === 0 && candidateIds.length > 1) {
        for (const altId of candidateIds.slice(1)) {
          const altSongs = await kugouClient.getPlaylistSongs(altId);
          if (reqSeq !== activePlaylistReqSeq) return;
          if (altSongs.length > 0) {
            songs = altSongs;
            break;
          }
        }
      }
      playlistSongs.value = songs;
      isLoadingLibrary.value = false;

      if (selectedPlaylist.value) {
        if (selectedPlaylist.value.trackCount < songs.length) {
          selectedPlaylist.value.trackCount = songs.length;
        }
        if (!selectedPlaylist.value.coverUrl && songs[0]?.coverUrl) {
          selectedPlaylist.value.coverUrl = songs[0].coverUrl;
        }
      }
      if (pl.isLiked || pl.title === "我喜欢") {
        likedSongs.value = songs;
      }

      const metaInfo = await metaPromise;
      if (reqSeq !== activePlaylistReqSeq) return;
      if (metaInfo && selectedPlaylist.value) {
        selectedPlaylist.value = {
          ...selectedPlaylist.value,
          title: metaInfo.title || selectedPlaylist.value.title,
          name: metaInfo.title || selectedPlaylist.value.name,
          description: metaInfo.description || selectedPlaylist.value.description,
          intro: metaInfo.intro || metaInfo.description || selectedPlaylist.value.intro,
          coverUrl: metaInfo.coverUrl || selectedPlaylist.value.coverUrl || songs[0]?.coverUrl || "",
          creator: metaInfo.creator || selectedPlaylist.value.creator,
          playCount: metaInfo.playCount || selectedPlaylist.value.playCount,
          trackCount: Math.max(songs.length, metaInfo.trackCount || 0, selectedPlaylist.value.trackCount || 0),
        };
      }
      if (gid && selectedPlaylist.value) {
        playlistSwrCache.set(gid, {
          playlist: { ...selectedPlaylist.value },
          songs: [...songs],
        });
      }
    } catch (e: any) {
      if (reqSeq === activePlaylistReqSeq && !cached) {
        notify(`加载歌单曲目失败：${String(e)}`);
      }
    } finally {
      if (reqSeq === activePlaylistReqSeq) {
        isLoadingLibrary.value = false;
      }
    }
  }

  async function openPlaylistDetail(pl: Playlist) {
    await selectPlaylist(pl, true);
  }

  async function openLikedSongsDetail() {
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return;
    }
    const likedPl = likedPlaylistMeta.value;
    if (likedPl && (likedPl.globalCollectionId || likedPl.id)) {
      if (likedSongs.value.length > 0) {
        playlistSwrCache.set(likedPl.globalCollectionId || likedPl.id, {
          playlist: {
            ...likedPl,
            title: "我喜欢的音乐",
            name: "我喜欢的音乐",
            isLiked: true,
            trackCount: Math.max(likedSongs.value.length, likedPl.trackCount || 0),
            coverUrl: likedSongs.value[0]?.coverUrl || likedPl.coverUrl || "",
          },
          songs: [...likedSongs.value],
        });
      }
      await selectPlaylist(
        {
          ...likedPl,
          title: "我喜欢的音乐",
          name: "我喜欢的音乐",
          isLiked: true,
          trackCount: Math.max(likedSongs.value.length, likedPl.trackCount || 0),
          coverUrl: likedSongs.value[0]?.coverUrl || likedPl.coverUrl || "",
        },
        true
      );
      return;
    }
    pushNavHistory();
    selectedPlaylist.value = {
      id: "__liked_music__",
      listId: 0,
      globalCollectionId: "",
      title: "我喜欢的音乐",
      name: "我喜欢的音乐",
      description: "你在酷狗概念版标记红心的全部珍藏单曲，支持云端实时双向同步与无损畅听。",
      intro: "你在酷狗概念版标记红心的全部珍藏单曲，支持云端实时双向同步与无损畅听。",
      coverUrl: likedSongs.value[0]?.coverUrl || "",
      trackCount: likedSongs.value.length,
      playCount: 0,
      creatorName: profile.value?.nickname || session.value?.nickname || "我",
      creator: profile.value?.nickname || session.value?.nickname || "我",
      isLiked: true,
      isCreatedByMe: true,
      isFavorite: false,
    };
    playlistSongs.value = [...likedSongs.value];
    activeTab.value = "playlist_detail";
    isNowPlayingOpen.value = false;
    isSearchPaletteOpen.value = false;
    scrollStageToTop();
  }

  async function openRecentHistoryDetail() {
    pushNavHistory();
    selectedPlaylist.value = {
      id: "__recent_history__",
      listId: 0,
      globalCollectionId: "__recent_history__",
      title: "最近播放",
      name: "最近播放",
      description: "自动持久化保存本地与酷狗概念版云端双端最近聆听足迹，重启软件不丢失。",
      intro: "自动持久化保存本地与酷狗概念版云端双端最近聆听足迹，重启软件不丢失。",
      coverUrl: recentHistorySongs.value[0]?.coverUrl || "",
      trackCount: recentHistorySongs.value.length,
      playCount: 0,
      creatorName: profile.value?.nickname || session.value?.nickname || "微风乐迷",
      creator: profile.value?.nickname || session.value?.nickname || "微风乐迷",
      isLiked: false,
      isCreatedByMe: false,
      isFavorite: false,
    };
    playlistSongs.value = [...recentHistorySongs.value];
    activeTab.value = "playlist_detail";
    isNowPlayingOpen.value = false;
    isSearchPaletteOpen.value = false;
    scrollStageToTop();
    await loadRecentHistory();
    if (selectedPlaylist.value?.id === "__recent_history__") {
      playlistSongs.value = [...recentHistorySongs.value];
      selectedPlaylist.value.trackCount = recentHistorySongs.value.length;
      if (!selectedPlaylist.value.coverUrl && recentHistorySongs.value[0]?.coverUrl) {
        selectedPlaylist.value.coverUrl = recentHistorySongs.value[0].coverUrl;
      }
    }
  }

  async function openPurchasedSongsDetail() {
    pushNavHistory();
    const songs = purchasedSongs.value.map((p) => p.song);
    selectedPlaylist.value = {
      id: "__purchased_songs__",
      listId: 0,
      globalCollectionId: "__purchased_songs__",
      title: "已购单曲合集",
      name: "已购单曲合集",
      description: "你在酷狗音乐已付费购买的数字单曲与无损珍藏曲目。",
      intro: "你在酷狗音乐已付费购买的数字单曲与无损珍藏曲目。",
      coverUrl: songs[0]?.coverUrl || purchasedAlbums.value[0]?.coverUrl || "",
      trackCount: songs.length,
      playCount: 0,
      creatorName: profile.value?.nickname || session.value?.nickname || "我",
      creator: profile.value?.nickname || session.value?.nickname || "我",
      isLiked: false,
      isCreatedByMe: false,
      isFavorite: false,
    };
    playlistSongs.value = [...songs];
    activeTab.value = "playlist_detail";
    isNowPlayingOpen.value = false;
    isSearchPaletteOpen.value = false;
    scrollStageToTop();
    await loadPurchasedAssets();
    if (selectedPlaylist.value?.id === "__purchased_songs__") {
      const updated = purchasedSongs.value.map((p) => p.song);
      playlistSongs.value = updated;
      selectedPlaylist.value.trackCount = updated.length;
      if (!selectedPlaylist.value.coverUrl && updated[0]?.coverUrl) {
        selectedPlaylist.value.coverUrl = updated[0].coverUrl;
      }
    }
  }

  async function refreshCurrentPlaylistOnly() {
    const pl = selectedPlaylist.value;
    if (!pl) return;
    isLoadingLibrary.value = true;
    try {
      if (pl.id === "__recent_history__") {
        await loadRecentHistory();
        playlistSongs.value = [...recentHistorySongs.value];
        if (selectedPlaylist.value) {
          selectedPlaylist.value.trackCount = recentHistorySongs.value.length;
          selectedPlaylist.value.coverUrl =
            recentHistorySongs.value[0]?.coverUrl || selectedPlaylist.value.coverUrl;
        }
        notify(`已刷新「最近播放」（共 ${recentHistorySongs.value.length} 首）`);
        return;
      }
      if (pl.id === "__purchased_songs__") {
        await loadPurchasedAssets();
        const songs = purchasedSongs.value.map((p) => p.song);
        playlistSongs.value = songs;
        if (selectedPlaylist.value) {
          selectedPlaylist.value.trackCount = songs.length;
          selectedPlaylist.value.coverUrl =
            songs[0]?.coverUrl || selectedPlaylist.value.coverUrl;
        }
        notify(`已刷新「已购单曲合集」（共 ${songs.length} 首）`);
        return;
      }
      const gid = pl.globalCollectionId || pl.id;
      const [songs, metaInfo] = await Promise.all([
        kugouClient.getPlaylistSongs(gid),
        !pl.isCreatedByMe && gid
          ? kugouClient.getPlaylistInfo(gid).catch(() => null)
          : Promise.resolve(null),
      ]);
      playlistSongs.value = songs;
      if (metaInfo && selectedPlaylist.value) {
        selectedPlaylist.value = {
          ...selectedPlaylist.value,
          title: metaInfo.title || selectedPlaylist.value.title,
          name: metaInfo.title || selectedPlaylist.value.name,
          description: metaInfo.description || selectedPlaylist.value.description,
          intro: metaInfo.intro || metaInfo.description || selectedPlaylist.value.intro,
          coverUrl: metaInfo.coverUrl || selectedPlaylist.value.coverUrl || songs[0]?.coverUrl || "",
          trackCount: Math.max(songs.length, metaInfo.trackCount || 0),
        };
      } else if (selectedPlaylist.value) {
        selectedPlaylist.value.trackCount = Math.max(songs.length, selectedPlaylist.value.trackCount || 0);
        if (!selectedPlaylist.value.coverUrl && songs[0]?.coverUrl) {
          selectedPlaylist.value.coverUrl = songs[0].coverUrl;
        }
      }
      if (pl.isLiked || pl.title === "我喜欢" || pl.title === "我喜欢的音乐") {
        likedSongs.value = songs;
      }
      notify(`已刷新歌单「${selectedPlaylist.value?.title || pl.title}」（共 ${songs.length} 首）`);
    } catch (e: any) {
      notify(`刷新歌单失败：${String(e)}`);
    } finally {
      isLoadingLibrary.value = false;
    }
  }

  async function updatePlaylistMeta(listId: number, name: string, intro: string, coverUrl = "") {
    try {
      const target =
        userPlaylists.value.find((p) => p.listId === listId) ||
        (selectedPlaylist.value?.listId === listId ? selectedPlaylist.value : null);
      const globalId = target?.globalCollectionId || target?.id || "";
      await kugouClient.updateUserPlaylist(listId, globalId, name, intro, coverUrl);
      if (selectedPlaylist.value && selectedPlaylist.value.listId === listId) {
        selectedPlaylist.value.title = name;
        selectedPlaylist.value.name = name;
        selectedPlaylist.value.description = intro;
        selectedPlaylist.value.intro = intro;
        if (coverUrl) selectedPlaylist.value.coverUrl = coverUrl;
      }
      if (target) {
        target.title = name;
        target.name = name;
        target.description = intro;
        target.intro = intro;
        if (coverUrl) target.coverUrl = coverUrl;
      }
      invalidatePlaylistSwrCacheFor(target || selectedPlaylist.value, listId);
      notify(`已保存歌单「${name}」编辑信息并同步`);
      return true;
    } catch (e: any) {
      notify(`编辑歌单失败：${String(e)}`);
      return false;
    }
  }

  async function batchAddSongsToPlaylist(listId: number, songs: Song[]) {
    if (!songs.length) return false;
    try {
      await kugouClient.addSongsToPlaylistBatch(listId, songs);
      invalidatePlaylistSwrCacheFor(null, listId);
      if (selectedPlaylist.value?.listId === listId) {
        const toPrepend = songs.filter(
          (s) => !playlistSongs.value.some((e) => isSameTrackIdentity(e, s))
        );
        if (toPrepend.length > 0) {
          playlistSongs.value = [...toPrepend, ...playlistSongs.value];
          selectedPlaylist.value.trackCount = playlistSongs.value.length;
        }
      }
      notify(`已将 ${songs.length} 首歌曲批量添加至目标歌单`);
      await loadUserPlaylists();
      return true;
    } catch (e: any) {
      notify(`批量添加失败：${String(e)}`);
      return false;
    }
  }

  async function batchRemoveSongsFromPlaylist(songsToRemove: Song[]) {
    const pl = selectedPlaylist.value;
    if (!pl || !pl.listId || songsToRemove.length === 0) {
      notify("仅自建云歌单支持批量移除曲目");
      return false;
    }
    let removedCount = 0;
    const removedSongs: Song[] = [];
    for (const s of songsToRemove) {
      try {
        await kugouClient.removeSongFromPlaylist(pl.listId, s);
        removedCount += 1;
        removedSongs.push(s);
      } catch {
        // continue
      }
    }
    if (removedSongs.length > 0) {
      playlistSongs.value = playlistSongs.value.filter(
        (s) => !removedSongs.some((r) => isSameTrackIdentity(s, r))
      );
      if (pl.isLiked) {
        likedSongs.value = likedSongs.value.filter(
          (s) => !removedSongs.some((r) => isSameTrackIdentity(s, r))
        );
      }
      pl.trackCount = playlistSongs.value.length;
      const inList = userPlaylists.value.find((p) => p.listId === pl.listId);
      if (inList) inList.trackCount = playlistSongs.value.length;
      invalidatePlaylistSwrCacheFor(pl, pl.listId);
    }
    notify(`已从「${pl.title}」批量移除 ${removedCount} 首歌曲`);
    return true;
  }

  async function createNewPlaylist(name: string) {
    if (
      Boolean(import.meta.env.DEV) &&
      typeof window !== "undefined" &&
      !("__TAURI_INTERNALS__" in window)
    ) {
      const nextId = Date.now() % 1000000;
      userPlaylists.value = [
        ...userPlaylists.value,
        {
          id: `demo_pl_${nextId}`,
          title: name,
          name,
          coverUrl: "",
          trackCount: 0,
          description: "浏览器预览新建歌单",
          creatorName: "我",
          isCreatedByMe: true,
          isFavorite: false,
          isLiked: false,
          listId: nextId,
          globalCollectionId: `demo_gid_${nextId}`,
        },
      ];
      notify(`已创建歌单「${name}」`);
      return true;
    }
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return false;
    }
    try {
      await kugouClient.createUserPlaylist(name);
      notify(`已创建歌单「${name}」`);
      await loadUserPlaylists();
      return true;
    } catch (e: any) {
      notify(`新建歌单失败：${String(e)}`);
      return false;
    }
  }

  async function deletePlaylist(pl: Playlist) {
    if (!pl.listId) return;
    try {
      await kugouClient.deleteUserPlaylist(pl.listId);
      invalidatePlaylistSwrCacheFor(pl, pl.listId);
      notify(`已删除歌单「${pl.title}」`);
      await loadUserPlaylists();
    } catch (e: any) {
      notify(`删除歌单失败：${String(e)}`);
    }
  }

  async function removeSongFromCurrentPlaylist(song: Song) {
    // 始终以当前详情页实际打开的歌单为准，彻底修复 libraryAssetTab 停留在 "liked" 时误删「我喜欢」的严重 Bug
    const pl = selectedPlaylist.value;
    if (!pl || !pl.listId) {
      notify("仅自建云歌单支持移除曲目");
      return;
    }
    try {
      await kugouClient.removeSongFromPlaylist(pl.listId, song);
      playlistSongs.value = playlistSongs.value.filter(
        (s) => !isSameTrackIdentity(s, song)
      );
      if (pl.isLiked) {
        likedSongs.value = likedSongs.value.filter(
          (s) => !isSameTrackIdentity(s, song)
        );
      }
      pl.trackCount = playlistSongs.value.length;
      const inList = userPlaylists.value.find((p) => p.listId === pl.listId);
      if (inList) inList.trackCount = playlistSongs.value.length;
      invalidatePlaylistSwrCacheFor(pl, pl.listId);
      notify(`已从「${pl.title}」移除 ${song.title}`);
    } catch (e: any) {
      notify(`移除曲目失败：${String(e)}`);
    }
  }

  async function importExternalPlaylist(nameOrRawText: string, rawTextArg?: string) {
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return 0;
    }
    const customName = rawTextArg !== undefined ? nameOrRawText.trim() : "";
    const rawText = (rawTextArg !== undefined ? rawTextArg : nameOrRawText).trim();
    if (!rawText) return 0;

    notify("正在解析分享链接 / 口令 / 歌单曲目...");
    let songsToImport: Song[] = [];
    let resolvedTitle = customName || `导入歌单 ${new Date().toLocaleDateString()}`;

    try {
      const parsed = await kugouClient.parseSharedMusic(rawText);
      if (parsed.songs && parsed.songs.length > 0) {
        songsToImport = parsed.songs;
        if (!customName && parsed.playlist?.title) {
          resolvedTitle = parsed.playlist.title;
        }
      }
    } catch {
      // 回退至逐行搜索匹配
    }

    if (songsToImport.length === 0) {
      const lines = rawText
        .split(/\r?\n/)
        .map((l) => l.trim())
        .filter((l) => l.length > 1);
      for (const line of lines.slice(0, 35)) {
        try {
          const res = await kugouClient.searchSongs(line, 1, 2);
          if (res.length > 0) {
            songsToImport.push(res[0]);
          }
        } catch {
          // ignore
        }
      }
    }

    if (songsToImport.length === 0) {
      notify("未能从输入内容中解析到可导入的歌曲");
      return 0;
    }

    await kugouClient.createUserPlaylist(resolvedTitle);
    await loadUserPlaylists();
    const target = userPlaylists.value.find((p) => p.title === resolvedTitle);
    let matchedCount = 0;

    if (target?.listId) {
      for (const song of songsToImport.slice(0, 60)) {
        try {
          await kugouClient.addSongToPlaylist(target.listId, song);
          matchedCount += 1;
        } catch {
          // ignore single failure
        }
      }
    }
    await loadUserPlaylists();
    if (target) {
      await selectPlaylist(target);
    }
    notify(`歌单「${resolvedTitle}」导入完成，成功同步 ${matchedCount} 首曲目`);
    return matchedCount;
  }

  function isSamePlaylistIdentity(a: Playlist, b: Playlist): boolean {
    const aGid = (a.globalCollectionId || "").trim();
    const bGid = (b.globalCollectionId || "").trim();
    if (aGid && bGid && aGid === bGid) return true;
    const aSpec = (a.specialId || "").trim();
    const bSpec = (b.specialId || "").trim();
    if (aSpec && bSpec && aSpec === bSpec) return true;
    const aId = (a.id || "").trim();
    const bId = (b.id || "").trim();
    if (aId && bId && aId === bId) return true;
    if (aGid && bId && aGid === bId) return true;
    if (bGid && aId && bGid === aId) return true;
    return false;
  }

  function isPlaylistCollected(pl: Playlist): boolean {
    return collectedPlaylists.value.some((p) => isSamePlaylistIdentity(p, pl));
  }

  async function toggleCollectPlaylist(pl: Playlist) {
    const already = isPlaylistCollected(pl);
    const prevCollected = [...collectedRemotePlaylists.value];
    if (already) {
      collectedRemotePlaylists.value = collectedRemotePlaylists.value.filter(
        (p) => !isSamePlaylistIdentity(p, pl)
      );
      if (isLoggedIn.value) {
        try {
          await kugouClient.uncollectPlaylist(pl);
          notify(`已取消收藏歌单「${pl.title}」`);
          await loadUserPlaylists();
        } catch (e: any) {
          collectedRemotePlaylists.value = prevCollected;
          notify(`取消收藏失败：${String(e)}`);
        }
      } else {
        notify(`已取消收藏歌单「${pl.title}」`);
      }
    } else {
      collectedRemotePlaylists.value.unshift({
        ...pl,
        isFavorite: true,
        isCreatedByMe: false,
      });
      if (isLoggedIn.value) {
        try {
          await kugouClient.collectPlaylist(pl);
          notify(`已收藏歌单「${pl.title}」并同步至云端`);
          await loadUserPlaylists();
        } catch (e: any) {
          collectedRemotePlaylists.value = prevCollected;
          notify(`收藏歌单失败：${String(e)}`);
        }
      } else {
        notify(`已本地暂存收藏歌单「${pl.title}」（登录后同步云端）`);
      }
    }
  }

  async function addSongToUserPlaylist(
    listId: number,
    song: Song,
    options?: { keepOpen?: boolean }
  ) {
    if (
      Boolean(import.meta.env.DEV) &&
      typeof window !== "undefined" &&
      !("__TAURI_INTERNALS__" in window)
    ) {
      const pl = userPlaylists.value.find((p) => p.listId === listId);
      if (pl) {
        pl.trackCount = (pl.trackCount || 0) + 1;
      }
      notify(`已将「${song.title}」添加到歌单`);
      if (!options?.keepOpen) {
        addToPlaylistModalSong.value = null;
      }
      return true;
    }
    try {
      await kugouClient.addSongToPlaylist(listId, song);
      invalidatePlaylistSwrCacheFor(null, listId);
      if (
        selectedPlaylist.value?.listId === listId &&
        !playlistSongs.value.some((s) => isSameTrackIdentity(s, song))
      ) {
        playlistSongs.value = [song, ...playlistSongs.value];
        selectedPlaylist.value.trackCount = playlistSongs.value.length;
      }
      notify(`已将「${song.title}」添加到歌单`);
      if (!options?.keepOpen) {
        addToPlaylistModalSong.value = null;
      }
      await loadUserPlaylists();
      return true;
    } catch (e: any) {
      notify(`添加歌曲失败：${String(e)}`);
      return false;
    }
  }

  function isSongLiked(songOrHash: Song | string | null | undefined): boolean {
    if (!songOrHash) return false;
    return likedSongs.value.some((s) => isSameTrackIdentity(s, songOrHash));
  }

  function syncLikedViewAndCounts() {
    if (likedPlaylistMeta.value) {
      likedPlaylistMeta.value.trackCount = likedSongs.value.length;
      invalidatePlaylistSwrCacheFor(
        likedPlaylistMeta.value,
        likedPlaylistMeta.value.listId
      );
    }
    if (
      selectedPlaylist.value &&
      (selectedPlaylist.value.isLiked ||
        selectedPlaylist.value.title === "我喜欢的音乐" ||
        selectedPlaylist.value.title === "我喜欢")
    ) {
      playlistSongs.value = [...likedSongs.value];
      selectedPlaylist.value.trackCount = likedSongs.value.length;
    }
  }

  async function toggleFavoriteSong(song: Song | null) {
    if (!song) return;
    if (isLoggedIn.value && !likedPlaylistMeta.value?.listId) {
      await loadUserPlaylists();
    }
    const prevLiked = [...likedSongs.value];
    const idx = likedSongs.value.findIndex((s) => isSameTrackIdentity(s, song));
    if (idx >= 0) {
      const matched = likedSongs.value[idx];
      likedSongs.value.splice(idx, 1);
      syncLikedViewAndCounts();
      if (isLoggedIn.value && likedPlaylistMeta.value?.listId) {
        try {
          await kugouClient.removeSongFromPlaylist(
            likedPlaylistMeta.value.listId,
            matched || song
          );
          notify(`已从「我喜欢的音乐」移除 ${song.title}`);
        } catch (e: any) {
          likedSongs.value = prevLiked;
          syncLikedViewAndCounts();
          notify(`云端取消红心失败：${String(e)}`);
        }
      } else {
        notify(`已从「我喜欢的音乐」移除 ${song.title}`);
      }
    } else {
      likedSongs.value.unshift(song);
      syncLikedViewAndCounts();
      triggerHeartBurst();
      if (isLoggedIn.value && likedPlaylistMeta.value?.listId) {
        try {
          await kugouClient.addSongToPlaylist(likedPlaylistMeta.value.listId, song);
          notify(`♥ 已将「${song.title}」加入我喜欢的音乐并同步云端`);
        } catch (e: any) {
          likedSongs.value = prevLiked;
          syncLikedViewAndCounts();
          notify(`云端同步红心失败：${String(e)}`);
        }
      } else {
        notify(`♥ 已将「${song.title}」加入我喜欢的音乐`);
      }
    }
  }

  function isArtistFollowed(artistId?: string, name?: string): boolean {
    const idStr = String(artistId || "").trim();
    const nameStr = String(name || "").trim().toLowerCase();
    return followedArtists.value.some((a) => {
      const aId = String(a.artistId || a.singerId || "").trim();
      if (idStr && idStr !== "0" && aId && aId !== "0" && aId === idStr) {
        return true;
      }
      const aName = String(a.name || "").trim().toLowerCase();
      return Boolean(nameStr && aName && aName === nameStr);
    });
  }

  function syncArtistFollowFlags() {
    if (searchResultsArtists.value.length > 0) {
      searchResultsArtists.value = searchResultsArtists.value.map((a) => ({
        ...a,
        isFollowed: isArtistFollowed(a.artistId, a.name),
      }));
    }
    if (searchDiscovery.value.hotArtists.length > 0) {
      searchDiscovery.value = {
        ...searchDiscovery.value,
        hotArtists: searchDiscovery.value.hotArtists.map((a) => ({
          ...a,
          isFollowed: isArtistFollowed(a.artistId, a.name),
        })),
      };
    }
    if (activeArtistDetail.value) {
      activeArtistDetail.value = {
        ...activeArtistDetail.value,
        isFollowed: isArtistFollowed(
          activeArtistDetail.value.artistId,
          activeArtistDetail.value.name
        ),
      };
    }
  }

  async function loadFollowedArtists() {
    if (!isLoggedIn.value) return;
    try {
      const list = await kugouClient.getFollowedArtists();
      followedArtists.value = applySavedOrder(
        list.map((a) => ({ ...a, isFollowed: true })),
        artistsOrderIds.value,
        (a) => String(a.singerId || a.artistId || a.name)
      );
      syncArtistFollowFlags();
    } catch (e) {
      console.warn("loadFollowedArtists error:", e);
    }
  }

  async function toggleFollowArtist(artist: FollowedArtist) {
    const nextState = !artist.isFollowed;
    try {
      const ok = await kugouClient.toggleFollowArtist(
        artist.artistId,
        nextState,
        artist.targetUserId
      );
      if (!ok) {
        throw new Error("云端未确认关注状态变更");
      }
      artist.isFollowed = nextState;
      const targetId = String(artist.artistId || artist.singerId || "").trim();
      const targetName = String(artist.name || "").trim().toLowerCase();
      if (nextState) {
        if (!isArtistFollowed(artist.artistId, artist.name)) {
          followedArtists.value.unshift({ ...artist, isFollowed: true });
        }
        notify(`已关注歌手「${artist.name}」`);
      } else {
        followedArtists.value = followedArtists.value.filter((a) => {
          const aId = String(a.artistId || a.singerId || "").trim();
          const aName = String(a.name || "").trim().toLowerCase();
          if (targetId && targetId !== "0" && aId && aId !== "0" && aId === targetId) {
            return false;
          }
          if (targetName && aName && aName === targetName) {
            return false;
          }
          return true;
        });
        notify(`已取消关注「${artist.name}」`);
      }
      syncArtistFollowFlags();
    } catch (e: any) {
      notify(`操作失败：${String(e)}`);
    }
  }

  async function loadPurchasedAssets() {
    if (!isLoggedIn.value) return;
    try {
      const [albums, songs] = await Promise.all([
        kugouClient.getPurchasedAlbums().catch(() => []),
        kugouClient.getPurchasedSongs().catch(() => []),
      ]);
      purchasedAlbums.value = applySavedOrder(
        albums,
        purchasedOrderIds.value,
        (a) => String(a.id || a.albumId || a.albumName)
      );
      purchasedSongs.value = songs;
    } catch (e) {
      console.warn("loadPurchasedAssets error:", e);
    }
  }

  function reorderLibraryAssetList(
    category: "created" | "collected" | "artists" | "purchased",
    fromIndex: number,
    toIndex: number,
    customKeys?: string[]
  ) {
    if (fromIndex === toIndex && !customKeys) return;
    if (category === "created") {
      const current = [...createdPlaylists.value];
      if (
        fromIndex < 0 ||
        fromIndex >= current.length ||
        toIndex < 0 ||
        toIndex >= current.length
      )
        return;
      const [moved] = current.splice(fromIndex, 1);
      current.splice(toIndex, 0, moved);
      createdOrderIds.value = current.map((p) =>
        String(p.id || p.listId || p.title)
      );
      localStorage.setItem(
        "bm_order_created",
        JSON.stringify(createdOrderIds.value)
      );
    } else if (category === "collected") {
      const current = [...collectedPlaylists.value];
      if (
        fromIndex < 0 ||
        fromIndex >= current.length ||
        toIndex < 0 ||
        toIndex >= current.length
      )
        return;
      const [moved] = current.splice(fromIndex, 1);
      current.splice(toIndex, 0, moved);
      collectedOrderIds.value = current.map((p) =>
        String(p.id || p.globalCollectionId || p.title)
      );
      localStorage.setItem(
        "bm_order_collected",
        JSON.stringify(collectedOrderIds.value)
      );
    } else if (category === "artists") {
      const current = [...followedArtists.value];
      if (
        fromIndex < 0 ||
        fromIndex >= current.length ||
        toIndex < 0 ||
        toIndex >= current.length
      )
        return;
      const [moved] = current.splice(fromIndex, 1);
      current.splice(toIndex, 0, moved);
      followedArtists.value = current;
      artistsOrderIds.value = current.map((a) =>
        String(a.singerId || a.artistId || a.name)
      );
      localStorage.setItem(
        "bm_order_artists",
        JSON.stringify(artistsOrderIds.value)
      );
    } else if (category === "purchased") {
      if (customKeys) {
        purchasedOrderIds.value = [...customKeys];
        localStorage.setItem(
          "bm_order_purchased",
          JSON.stringify(purchasedOrderIds.value)
        );
        purchasedAlbums.value = applySavedOrder(
          purchasedAlbums.value,
          purchasedOrderIds.value,
          (a) => String(a.id || a.albumId || a.albumName)
        );
      } else {
        const current = [...purchasedAlbums.value];
        if (
          fromIndex < 0 ||
          fromIndex >= current.length ||
          toIndex < 0 ||
          toIndex >= current.length
        )
          return;
        const [moved] = current.splice(fromIndex, 1);
        current.splice(toIndex, 0, moved);
        purchasedAlbums.value = current;
        purchasedOrderIds.value = current.map((a) =>
          String(a.id || a.albumId || a.albumName)
        );
        localStorage.setItem(
          "bm_order_purchased",
          JSON.stringify(purchasedOrderIds.value)
        );
      }
    }
  }

  const artistSongsPage = ref<number>(1);
  const artistSongsHasMore = ref<boolean>(true);
  const isLoadingMoreArtistSongs = ref<boolean>(false);

  const artistAlbumsPage = ref<number>(1);
  const artistAlbumsSort = ref<"new" | "hot">("new");
  const artistAlbumsHasMore = ref<boolean>(true);
  const isLoadingMoreArtistAlbums = ref<boolean>(false);

  function findKnownArtistMeta(artistId: string, name: string): FollowedArtist | undefined {
    const idStr = (artistId || "").trim();
    const nameStr = (name || "").trim().toLowerCase();
    const pools = [
      searchResultsArtists.value,
      searchDiscovery.value.hotArtists,
      followedArtists.value,
    ];
    for (const pool of pools) {
      const hit = pool.find((a) => {
        const aId = String(a.artistId || a.singerId || "").trim();
        if (idStr && idStr !== "0" && aId && aId !== "0" && aId === idStr) return true;
        const aName = String(a.name || "").trim().toLowerCase();
        return Boolean(nameStr && aName && aName === nameStr);
      });
      if (hit && (hit.fansCount > 0 || hit.songCount > 0 || hit.albumCount > 0)) {
        return hit;
      }
    }
    return undefined;
  }

  async function openArtistDetail(
    artistId: string,
    fallbackName = "",
    fallbackAvatar = ""
  ) {
    const reqSeq = ++activeArtistReqSeq;
    const knownMeta = findKnownArtistMeta(artistId, fallbackName);
    const candidateId = (artistId || knownMeta?.artistId || "").trim();
    const cached =
      candidateId && candidateId !== "0"
        ? artistSwrCache.get(candidateId)
        : undefined;

    pushNavHistory();
    activeTab.value = "artist_detail";
    libraryDetailMode.value = "artist";
    artistDetailSubTab.value = "songs";
    artistSongsSort.value = "hot";
    artistSongsPage.value = 1;
    artistSongsHasMore.value = true;
    artistAlbumsPage.value = 1;
    artistAlbumsHasMore.value = true;
    isNowPlayingOpen.value = false;
    isSearchPaletteOpen.value = false;
    isMultiArtistModalOpen.value = false;
    scrollStageToTop();

    if (cached && cached.songs.length > 0) {
      activeArtistDetail.value = {
        ...cached.detail,
        isFollowed: isArtistFollowed(
          cached.detail.artistId || candidateId,
          cached.detail.name || fallbackName
        ),
      };
      activeArtistSongs.value = cached.songs;
      activeArtistAlbums.value = cached.albums;
      isLoadingArtist.value = false;
    } else {
      activeArtistSongs.value = [];
      activeArtistAlbums.value = [];
      isLoadingArtist.value = true;
      activeArtistDetail.value = {
        artistId: candidateId || "0",
        targetUserId: knownMeta?.targetUserId || "",
        name: fallbackName || knownMeta?.name || "歌手主页",
        avatarUrl: fallbackAvatar || knownMeta?.avatarUrl || "",
        songCount: knownMeta?.songCount || 0,
        albumCount: knownMeta?.albumCount || 0,
        fansCount: knownMeta?.fansCount || 0,
        isFollowed: isArtistFollowed(candidateId, fallbackName || knownMeta?.name),
        intro: knownMeta?.intro || "",
      };
    }

    try {
      let resolvedId = candidateId;
      let matchedExact: FollowedArtist | undefined = knownMeta;
      if ((!resolvedId || resolvedId === "0") && fallbackName) {
        const matchedArtists = await kugouClient.searchArtists(fallbackName).catch(() => []);
        if (reqSeq !== activeArtistReqSeq) return;
        const exact =
          matchedArtists.find((a) => a.name.toLowerCase() === fallbackName.trim().toLowerCase()) ||
          matchedArtists[0];
        if (exact && exact.artistId && exact.artistId !== "0") {
          resolvedId = exact.artistId;
          matchedExact = exact;
          if (!fallbackAvatar && exact.avatarUrl) {
            fallbackAvatar = exact.avatarUrl;
          }
          const resolvedCached = artistSwrCache.get(resolvedId);
          if (resolvedCached && resolvedCached.songs.length > 0 && activeArtistSongs.value.length === 0) {
            activeArtistDetail.value = {
              ...resolvedCached.detail,
              isFollowed: isArtistFollowed(resolvedId, resolvedCached.detail.name || fallbackName),
            };
            activeArtistSongs.value = resolvedCached.songs;
            activeArtistAlbums.value = resolvedCached.albums;
            isLoadingArtist.value = false;
          }
        }
      }

      if (resolvedId && resolvedId !== "0") {
        const songsPromise = kugouClient.getArtistSongs(resolvedId, artistSongsSort.value, 1);
        const detailPromise = kugouClient.getArtistDetail(resolvedId).catch(() => null);
        const albumsPromise = kugouClient
          .getArtistAlbums(resolvedId, 1, artistAlbumsSort.value)
          .catch(() => [] as AlbumItem[]);

        // 单曲列表返回后立即解除加载阻塞，实现歌手页秒开
        const songs = await songsPromise;
        if (reqSeq !== activeArtistReqSeq) return;
        activeArtistSongs.value = songs;
        isLoadingArtist.value = false;

        const fallbackFans = Math.max(
          knownMeta?.fansCount || 0,
          matchedExact?.fansCount || 0
        );
        const fallbackSongCount = Math.max(
          knownMeta?.songCount || 0,
          matchedExact?.songCount || 0,
          songs.length
        );
        if (activeArtistDetail.value) {
          activeArtistDetail.value.artistId = resolvedId;
          activeArtistDetail.value.songCount = Math.max(
            activeArtistDetail.value.songCount || 0,
            fallbackSongCount
          );
          activeArtistDetail.value.fansCount = Math.max(
            activeArtistDetail.value.fansCount || 0,
            fallbackFans
          );
          if (!activeArtistDetail.value.avatarUrl && songs[0]?.coverUrl) {
            activeArtistDetail.value.avatarUrl = songs[0].coverUrl;
          }
        }

        const [detail, albums] = await Promise.all([detailPromise, albumsPromise]);
        if (reqSeq !== activeArtistReqSeq) return;

        const fallbackAlbumCount = Math.max(
          knownMeta?.albumCount || 0,
          matchedExact?.albumCount || 0,
          albums.length
        );
        if (detail) {
          detail.isFollowed = isArtistFollowed(detail.artistId || resolvedId, detail.name || fallbackName);
          if (!detail.avatarUrl) detail.avatarUrl = fallbackAvatar || songs[0]?.coverUrl || "";
          detail.songCount = Math.max(detail.songCount || 0, fallbackSongCount);
          detail.albumCount = Math.max(detail.albumCount || 0, fallbackAlbumCount);
          detail.fansCount = Math.max(detail.fansCount || 0, fallbackFans);
          activeArtistDetail.value = detail;
        } else if (activeArtistDetail.value) {
          activeArtistDetail.value.artistId = resolvedId;
          activeArtistDetail.value.songCount = fallbackSongCount;
          activeArtistDetail.value.albumCount = fallbackAlbumCount;
          activeArtistDetail.value.fansCount = fallbackFans;
        }
        activeArtistAlbums.value = albums;
        const totalSongs = activeArtistDetail.value?.songCount || 0;
        const totalAlbums = activeArtistDetail.value?.albumCount || 0;
        artistSongsHasMore.value =
          songs.length >= 25 && (totalSongs === 0 || songs.length < totalSongs);
        artistAlbumsHasMore.value =
          albums.length >= 30 || (totalAlbums > 0 && albums.length > 0 && albums.length < totalAlbums);
        if (activeArtistDetail.value) {
          artistSwrCache.set(resolvedId, {
            detail: { ...activeArtistDetail.value },
            songs: [...songs],
            albums: [...albums],
          });
        }
      } else if (fallbackName) {
        const songsPromise = kugouClient.searchSongs(fallbackName, 1, 50);
        const albumsPromise = kugouClient.searchAlbums(fallbackName).catch(() => [] as AlbumItem[]);
        const songs = await songsPromise;
        if (reqSeq !== activeArtistReqSeq) return;
        activeArtistSongs.value = songs;
        isLoadingArtist.value = false;
        const albums = await albumsPromise;
        if (reqSeq !== activeArtistReqSeq) return;
        activeArtistAlbums.value = albums;
        artistSongsHasMore.value = false;
        artistAlbumsHasMore.value = false;
        if (activeArtistDetail.value) {
          if (!activeArtistDetail.value.avatarUrl && songs[0]?.coverUrl) {
            activeArtistDetail.value.avatarUrl = songs[0].coverUrl;
          }
          activeArtistDetail.value.songCount = Math.max(
            activeArtistDetail.value.songCount || 0,
            songs.length
          );
          activeArtistDetail.value.albumCount = Math.max(
            activeArtistDetail.value.albumCount || 0,
            albums.length
          );
        }
      }
    } catch (e: any) {
      if (reqSeq === activeArtistReqSeq && !cached) {
        notify(`加载歌手作品失败：${String(e)}`);
      }
    } finally {
      if (reqSeq === activeArtistReqSeq) {
        isLoadingArtist.value = false;
      }
    }
  }

  async function loadMoreArtistSongs() {
    const aid = activeArtistDetail.value?.artistId || activeArtistDetail.value?.singerId;
    if (!aid || aid === "0" || isLoadingMoreArtistSongs.value || !artistSongsHasMore.value) return;
    const reqSeq = activeArtistReqSeq;
    const targetAid = String(aid);
    isLoadingMoreArtistSongs.value = true;
    try {
      const nextPage = artistSongsPage.value + 1;
      const more = await kugouClient.getArtistSongs(targetAid, artistSongsSort.value, nextPage);
      if (reqSeq !== activeArtistReqSeq) return;
      if (more.length === 0) {
        artistSongsHasMore.value = false;
      } else {
        const seen = new Set(activeArtistSongs.value.map((s) => s.hash));
        const unique = more.filter((s) => !seen.has(s.hash));
        activeArtistSongs.value = [...activeArtistSongs.value, ...unique];
        artistSongsPage.value = nextPage;
        const totalSongs = activeArtistDetail.value?.songCount || 0;
        if (
          more.length < 25 ||
          unique.length === 0 ||
          (totalSongs > 0 && activeArtistSongs.value.length >= totalSongs)
        ) {
          artistSongsHasMore.value = false;
        }
        // 同步更新 SWR 缓存，确保切页返回后保留已自动加载的完整曲单
        if (activeArtistDetail.value) {
          artistSwrCache.set(targetAid, {
            detail: activeArtistDetail.value,
            songs: activeArtistSongs.value,
            albums: activeArtistAlbums.value,
          });
        }
      }
    } catch (e) {
      if (reqSeq === activeArtistReqSeq) {
        throw e;
      }
    } finally {
      if (reqSeq === activeArtistReqSeq) {
        isLoadingMoreArtistSongs.value = false;
      }
    }
  }

  async function loadMoreArtistAlbums() {
    const aid = activeArtistDetail.value?.artistId || activeArtistDetail.value?.singerId;
    if (!aid || aid === "0" || isLoadingMoreArtistAlbums.value || !artistAlbumsHasMore.value) return;
    const reqSeq = activeArtistReqSeq;
    const targetAid = String(aid);
    isLoadingMoreArtistAlbums.value = true;
    try {
      const nextPage = artistAlbumsPage.value + 1;
      const more = await kugouClient.getArtistAlbums(targetAid, nextPage, artistAlbumsSort.value);
      if (reqSeq !== activeArtistReqSeq) return;
      if (more.length === 0) {
        artistAlbumsHasMore.value = false;
      } else {
        const seen = new Set(
          activeArtistAlbums.value.map((a) => `${a.albumId}_${a.albumName}`)
        );
        const unique = more.filter((a) => {
          const k = `${a.albumId}_${a.albumName}`;
          if (seen.has(k)) return false;
          seen.add(k);
          return true;
        });
        activeArtistAlbums.value = [...activeArtistAlbums.value, ...unique];
        artistAlbumsPage.value = nextPage;
        const totalAlbums = activeArtistDetail.value?.albumCount || 0;
        if (
          unique.length === 0 ||
          (totalAlbums > 0 && activeArtistAlbums.value.length >= totalAlbums)
        ) {
          artistAlbumsHasMore.value = false;
        }
        if (activeArtistDetail.value) {
          artistSwrCache.set(targetAid, {
            detail: activeArtistDetail.value,
            songs: activeArtistSongs.value,
            albums: activeArtistAlbums.value,
          });
        }
      }
    } catch (e) {
      if (reqSeq === activeArtistReqSeq) {
        throw e;
      }
    } finally {
      if (reqSeq === activeArtistReqSeq) {
        isLoadingMoreArtistAlbums.value = false;
      }
    }
  }

  async function switchArtistAlbumSort(sort: "new" | "hot") {
    if (artistAlbumsSort.value === sort && activeArtistAlbums.value.length > 0) return;
    artistAlbumsSort.value = sort;
    artistAlbumsPage.value = 1;
    const aid = activeArtistDetail.value?.artistId || activeArtistDetail.value?.singerId;
    if (aid && aid !== "0") {
      isLoadingMoreArtistAlbums.value = true;
      try {
        const albums = await kugouClient.getArtistAlbums(String(aid), 1, sort);
        activeArtistAlbums.value = albums;
        const totalAlbums = activeArtistDetail.value?.albumCount || 0;
        artistAlbumsHasMore.value =
          albums.length >= 30 ||
          (totalAlbums > 0 && albums.length > 0 && albums.length < totalAlbums);
        notify(sort === "hot" ? "已切换为最热专辑排序" : "已切换为最新发行专辑排序");
      } catch {
      } finally {
        isLoadingMoreArtistAlbums.value = false;
      }
    }
  }

  async function openAlbumDetail(
    albumId: string,
    fallbackName = "",
    fallbackArtist = "",
    fallbackCover = ""
  ) {
    const reqSeq = ++activeAlbumReqSeq;
    const cleanId = (albumId || "").trim();
    const cached = cleanId && cleanId !== "0" ? albumSwrCache.get(cleanId) : undefined;

    pushNavHistory();
    activeTab.value = "album_detail";
    libraryDetailMode.value = "album";
    isNowPlayingOpen.value = false;
    isSearchPaletteOpen.value = false;
    scrollStageToTop();

    if (cached && cached.songs.length > 0) {
      activeAlbumDetail.value = { ...cached.detail };
      activeAlbumSongs.value = cached.songs;
      isLoadingAlbum.value = false;
    } else {
      activeAlbumSongs.value = [];
      isLoadingAlbum.value = true;
      activeAlbumDetail.value = {
        albumId: cleanId || "0",
        albumName: fallbackName || "专辑详情",
        artistName: fallbackArtist,
        artistId: "",
        coverUrl: fallbackCover,
        publishDate: "",
        publishCompany: "酷狗概念版数字发行",
        songCount: 0,
        intro: "",
      };
    }

    try {
      if (cleanId && cleanId !== "0") {
        const songsPromise = kugouClient.getAlbumSongs(cleanId);
        const detailPromise = kugouClient.getAlbumDetail(cleanId).catch(() => null);

        const songs = await songsPromise;
        if (reqSeq !== activeAlbumReqSeq) return;
        activeAlbumSongs.value = songs;
        isLoadingAlbum.value = false;

        if (activeAlbumDetail.value) {
          activeAlbumDetail.value.songCount = Math.max(
            activeAlbumDetail.value.songCount || 0,
            songs.length
          );
          if (!activeAlbumDetail.value.coverUrl && songs[0]?.coverUrl) {
            activeAlbumDetail.value.coverUrl = songs[0].coverUrl;
          }
        }

        const detail = await detailPromise;
        if (reqSeq !== activeAlbumReqSeq) return;
        if (detail) {
          activeAlbumDetail.value = {
            ...detail,
            coverUrl:
              detail.coverUrl || fallbackCover || songs[0]?.coverUrl || "",
            songCount: songs.length || detail.songCount,
          };
        }
        if (activeAlbumDetail.value) {
          albumSwrCache.set(cleanId, {
            detail: { ...activeAlbumDetail.value },
            songs: [...songs],
          });
        }
      } else if (fallbackName) {
        const songs = await kugouClient.searchSongs(
          `${fallbackArtist} ${fallbackName}`.trim(),
          1,
          25
        );
        if (reqSeq !== activeAlbumReqSeq) return;
        activeAlbumSongs.value = songs;
      }
    } catch (e: any) {
      if (reqSeq === activeAlbumReqSeq && !cached) {
        notify(`加载专辑曲目失败：${String(e)}`);
      }
    } finally {
      if (reqSeq === activeAlbumReqSeq) {
        isLoadingAlbum.value = false;
      }
    }
  }

  async function navigateToSongArtist(song: Song | null | undefined, specificAuthor?: SongAuthor) {
    if (!song && !specificAuthor) return;
    if (specificAuthor) {
      isMultiArtistModalOpen.value = false;
      await openArtistDetail(specificAuthor.id || "0", specificAuthor.name, specificAuthor.avatar || song?.coverUrl || "");
      return;
    }
    if (!song) return;
    const authors = song.authors && song.authors.length > 0 ? song.authors : splitSongAuthors(song.artist, song.artistId, song.coverUrl);
    if (authors.length > 1) {
      multiArtistCandidates.value = authors;
      multiArtistSongTitle.value = song.title;
      isMultiArtistModalOpen.value = true;
      return;
    }
    const target = authors[0] || { id: song.artistId || "0", name: song.artist || "未知歌手", avatar: song.coverUrl };
    await openArtistDetail(target.id || song.artistId || "0", target.name, target.avatar || song.coverUrl || "");
  }

  async function navigateToSongAlbum(song: Song | null | undefined) {
    if (!song) return;
    let albumId = (song.albumId || "").trim();
    let albumName = (song.album || "").trim();
    let artistId = (song.artistId || "").trim();
    let artistName = (song.artist || "").trim();
    let coverUrl = song.coverUrl || "";

    if ((!albumId || albumId === "0") && song.hash) {
      try {
        const resolved = await kugouClient.resolveSongAlbumMeta(song.hash, song.albumAudioId);
        if (resolved.albumId && resolved.albumId !== "0") {
          albumId = resolved.albumId;
          song.albumId = resolved.albumId;
        }
        if (resolved.albumName && !albumName) {
          albumName = resolved.albumName;
          song.album = resolved.albumName;
        }
        if (resolved.artistId && (!artistId || artistId === "0")) {
          artistId = resolved.artistId;
          song.artistId = resolved.artistId;
        }
        if (resolved.artistName && !artistName) {
          artistName = resolved.artistName;
        }
        if (resolved.coverUrl && !coverUrl) {
          coverUrl = resolved.coverUrl;
        }
      } catch {
        // fallback to search inside openAlbumDetail
      }
    }

    if ((!albumId || albumId === "0") && !albumName) {
      notify("该曲目暂无关联专辑信息");
      return;
    }
    await openAlbumDetail(albumId || "0", albumName || song.title, artistName, coverUrl);
  }


  async function switchArtistSongSort(sort: "hot" | "new") {
    artistSongsSort.value = sort;
    artistSongsPage.value = 1;
    const aid = activeArtistDetail.value?.artistId || activeArtistDetail.value?.singerId;
    if (aid && aid !== "0") {
      try {
        const songs = await kugouClient.getArtistSongs(String(aid), sort, 1);
        activeArtistSongs.value = songs;
        const totalSongs = activeArtistDetail.value?.songCount || 0;
        artistSongsHasMore.value =
          songs.length >= 25 && (totalSongs === 0 || songs.length < totalSongs);
        notify(sort === "new" ? "已切换为最新发行单曲排序" : "已切换为最热代表作排序");
      } catch {}
    }
  }


  function clearSessionPlayedSongs() {
    sessionPlayedSongs = [];
  }

  function clearAccountSwrCaches() {
    playlistSwrCache.clear();
    artistSwrCache.clear();
    albumSwrCache.clear();
    selectedPlaylist.value = null;
    collectedRemotePlaylists.value = [];
    activeArtistDetail.value = null;
    activeArtistSongs.value = [];
    activeArtistAlbums.value = [];
    activeAlbumDetail.value = null;
    activeAlbumSongs.value = [];
  }

  const cloudDiskSongs = ref<Song[]>([]);
  const cloudDiskTotal = ref<number>(0);
  const cloudDiskUsedBytes = ref<number>(0);
  const cloudDiskMaxBytes = ref<number>(0);
  const cloudDiskPage = ref<number>(1);
  const cloudDiskHasMore = ref<boolean>(false);
  const isLoadingCloudDisk = ref<boolean>(false);

  async function loadUserCloudDisk(force = false, silent = false) {
    if (!isLoggedIn.value) {
      cloudDiskSongs.value = [];
      cloudDiskTotal.value = 0;
      cloudDiskUsedBytes.value = 0;
      cloudDiskMaxBytes.value = 0;
      cloudDiskPage.value = 1;
      cloudDiskHasMore.value = false;
      return;
    }
    if (!force && cloudDiskSongs.value.length > 0) return;
    const showLoadingState = !silent || cloudDiskSongs.value.length === 0;
    if (showLoadingState) {
      isLoadingCloudDisk.value = true;
    }
    try {
      const res = await kugouClient.getUserCloudSongs(1, 100);
      const nextItems = res.items || [];
      // 静默后台刷新若接口返回空而本地已有曲目时保留旧数据，防止网络抖动清空列表
      if (!silent || nextItems.length > 0 || cloudDiskSongs.value.length === 0) {
        cloudDiskSongs.value = nextItems;
        cloudDiskTotal.value = res.total || cloudDiskSongs.value.length;
        cloudDiskUsedBytes.value = res.usedSizeBytes || 0;
        cloudDiskMaxBytes.value = res.maxSizeBytes || 0;
        cloudDiskPage.value = res.page || 1;
        cloudDiskHasMore.value = Boolean(res.hasMore);
      }
    } catch (e: any) {
      if (!silent) {
        notify(`获取云盘曲目失败：${String(e)}`);
      }
    } finally {
      if (showLoadingState) {
        isLoadingCloudDisk.value = false;
      }
    }
  }

  async function loadMoreCloudDisk() {
    if (!isLoggedIn.value || isLoadingCloudDisk.value || !cloudDiskHasMore.value) return;
    isLoadingCloudDisk.value = true;
    try {
      const nextPage = cloudDiskPage.value + 1;
      const res = await kugouClient.getUserCloudSongs(nextPage, 100);
      const seen = new Set(cloudDiskSongs.value.map((s) => `${s.fileId || ""}_${s.hash || s.id}`));
      const extra = (res.items || []).filter((s) => {
        const key = `${s.fileId || ""}_${s.hash || s.id}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      cloudDiskSongs.value = [...cloudDiskSongs.value, ...extra];
      cloudDiskTotal.value = Math.max(res.total || 0, cloudDiskSongs.value.length);
      if (res.usedSizeBytes) cloudDiskUsedBytes.value = res.usedSizeBytes;
      if (res.maxSizeBytes) cloudDiskMaxBytes.value = res.maxSizeBytes;
      cloudDiskPage.value = nextPage;
      cloudDiskHasMore.value = Boolean(res.hasMore) && extra.length > 0;
    } catch (e: any) {
      notify(`加载更多云盘歌曲失败：${String(e)}`);
    } finally {
      isLoadingCloudDisk.value = false;
    }
  }

  async function deleteCloudDiskSongs(songsToDelete: Song[]): Promise<boolean> {
    if (!isLoggedIn.value || songsToDelete.length === 0) return false;
    const items = songsToDelete
      .map((s) => ({
        kvId: String(s.fileId || s.albumAudioId || ""),
        albumAudioId: s.albumAudioId ? String(s.albumAudioId) : undefined,
      }))
      .filter((it) => it.kvId.length > 0);
    if (items.length === 0) {
      notify("所选云盘歌曲缺少云盘标识 (kvId)，无法删除");
      return false;
    }
    try {
      const ok = await kugouClient.deleteCloudSongs(items);
      if (!ok) {
        notify("删除云盘歌曲未获云端确认");
        return false;
      }
      const deletedHashes = new Set(songsToDelete.map((s) => (s.hash || s.id || "").toLowerCase()));
      const deletedFileIds = new Set(songsToDelete.map((s) => Number(s.fileId || 0)).filter(Boolean));
      cloudDiskSongs.value = cloudDiskSongs.value.filter(
        (s) =>
          !deletedFileIds.has(Number(s.fileId || 0)) &&
          !deletedHashes.has((s.hash || s.id || "").toLowerCase())
      );
      cloudDiskTotal.value = Math.max(0, cloudDiskTotal.value - items.length);
      notify(`已从云盘永久删除 ${items.length} 首曲目`);
      loadUserCloudDisk(true);
      return true;
    } catch (e: any) {
      notify(`删除云盘曲目失败：${String(e)}`);
      return false;
    }
  }

  const LOCAL_SAVED_DIRS_KEY = "bm_saved_local_dirs_v1";
  const LOCAL_SETTINGS_KEY = "bm_local_music_settings_v1";

  const DEFAULT_LOCAL_MUSIC_SETTINGS: LocalMusicSettings = {
    autoScanOnStartup: false,
    scanSubfolders: true,
    minDurationSec: 0,
    minFileSizeKb: 0,
    preferEmbeddedTags: true,
    deduplicate: true,
  };

  function loadPersistedLocalDirs(): string[] {
    try {
      const raw = localStorage.getItem(LOCAL_SAVED_DIRS_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((s) => String(s || "").trim())
        .filter(Boolean)
        .slice(0, 12);
    } catch {
      return [];
    }
  }

  function loadPersistedLocalSettings(): LocalMusicSettings {
    try {
      const raw = localStorage.getItem(LOCAL_SETTINGS_KEY);
      if (!raw) return { ...DEFAULT_LOCAL_MUSIC_SETTINGS };
      const parsed = JSON.parse(raw);
      return {
        autoScanOnStartup: Boolean(parsed?.autoScanOnStartup),
        scanSubfolders: parsed?.scanSubfolders !== false,
        minDurationSec: Math.max(0, Math.min(300, Number(parsed?.minDurationSec) || 0)),
        minFileSizeKb: Math.max(0, Math.min(51200, Number(parsed?.minFileSizeKb) || 0)),
        preferEmbeddedTags: parsed?.preferEmbeddedTags !== false,
        deduplicate: parsed?.deduplicate !== false,
      };
    } catch {
      return { ...DEFAULT_LOCAL_MUSIC_SETTINGS };
    }
  }

  const localMusicSongs = ref<Song[]>([]);
  const localMusicDir = ref<string>(
    localStorage.getItem("bm_local_music_dir") || ""
  );
  const localMusicSavedDirs = ref<string[]>(loadPersistedLocalDirs());
  const localMusicSettings = ref<LocalMusicSettings>(loadPersistedLocalSettings());
  const isScanningLocalMusic = ref<boolean>(false);

  function persistLocalMusicSavedDirs(list: string[]) {
    const deduped: string[] = [];
    const seen = new Set<string>();
    for (const item of list) {
      const clean = String(item || "").trim();
      if (!clean) continue;
      const norm = clean.replace(/[\\/]+$/, "").toLowerCase();
      if (seen.has(norm)) continue;
      seen.add(norm);
      deduped.push(clean);
      if (deduped.length >= 12) break;
    }
    localMusicSavedDirs.value = deduped;
    try {
      localStorage.setItem(LOCAL_SAVED_DIRS_KEY, JSON.stringify(deduped));
    } catch {
      // ignore storage quota errors
    }
  }

  function addLocalMusicSavedDir(dir: string) {
    const clean = String(dir || "").trim();
    if (!clean) return;
    persistLocalMusicSavedDirs([clean, ...localMusicSavedDirs.value]);
  }

  function removeLocalMusicSavedDir(dir: string) {
    const targetNorm = String(dir || "").replace(/[\\/]+$/, "").toLowerCase();
    persistLocalMusicSavedDirs(
      localMusicSavedDirs.value.filter(
        (d) => d.replace(/[\\/]+$/, "").toLowerCase() !== targetNorm
      )
    );
  }

  function updateLocalMusicSettings(patch: Partial<LocalMusicSettings>) {
    const next: LocalMusicSettings = {
      ...localMusicSettings.value,
      ...patch,
      minDurationSec: Math.max(
        0,
        Math.min(300, Number(patch.minDurationSec ?? localMusicSettings.value.minDurationSec) || 0)
      ),
      minFileSizeKb: Math.max(
        0,
        Math.min(51200, Number(patch.minFileSizeKb ?? localMusicSettings.value.minFileSizeKb) || 0)
      ),
    };
    localMusicSettings.value = next;
    try {
      localStorage.setItem(LOCAL_SETTINGS_KEY, JSON.stringify(next));
    } catch {
      // ignore
    }
  }

  async function scanLocalMusicLibrary(
    customDir?: string,
    silent = false,
    forceRebuildCache = false
  ) {
    if (isScanningLocalMusic.value) return;
    const showLoadingState = !silent || localMusicSongs.value.length === 0;
    if (showLoadingState) {
      isScanningLocalMusic.value = true;
    }
    const targetDir =
      typeof customDir === "string" ? customDir.trim() : localMusicDir.value.trim();
    try {
      const songs = await kugouClient.scanLocalMusicDir(targetDir, {
        scanSubfolders: localMusicSettings.value.scanSubfolders,
        minDurationSec: localMusicSettings.value.minDurationSec,
        minFileSizeKb: localMusicSettings.value.minFileSizeKb,
        preferEmbeddedTags: localMusicSettings.value.preferEmbeddedTags,
        deduplicate: localMusicSettings.value.deduplicate,
        forceRebuildCache,
      });
      localMusicSongs.value = songs;
      if (targetDir) {
        localMusicDir.value = targetDir;
        localStorage.setItem("bm_local_music_dir", targetDir);
        addLocalMusicSavedDir(targetDir);
      }
      if (!silent) {
        notify(
          forceRebuildCache
            ? `已重建元数据与封面缓存，共索引 ${songs.length} 首本地音乐`
            : songs.length > 0
            ? `已扫描本地音乐文件夹，共发现 ${songs.length} 首音频文件`
            : "当前目录未扫描到符合筛选条件的音频文件 (.flac / .mp3 / .wav / .ogg / .opus / .m4a / .aac / .alac / .aiff)"
        );
      }
    } catch (e: any) {
      if (!silent) {
        notify(`扫描本地音乐目录失败：${String(e)}`);
      }
    } finally {
      if (showLoadingState) {
        isScanningLocalMusic.value = false;
      }
    }
  }

  async function rebuildLocalMusicCache() {
    await scanLocalMusicLibrary(localMusicDir.value, false, true);
  }

  async function pickAndScanLocalMusicDir() {
    try {
      const picked = await kugouClient.pickFolderDialog(
        localMusicDir.value,
        "选择要扫描导入的本地音乐文件夹"
      );
      if (picked && picked.trim()) {
        localMusicDir.value = picked.trim();
        localStorage.setItem("bm_local_music_dir", localMusicDir.value);
        addLocalMusicSavedDir(localMusicDir.value);
        await scanLocalMusicLibrary(localMusicDir.value);
      }
    } catch (e: any) {
      notify(`打开文件夹选择器失败：${String(e)}`);
    }
  }

  return {
    localMusicSongs,
    localMusicDir,
    localMusicSavedDirs,
    localMusicSettings,
    isScanningLocalMusic,
    addLocalMusicSavedDir,
    removeLocalMusicSavedDir,
    updateLocalMusicSettings,
    scanLocalMusicLibrary,
    rebuildLocalMusicCache,
    pickAndScanLocalMusicDir,
    cloudDiskSongs,
    cloudDiskTotal,
    cloudDiskUsedBytes,
    cloudDiskMaxBytes,
    cloudDiskHasMore,
    isLoadingCloudDisk,
    loadUserCloudDisk,
    loadMoreCloudDisk,
    deleteCloudDiskSongs,
    userPlaylists,
    selectedPlaylist,
    playlistSongs,
    likedSongs,
    recentHistorySongs,
    followedArtists,
    purchasedAlbums,
    purchasedSongs,
    collectedRemotePlaylists,
    libraryAssetTab,
    libraryDetailMode,
    isLibraryGridView,
    isLoadingLibrary,
    activeArtistDetail,
    activeArtistSongs,
    activeArtistAlbums,
    artistDetailSubTab,
    artistSongsSort,
    isLoadingArtist,
    activeAlbumDetail,
    activeAlbumSongs,
    isLoadingAlbum,
    createdOrderIds,
    collectedOrderIds,
    artistsOrderIds,
    purchasedOrderIds,
    createdPlaylists,
    collectedPlaylists,
    likedPlaylistMeta,
    artistSongsPage,
    artistSongsHasMore,
    isLoadingMoreArtistSongs,
    artistAlbumsPage,
    artistAlbumsSort,
    artistAlbumsHasMore,
    isLoadingMoreArtistAlbums,
    isSameHistoryTrack,
    recordSongToRecentHistory,
    loadRecentHistory,
    loadUserPlaylists,
    pushNavHistory,
    clearNavHistory,
    navigateBack,
    openSoundMixerFromPlayer,
    selectPlaylist,
    openPlaylistDetail,
    openLikedSongsDetail,
    openRecentHistoryDetail,
    openPurchasedSongsDetail,
    openCloudDrivePage,
    openLocalMusicPage,
    refreshCurrentPlaylistOnly,
    updatePlaylistMeta,
    batchAddSongsToPlaylist,
    batchRemoveSongsFromPlaylist,
    createNewPlaylist,
    deletePlaylist,
    removeSongFromCurrentPlaylist,
    importExternalPlaylist,
    isPlaylistCollected,
    toggleCollectPlaylist,
    addSongToUserPlaylist,
    isSongLiked,
    toggleFavoriteSong,
    isArtistFollowed,
    syncArtistFollowFlags,
    loadFollowedArtists,
    toggleFollowArtist,
    loadPurchasedAssets,
    reorderLibraryAssetList,
    openArtistDetail,
    loadMoreArtistSongs,
    loadMoreArtistAlbums,
    switchArtistAlbumSort,
    switchArtistSongSort,
    openAlbumDetail,
    navigateToSongArtist,
    navigateToSongAlbum,
    clearSessionPlayedSongs,
    clearAccountSwrCaches,
  };
}
