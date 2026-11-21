import type { LyricData, Playlist, Song, SongComment } from "../types";

export function isBrowserDevPreviewEnv(): boolean {
  return (
    Boolean(import.meta.env.DEV) &&
    typeof window !== "undefined" &&
    !("__TAURI_INTERNALS__" in window)
  );
}

/** 生成 1080×1080 本地高清 SVG 封面 Data URI（无外部 CORS 污染，兼容 OKLab 取色与 4K 海报直出） */
function makeShowcaseCoverSvgDataUri(
  c1: string,
  c2: string,
  c3: string,
  titleLabel: string
): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1080" height="1080" viewBox="0 0 1080 1080">
  <defs>
    <radialGradient id="g1" cx="28%" cy="24%" r="68%">
      <stop offset="0%" stop-color="${c1}" />
      <stop offset="55%" stop-color="${c2}" />
      <stop offset="100%" stop-color="#0b1020" />
    </radialGradient>
    <radialGradient id="g2" cx="76%" cy="74%" r="54%">
      <stop offset="0%" stop-color="${c3}" stop-opacity="0.88" />
      <stop offset="100%" stop-color="${c3}" stop-opacity="0" />
    </radialGradient>
  </defs>
  <rect width="1080" height="1080" fill="url(#g1)" />
  <rect width="1080" height="1080" fill="url(#g2)" />
  <circle cx="540" cy="540" r="310" fill="none" stroke="rgba(255,255,255,0.22)" stroke-width="4" />
  <circle cx="540" cy="540" r="205" fill="rgba(255,255,255,0.08)" />
  <circle cx="540" cy="540" r="18" fill="${c3}" />
  <text x="540" y="960" text-anchor="middle" fill="rgba(255,255,255,0.88)" font-family="Segoe UI, PingFang SC, sans-serif" font-size="38" font-weight="800" letter-spacing="8">${titleLabel}</text>
</svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

export function createBrowserDevShowcase(): {
  songs: Song[];
  lyric: LyricData;
  comments: SongComment[];
  floorRepliesMap: Record<string, SongComment[]>;
  playlists: Playlist[];
} {
  const coverA = makeShowcaseCoverSvgDataUri(
    "#f43f5e",
    "#3b2a6e",
    "#38bdf8",
    "ONCE UPON A TIME"
  );
  const coverB = makeShowcaseCoverSvgDataUri(
    "#10b981",
    "#1e293b",
    "#fbbf24",
    "STARLIGHT ACOUSTIC"
  );

  const songs: Song[] = [
    {
      id: "demo_song_1",
      hash: "DEMO_HASH_ONCE_UPON_A_TIME_001",
      hash128: "DEMO_128_001",
      hash320: "DEMO_320_001",
      hashFlac: "DEMO_FLAC_001",
      hashHiRes: "DEMO_HIRES_001",
      size128: 4120000,
      size320: 9850000,
      sizeFlac: 28400000,
      sizeHiRes: 49600000,
      title: "Once upon a Time",
      artist: "Max Oazo / Moonessa",
      artistId: "demo_artist_1",
      authors: [
        { id: "demo_artist_1", name: "Max Oazo" },
        { id: "demo_artist_2", name: "Moonessa" },
      ],
      album: "Once Upon a Time (Concept Hi-Res)",
      albumId: "demo_album_1",
      coverUrl: coverA,
      rawCoverTemplate: coverA,
      hdCoverCandidates: [coverA],
      durationMs: 206000,
      durationSeconds: 206,
      albumAudioId: 1001,
      audioId: 1001,
      mixSongId: "1001",
      isVip: false,
      isLossless: true,
      isHiRes: true,
      hasHiRes: true,
      hasSq: true,
      isSq: true,
      hasFlac: true,
      has320: true,
      has128: true,
      availableQualities: ["high", "flac", "320", "128"],
      climaxStartMs: 52000,
      climaxEndMs: 88000,
      highestQuality: "Hi-Res 24bit/96kHz",
      publishDate: "2024-06-18",
      publishCompany: "Concept Acoustic Studio",
      language: "英语 / Bilingual",
      genre: "Electronic Chill",
      lyricist: "Max Oazo",
      composer: "Max Oazo",
      arranger: "Concept Mastering",
    },
    {
      id: "demo_song_2",
      hash: "DEMO_HASH_STARLIGHT_002",
      hash128: "DEMO_128_002",
      hash320: "DEMO_320_002",
      hashFlac: "DEMO_FLAC_002",
      hashHiRes: "",
      size128: 3820000,
      size320: 8950000,
      sizeFlac: 25600000,
      sizeHiRes: 0,
      title: "夜空中最亮的星 (Hi-Res Live)",
      artist: "逃跑计划",
      artistId: "demo_artist_3",
      album: "世界",
      albumId: "demo_album_2",
      coverUrl: coverB,
      rawCoverTemplate: coverB,
      hdCoverCandidates: [coverB],
      durationMs: 252000,
      durationSeconds: 252,
      albumAudioId: 1002,
      audioId: 1002,
      mixSongId: "1002",
      isVip: false,
      isLossless: true,
      isHiRes: false,
      hasSq: true,
      isSq: true,
      hasFlac: true,
      has320: true,
      has128: true,
      availableQualities: ["flac", "320", "128"],
      highestQuality: "SQ 无损 FLAC",
      publishDate: "2011-04-12",
      publishCompany: "美丽世界",
      language: "国语",
      genre: "Pop Rock",
    },
  ];

  const lyric: LyricData = {
    isKrc: true,
    hasTranslation: true,
    hasPhonetic: true,
    rawText: "",
    lines: [
      {
        startTimeMs: 0,
        durationMs: 4200,
        text: "I think of you now and then",
        translation: "我还是会时不时想起你",
        phonetic: "ai think of you now and then",
        words: [
          { startTimeMs: 0, durationMs: 450, text: "I " },
          { startTimeMs: 450, durationMs: 650, text: "think " },
          { startTimeMs: 1100, durationMs: 450, text: "of " },
          { startTimeMs: 1550, durationMs: 650, text: "you " },
          { startTimeMs: 2200, durationMs: 650, text: "now " },
          { startTimeMs: 2850, durationMs: 550, text: "and " },
          { startTimeMs: 3400, durationMs: 800, text: "then" },
        ],
      },
      {
        startTimeMs: 4200,
        durationMs: 4800,
        text: "Won't let the memories fade to dust",
        translation: "我不会让过往记忆褪成尘埃",
        phonetic: "wont let the me-mo-ries fade to dust",
        words: [
          { startTimeMs: 0, durationMs: 600, text: "Won't " },
          { startTimeMs: 600, durationMs: 550, text: "let " },
          { startTimeMs: 1150, durationMs: 550, text: "the " },
          { startTimeMs: 1700, durationMs: 1100, text: "memories " },
          { startTimeMs: 2800, durationMs: 750, text: "fade " },
          { startTimeMs: 3550, durationMs: 450, text: "to " },
          { startTimeMs: 4000, durationMs: 800, text: "dust" },
        ],
      },
      {
        startTimeMs: 9000,
        durationMs: 5000,
        text: "Baby remember when we were riding through the night",
        translation: "亲爱的还记得我们一同穿过星夜吗",
        phonetic: "ba-by re-mem-ber when we were ri-ding through the night",
        words: [
          { startTimeMs: 0, durationMs: 750, text: "Baby " },
          { startTimeMs: 750, durationMs: 950, text: "remember " },
          { startTimeMs: 1700, durationMs: 600, text: "when " },
          { startTimeMs: 2300, durationMs: 450, text: "we " },
          { startTimeMs: 2750, durationMs: 550, text: "were " },
          { startTimeMs: 3300, durationMs: 650, text: "riding " },
          { startTimeMs: 3950, durationMs: 500, text: "through " },
          { startTimeMs: 4450, durationMs: 550, text: "the night" },
        ],
      },
      {
        startTimeMs: 14000,
        durationMs: 4800,
        text: "The moon was on our side",
        translation: "月光也曾温柔地站成我们的依靠",
        phonetic: "the moon was on our side",
        words: [
          { startTimeMs: 0, durationMs: 550, text: "The " },
          { startTimeMs: 550, durationMs: 900, text: "moon " },
          { startTimeMs: 1450, durationMs: 650, text: "was " },
          { startTimeMs: 2100, durationMs: 650, text: "on " },
          { startTimeMs: 2750, durationMs: 750, text: "our " },
          { startTimeMs: 3500, durationMs: 1300, text: "side" },
        ],
      },
    ],
  };

  const rootCmt1Id = "demo_cmt_1";
  const sub1Id = "demo_sub_1_1";
  const sub1ChildId = "demo_sub_1_1_1";
  const sub2Id = "demo_sub_1_2";
  const sub3Id = "demo_sub_1_3";
  const sub4Id = "demo_sub_1_4";

  const floor1Replies: SongComment[] = [
    {
      id: sub1Id,
      userId: "u_sub_1",
      userName: "麻麻赖赖的心心仔",
      userAvatar: "",
      content: "前奏响起来的一瞬间整个人都被夜风包裹住了，编曲空间感太绝。",
      createdTime: "2026-11-18 18:53",
      timeText: "2026-11-18 18:53",
      likeCount: 26,
      isLiked: false,
      vipLevel: 6,
      location: "广西",
      replyCount: 1,
      specialChildId: "sc_1",
      replies: [],
      replyToUser: "美式全糖糖",
      parentId: rootCmt1Id,
      replyTargetCommentId: rootCmt1Id,
    },
    {
      id: sub1ChildId,
      userId: "u_sub_2",
      userName: "其实我说的不算",
      userAvatar: "",
      content: "确实！副歌吉他扫弦和低频下潜一出来，戴耳机听直接起鸡皮疙瘩。",
      createdTime: "2026-11-18 19:14",
      timeText: "2026-11-18 19:14",
      likeCount: 14,
      isLiked: false,
      vipLevel: 4,
      location: "福建",
      replyCount: 0,
      specialChildId: "sc_1",
      replies: [],
      replyToUser: "麻麻赖赖的心心仔",
      parentId: sub1Id,
      replyTargetCommentId: sub1Id,
      replyQuotedContent: "前奏响起来的一瞬间整个人都被夜风包裹住了，编曲空间感太绝。",
    },
    {
      id: sub2Id,
      userId: "u_sub_3",
      userName: "麋鹿不见山",
      userAvatar: "",
      content: "只需要这句旋律一落下来，就足够把深夜里的疲惫抚平了。",
      createdTime: "2026-11-18 20:24",
      timeText: "2026-11-18 20:24",
      likeCount: 18,
      isLiked: false,
      vipLevel: 6,
      location: "浙江",
      replyCount: 0,
      specialChildId: "sc_1",
      replies: [],
      replyToUser: "美式全糖糖",
      parentId: rootCmt1Id,
      replyTargetCommentId: rootCmt1Id,
    },
    {
      id: sub3Id,
      userId: "u_sub_4",
      userName: "飘过的云是颗粒",
      userAvatar: "",
      content: "开启 Hi-Res 母带直出后连尾音气声都清晰得像在耳边。",
      createdTime: "2026-11-18 20:38",
      timeText: "2026-11-18 20:38",
      likeCount: 9,
      isLiked: false,
      vipLevel: 5,
      location: "广东",
      replyCount: 0,
      specialChildId: "sc_1",
      replies: [],
      replyToUser: "美式全糖糖",
      parentId: rootCmt1Id,
      replyTargetCommentId: rootCmt1Id,
    },
    {
      id: sub4Id,
      userId: "u_root_1",
      userName: "美式全糖糖",
      userAvatar: "",
      content: "大家喜欢这一首太好了，祝今晚好梦！",
      createdTime: "2026-11-18 21:05",
      timeText: "2026-11-18 21:05",
      likeCount: 5,
      isLiked: false,
      vipLevel: 6,
      location: "新疆",
      replyCount: 0,
      specialChildId: "sc_1",
      replies: [],
      replyToUser: "飘过的云是颗粒",
      parentId: sub3Id,
      replyTargetCommentId: sub3Id,
    },
  ];

  const comments: SongComment[] = [
    {
      id: rootCmt1Id,
      userId: "u_root_1",
      userName: "美式全糖糖",
      userAvatar: "",
      content: "爱与不爱其实早就藏在前奏的每一次呼吸感里了，听完好久都没缓过来。",
      createdTime: "2026-11-18 18:03",
      timeText: "2026-11-18 18:03",
      likeCount: 134,
      isLiked: false,
      isHot: true,
      vipLevel: 6,
      isCmtTalent: true,
      location: "新疆",
      replyCount: floor1Replies.length,
      specialChildId: "sc_1",
      replies: floor1Replies,
    },
    {
      id: "demo_cmt_2",
      userId: "u_root_2",
      userName: "晚风拾光",
      userAvatar: "",
      content: "每次夜晚一个人开车回家必播这首，仪表的微光配合歌词真的太有故事感。",
      createdTime: "2026-11-18 21:40",
      timeText: "2026-11-18 21:40",
      likeCount: 72,
      isLiked: false,
      isHot: false,
      vipLevel: 5,
      location: "上海",
      replyCount: 0,
      specialChildId: "",
      replies: [],
    },
  ];

  const playlists: Playlist[] = [
    {
      id: "demo_pl_liked",
      title: "我喜欢",
      name: "我喜欢",
      coverUrl: coverA,
      trackCount: 18,
      description: "心动珍藏曲库",
      creatorName: "我",
      isCreatedByMe: true,
      isFavorite: true,
      isLiked: true,
      listId: 10001,
      globalCollectionId: "demo_gid_liked",
    },
    {
      id: "demo_pl_1",
      title: "深夜微风与纯氧",
      name: "深夜微风与纯氧",
      coverUrl: coverB,
      trackCount: 24,
      description: "深夜听歌专属歌单",
      creatorName: "我",
      isCreatedByMe: true,
      isFavorite: false,
      isLiked: false,
      listId: 10002,
      globalCollectionId: "demo_gid_1",
    },
    {
      id: "demo_pl_2",
      title: "Hi-Res 母带试音珍藏",
      name: "Hi-Res 母带试音珍藏",
      coverUrl: coverA,
      trackCount: 15,
      description: "高解析声场评测曲目",
      creatorName: "我",
      isCreatedByMe: true,
      isFavorite: false,
      isLiked: false,
      listId: 10003,
      globalCollectionId: "demo_gid_2",
    },
  ];

  return {
    songs,
    lyric,
    comments,
    floorRepliesMap: {
      [rootCmt1Id]: floor1Replies,
    },
    playlists,
  };
}
