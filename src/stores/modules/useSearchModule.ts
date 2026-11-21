import { ref, reactive, watch, type Ref } from "vue";
import { kugouClient } from "../../api/kugou";
import type {
  AlbumItem,
  FollowedArtist,
  MainNavTab,
  Playlist,
  SearchCategoryTab,
  SearchDiscoveryData,
  Song,
} from "../../types";

export interface SearchModuleDeps {
  activeTab: Ref<MainNavTab>;
  isSearchPaletteOpen: Ref<boolean>;
  isNowPlayingOpen: Ref<boolean>;
  notify: (msg: string) => void;
  scrollStageToTop: () => void;
  isArtistFollowed: (artistId?: string, name?: string) => boolean;
  pushNavHistory?: () => void;
}

export function createSearchModule(deps: SearchModuleDeps) {
  const {
    activeTab,
    isSearchPaletteOpen,
    isNowPlayingOpen,
    notify,
    scrollStageToTop,
    isArtistFollowed,
    pushNavHistory,
  } = deps;

  // ================= 4. 独立搜索页（热搜探索 + 全量无上限结果）状态 =================
  const searchKeyword = ref<string>("");
  const searchViewMode = ref<"landing" | "results">("landing");
  const searchSuggestions = ref<string[]>([]);
  const searchCategoryTab = ref<SearchCategoryTab>("all");
  const hotSearchKeywords = ref<string[]>([]);
  const searchDiscovery = ref<SearchDiscoveryData>({
    hotSongs: [],
    hotBoards: [],
    hotArtists: [],
  });
  const isLoadingSearchDiscovery = ref(false);
  const hotArtistCategory = ref<number>(1);
  const isChangingHotArtistCat = ref(false);

  const searchHistory = ref<string[]>(
    (() => {
      try {
        const parsed = JSON.parse(localStorage.getItem("bm_search_history") || "[]");
        return Array.isArray(parsed)
          ? parsed.filter((v): v is string => typeof v === "string")
          : [];
      } catch {
        return [];
      }
    })()
  );
  const searchResultsSongs = ref<Song[]>([]);
  const searchResultsPlaylists = ref<Playlist[]>([]);
  const searchResultsAlbums = ref<AlbumItem[]>([]);
  const searchResultsArtists = ref<FollowedArtist[]>([]);
  const searchTotals = reactive({
    songs: 0,
    albums: 0,
    artists: 0,
    playlists: 0,
  });
  const searchPages = reactive({
    songs: 1,
    albums: 1,
    artists: 1,
    playlists: 1,
  });
  const searchHasMore = reactive({
    songs: false,
    albums: false,
    artists: false,
    playlists: false,
  });
  const isLoadingMoreSearch = reactive({
    songs: false,
    albums: false,
    artists: false,
    playlists: false,
  });
  const isSearching = ref(false);
  let activeSearchSeq = 0;

  let suggestDebounceTimer: number | null = null;
  watch(searchKeyword, (kw) => {
    if (suggestDebounceTimer) clearTimeout(suggestDebounceTimer);
    const trimmed = kw.trim();
    if (!trimmed) {
      searchSuggestions.value = [];
      return;
    }
    suggestDebounceTimer = window.setTimeout(async () => {
      try {
        searchSuggestions.value = await kugouClient.getSearchSuggest(trimmed);
      } catch {
        searchSuggestions.value = [];
      }
    }, 240);
  });


  // ================= 10. 独立搜索页探索与全量无上限多维搜索 =================
  let lastSearchDiscoverySyncAt = 0;

  async function loadSearchDiscovery(
    force = false,
    _section: "all" | "artists" | "songs" | "keywords" = "all"
  ) {
    if (![1, 2, 3].includes(hotArtistCategory.value)) {
      hotArtistCategory.value = 1;
    }
    const targetCat = hotArtistCategory.value;
    const hasExistingData =
      searchDiscovery.value.hotSongs.length > 0 ||
      searchDiscovery.value.hotArtists.length > 0 ||
      searchDiscovery.value.hotBoards.length > 0;
    const now = Date.now();
    // 仅对 3 秒内的重复挂载防抖，其余进入均真实请求官方最新接口
    if (!force && hasExistingData && now - lastSearchDiscoverySyncAt < 3_000) {
      return;
    }
    if (isLoadingSearchDiscovery.value) return;
    isLoadingSearchDiscovery.value = true;
    try {
      const [discRes, hotKwRes] = await Promise.allSettled([
        kugouClient.getSearchDiscovery(targetCat),
        kugouClient.getSearchHot(),
      ]);

      if (discRes.status === "fulfilled") {
        const data = discRes.value;
        const nextArtists = (data.hotArtists || []).map((a) => ({
          ...a,
          isFollowed: isArtistFollowed(a.artistId, a.name),
        }));
        const nextSongs = data.hotSongs || [];
        const nextBoards = (data.hotBoards || []).filter(
          (b) => b.entries && b.entries.length > 0
        );

        searchDiscovery.value = {
          hotArtists: nextArtists,
          hotSongs: nextSongs,
          hotBoards: nextBoards,
        };
      }

      if (hotKwRes.status === "fulfilled" && hotKwRes.value.length > 0) {
        hotSearchKeywords.value = hotKwRes.value;
      } else if (searchDiscovery.value.hotBoards.length > 0) {
        const fallbackKws = searchDiscovery.value.hotBoards[0].entries
          .map((e) => e.keyword)
          .filter(Boolean);
        if (fallbackKws.length > 0) {
          hotSearchKeywords.value = fallbackKws;
        }
      }

      lastSearchDiscoverySyncAt = Date.now();
    } catch (e) {
      console.warn("loadSearchDiscovery error:", e);
    } finally {
      isLoadingSearchDiscovery.value = false;
    }
  }

  async function switchHotArtistCategory(category: number) {
    const targetCat = [1, 2, 3].includes(category) ? category : 1;
    if (
      hotArtistCategory.value === targetCat &&
      searchDiscovery.value.hotArtists.length > 0
    ) {
      return;
    }
    hotArtistCategory.value = targetCat;
    isChangingHotArtistCat.value = true;
    try {
      const artists = await kugouClient.getHotArtists(targetCat, 24);
      if (artists.length > 0) {
        searchDiscovery.value = {
          ...searchDiscovery.value,
          hotArtists: artists.map((a) => ({
            ...a,
            isFollowed: isArtistFollowed(a.artistId, a.name),
          })),
        };
      }
    } catch (e) {
      console.warn("switchHotArtistCategory error:", e);
    } finally {
      isChangingHotArtistCat.value = false;
    }
  }

  function openSearchLanding(clearInput = true) {
    // 无论搜索了什么，返回搜索探索页（landing）一律清空搜索框残留内容
    if (clearInput || searchViewMode.value === "results") {
      searchKeyword.value = "";
    }
    searchKeyword.value = "";
    searchSuggestions.value = [];
    searchCategoryTab.value = "all";
    searchViewMode.value = "landing";
    scrollStageToTop();
    loadSearchDiscovery(true);
  }

  function buildSongSearchDedupKey(s: Song): string {
    return `${(s.hash || "").toLowerCase()}_${Number(s.albumAudioId) || 0}_${s.albumId || ""}_${s.id || ""}_${s.title || ""}`;
  }

  async function executeSearch(kw?: string) {
    const target = (kw !== undefined ? kw : searchKeyword.value).trim();
    if (!target) {
      openSearchLanding(true);
      return;
    }
    const seq = ++activeSearchSeq;
    if (!(activeTab.value === "search" && searchViewMode.value === "results")) {
      pushNavHistory?.();
    }
    searchKeyword.value = target;
    searchSuggestions.value = [];
    // 只要触发搜索，无条件切回结果页「综合」Tab
    searchCategoryTab.value = "all";
    activeTab.value = "search";
    searchViewMode.value = "results";
    isSearchPaletteOpen.value = false;
    isNowPlayingOpen.value = false;
    scrollStageToTop();

    searchHistory.value = [
      target,
      ...searchHistory.value.filter((item) => item !== target),
    ].slice(0, 20);
    localStorage.setItem(
      "bm_search_history",
      JSON.stringify(searchHistory.value)
    );

    searchPages.songs = 1;
    searchPages.albums = 1;
    searchPages.artists = 1;
    searchPages.playlists = 1;
    isLoadingMoreSearch.songs = false;
    isLoadingMoreSearch.albums = false;
    isLoadingMoreSearch.artists = false;
    isLoadingMoreSearch.playlists = false;

    searchResultsSongs.value = [];
    searchResultsArtists.value = [];
    searchResultsAlbums.value = [];
    searchResultsPlaylists.value = [];
    searchTotals.songs = 0;
    searchTotals.albums = 0;
    searchTotals.artists = 0;
    searchTotals.playlists = 0;

    isSearching.value = true;
    try {
      const songsTask = kugouClient
        .searchSongsPaged(target, 1, 30)
        .then((songsRes) => {
          if (seq !== activeSearchSeq) return;
          searchResultsSongs.value = songsRes.items;
          searchTotals.songs = Math.max(
            searchTotals.songs,
            songsRes.total || 0,
            songsRes.items.length
          );
          searchHasMore.songs = Boolean(songsRes.hasMore);
          // 单曲结果返回后立即结束全局阻塞态，实现搜索结果秒开渲染
          isSearching.value = false;
        })
        .catch(() => {});

      const artistsTask = kugouClient
        .searchArtistsPaged(target, 1, 30)
        .then((artistsRes) => {
          if (seq !== activeSearchSeq) return;
          searchResultsArtists.value = artistsRes.items.map((a) => ({
            ...a,
            isFollowed: isArtistFollowed(a.artistId, a.name),
          }));
          searchTotals.artists = Math.max(
            artistsRes.total || 0,
            artistsRes.items.length
          );
          searchHasMore.artists = Boolean(artistsRes.hasMore);
        })
        .catch(() => {});

      const albumsTask = kugouClient
        .searchAlbumsPaged(target, 1, 30)
        .then((albumsRes) => {
          if (seq !== activeSearchSeq) return;
          searchResultsAlbums.value = albumsRes.items;
          searchTotals.albums = Math.max(
            albumsRes.total || 0,
            albumsRes.items.length
          );
          searchHasMore.albums = Boolean(albumsRes.hasMore);
        })
        .catch(() => {});

      const playlistsTask = kugouClient
        .searchPlaylistsPaged(target, 1, 30)
        .then((playlistsRes) => {
          if (seq !== activeSearchSeq) return;
          searchResultsPlaylists.value = playlistsRes.items;
          searchTotals.playlists = Math.max(
            playlistsRes.total || 0,
            playlistsRes.items.length
          );
          searchHasMore.playlists = Boolean(playlistsRes.hasMore);
        })
        .catch(() => {});

      await Promise.allSettled([
        songsTask,
        artistsTask,
        albumsTask,
        playlistsTask,
      ]);
    } catch (e: any) {
      if (seq === activeSearchSeq) {
        notify(`搜索失败：${String(e)}`);
      }
    } finally {
      if (seq === activeSearchSeq) {
        isSearching.value = false;
      }
    }
  }

  async function loadMoreSearchCategory(
    cat: "songs" | "albums" | "artists" | "playlists"
  ) {
    const kw = searchKeyword.value.trim();
    if (
      !kw ||
      isSearching.value ||
      !searchHasMore[cat] ||
      isLoadingMoreSearch[cat]
    ) {
      return;
    }
    const seq = activeSearchSeq;
    const nextPage = searchPages[cat] + 1;
    isLoadingMoreSearch[cat] = true;
    try {
      if (cat === "songs") {
        const res = await kugouClient.searchSongsPaged(kw, nextPage, 30);
        if (seq !== activeSearchSeq) return;
        const seen = new Set(
          searchResultsSongs.value.map(buildSongSearchDedupKey)
        );
        const fresh = res.items.filter((s) => {
          const k = buildSongSearchDedupKey(s);
          if (seen.has(k)) return false;
          seen.add(k);
          return true;
        });
        searchResultsSongs.value = [...searchResultsSongs.value, ...fresh];
        searchPages.songs = nextPage;
        searchTotals.songs = Math.max(
          searchTotals.songs,
          res.total || 0,
          searchResultsSongs.value.length
        );
        searchHasMore.songs = fresh.length > 0 && Boolean(res.hasMore);
      } else if (cat === "albums") {
        const res = await kugouClient.searchAlbumsPaged(kw, nextPage, 30);
        if (seq !== activeSearchSeq) return;
        const seen = new Set(searchResultsAlbums.value.map((a) => a.albumId));
        const fresh = res.items.filter((a) => {
          if (!a.albumId || seen.has(a.albumId)) return false;
          seen.add(a.albumId);
          return true;
        });
        searchResultsAlbums.value = [...searchResultsAlbums.value, ...fresh];
        searchPages.albums = nextPage;
        searchTotals.albums = Math.max(
          searchTotals.albums,
          res.total || 0,
          searchResultsAlbums.value.length
        );
        searchHasMore.albums = fresh.length > 0 && Boolean(res.hasMore);
      } else if (cat === "artists") {
        const res = await kugouClient.searchArtistsPaged(kw, nextPage, 30);
        if (seq !== activeSearchSeq) return;
        const seen = new Set(searchResultsArtists.value.map((a) => a.artistId));
        const fresh = res.items
          .filter((a) => {
            if (!a.artistId || seen.has(a.artistId)) return false;
            seen.add(a.artistId);
            return true;
          })
          .map((a) => ({
            ...a,
            isFollowed: isArtistFollowed(a.artistId, a.name),
          }));
        searchResultsArtists.value = [...searchResultsArtists.value, ...fresh];
        searchPages.artists = nextPage;
        searchTotals.artists = Math.max(
          searchTotals.artists,
          res.total || 0,
          searchResultsArtists.value.length
        );
        searchHasMore.artists = fresh.length > 0 && Boolean(res.hasMore);
      } else if (cat === "playlists") {
        const res = await kugouClient.searchPlaylistsPaged(kw, nextPage, 30);
        if (seq !== activeSearchSeq) return;
        const seen = new Set(
          searchResultsPlaylists.value.map(
            (p) => p.id || p.globalCollectionId
          )
        );
        const fresh = res.items.filter((p) => {
          const k = p.id || p.globalCollectionId;
          if (!k || seen.has(k)) return false;
          seen.add(k);
          return true;
        });
        searchResultsPlaylists.value = [
          ...searchResultsPlaylists.value,
          ...fresh,
        ];
        searchPages.playlists = nextPage;
        searchTotals.playlists = Math.max(
          searchTotals.playlists,
          res.total || 0,
          searchResultsPlaylists.value.length
        );
        searchHasMore.playlists = fresh.length > 0 && Boolean(res.hasMore);
      }
    } catch (e) {
      console.warn(`loadMoreSearchCategory(${cat}) error:`, e);
    } finally {
      if (seq === activeSearchSeq) {
        isLoadingMoreSearch[cat] = false;
      }
    }
  }

  function clearSearchHistory() {
    searchHistory.value = [];
    localStorage.removeItem("bm_search_history");
  }


  return {
    searchKeyword,
    searchViewMode,
    searchSuggestions,
    searchCategoryTab,
    hotSearchKeywords,
    searchDiscovery,
    isLoadingSearchDiscovery,
    hotArtistCategory,
    isChangingHotArtistCat,
    searchHistory,
    searchResultsSongs,
    searchResultsPlaylists,
    searchResultsAlbums,
    searchResultsArtists,
    searchTotals,
    searchPages,
    searchHasMore,
    isLoadingMoreSearch,
    isSearching,
    loadSearchDiscovery,
    switchHotArtistCategory,
    openSearchLanding,
    executeSearch,
    loadMoreSearchCategory,
    clearSearchHistory,
  };
}
