import { computed, ref } from "vue";
import type { AlbumItem } from "../../types";

const ALBUM_COLLECTION_STORAGE_KEY = "bm_collected_albums_v1";

function readPersistedAlbums(): AlbumItem[] {
  try {
    const raw = localStorage.getItem(ALBUM_COLLECTION_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((item): item is AlbumItem => Boolean(item && (item.albumId || item.albumName)))
      : [];
  } catch {
    return [];
  }
}

function persistAlbums(list: AlbumItem[]) {
  try {
    localStorage.setItem(ALBUM_COLLECTION_STORAGE_KEY, JSON.stringify(list));
  } catch {
    // 忽略本地存储写入异常
  }
}

function buildAlbumIdentity(
  album: Pick<AlbumItem, "albumId" | "albumName" | "artistName">
): string {
  const id = String(album.albumId || "").trim();
  if (id && id !== "0") {
    return `id:${id}`;
  }
  const name = (album.albumName || "").trim().toLowerCase();
  const artist = (album.artistName || "").trim().toLowerCase();
  return `meta:${name}::${artist}`;
}

const collectedAlbums = ref<AlbumItem[]>(readPersistedAlbums());

/**
 * 独立专辑收藏响应式状态模块（本地 localStorage 持久化，后续可平滑对接云端收藏接口）
 */
export function useAlbumCollection() {
  function isAlbumCollected(album: Partial<AlbumItem> | null | undefined): boolean {
    if (!album) return false;
    const key = buildAlbumIdentity({
      albumId: album.albumId || "",
      albumName: album.albumName || "",
      artistName: album.artistName || "",
    });
    if (key === "meta::") return false;
    return collectedAlbums.value.some((item) => buildAlbumIdentity(item) === key);
  }

  function toggleCollectAlbum(album: Partial<AlbumItem> | null | undefined): boolean {
    if (!album || (!album.albumId && !album.albumName)) {
      return false;
    }
    const normalized: AlbumItem = {
      albumId: String(album.albumId || ""),
      albumName: album.albumName || "未知专辑",
      artistName: album.artistName || "未知歌手",
      artistId: album.artistId || "",
      coverUrl: album.coverUrl || "",
      publishDate: album.publishDate || "",
      publishCompany: album.publishCompany || "",
      intro: album.intro || "",
      songCount: album.songCount || 0,
    };
    const targetKey = buildAlbumIdentity(normalized);
    const exists = collectedAlbums.value.some(
      (item) => buildAlbumIdentity(item) === targetKey
    );
    if (exists) {
      collectedAlbums.value = collectedAlbums.value.filter(
        (item) => buildAlbumIdentity(item) !== targetKey
      );
      persistAlbums(collectedAlbums.value);
      return false;
    }
    collectedAlbums.value = [normalized, ...collectedAlbums.value];
    persistAlbums(collectedAlbums.value);
    return true;
  }

  const collectedAlbumCount = computed(() => collectedAlbums.value.length);

  return {
    collectedAlbums,
    collectedAlbumCount,
    isAlbumCollected,
    toggleCollectAlbum,
  };
}
