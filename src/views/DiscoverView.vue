<template>
  <div
    class="discover-page"
    :class="{ 'is-hero-expanded': isDailyExpandedMode || isFmExpandedMode }"
  >
    <transition
      :name="discoverSubTransition"
      mode="out-in"
      @before-enter="notifyStageSubpageBeforeEnter"
    >
      <!-- ==================== 顶部沉浸双雄舞台（固定高度 168px 不变，CSS Grid 1fr/0fr 丝滑横向展开，点击卡片即可展开/返回） ==================== -->
      <div
        v-if="
          store.discoverSubView === 'home' ||
          store.discoverSubView === 'daily' ||
          store.discoverSubView === 'fm_detail'
        "
        key="discover-home-stage"
        class="discover-hero-flow-container"
        :class="{ 'is-hero-expanded': isDailyExpandedMode || isFmExpandedMode }"
      >
      <!-- 日推 & 猜你喜欢 横向可展开双卡舞台（展开时绝不增大高度） -->
      <div
        class="bento-hero-stage"
        :class="{
          'mode-daily-expanded': isDailyExpandedMode,
          'mode-fm-expanded': isFmExpandedMode,
        }"
      >
        <!-- 左卡：每日推荐 (晴空深海蓝，点击展开或收起) -->
        <div
          class="hero-immersive-card daily-immersive-card"
          :class="{
            'is-expanded-banner': isDailyExpandedMode,
            'is-collapsed-left': isFmExpandedMode,
          }"
          @click="handleDailyCardClick"
        >
          <div class="hero-card-inner">
            <div class="hero-card-main-left">
              <!-- 三封面交错立体叠层 -->
              <div
                v-tilt="{ maxDeg: 4 }"
                class="staggered-three-covers"
                :class="{ enlarged: isDailyExpandedMode }"
              >
                <div class="stagger-cover-layer layer-back">
                  <img
                    v-if="dailyCoverTriple[2]"
                    :src="dailyCoverTriple[2]"
                    draggable="false"
                    alt="daily-back"
                  />
                </div>
                <div class="stagger-cover-layer layer-mid">
                  <img
                    v-if="dailyCoverTriple[1]"
                    :src="dailyCoverTriple[1]"
                    draggable="false"
                    alt="daily-mid"
                  />
                </div>
                <div class="stagger-cover-layer layer-front">
                  <img
                    v-if="dailyCoverTriple[0]"
                    :src="dailyCoverTriple[0]"
                    draggable="false"
                    alt="daily-front"
                  />
                  <div class="stagger-date-badge">
                    <span class="stagger-month">{{ todayMonthEng }}</span>
                    <span class="stagger-day">{{ todayDayNum }}</span>
                  </div>
                </div>
              </div>

              <!-- 精简大气的日推信息区 -->
              <div class="hero-card-copy">
                <div class="hero-eyebrow-line">
                  <span>DAILY · {{ todayDateLabel }}</span>
                </div>

                <div class="hero-title-row">
                  <h2 class="hero-main-title">每日推荐</h2>
                  <span v-if="!isDailyExpandedMode" class="hero-count-pill">
                    {{ store.recommendSongs.length || 30 }} 首
                  </span>
                </div>

                <div class="hero-sub-stack">
                  <div
                    class="hero-sub-layer preview-layer"
                    :class="{ 'layer-hidden': isDailyExpandedMode }"
                  >
                    <div
                      v-for="s in store.recommendSongs.slice(0, 1)"
                      :key="s.hash"
                      class="hero-preview-line"
                    >
                      <span class="preview-song">{{ s.title }}</span>
                      <span class="preview-artist">— {{ s.artist }}</span>
                    </div>
                  </div>

                  <div
                    class="hero-sub-layer stats-layer"
                    :class="{ 'layer-active': isDailyExpandedMode }"
                  >
                    <span class="h-stat">每日清晨定制</span>
                    <template v-if="dailyHiResCount > 0">
                      <span class="h-dot">·</span>
                      <span class="h-stat highlight">{{ dailyHiResCount }} 首无损</span>
                    </template>
                  </div>
                </div>
              </div>
            </div>

            <!-- 展开态平滑浮现的精选日推曲目画廊 -->
            <div
              class="hero-expanded-gallery"
              :class="{ 'is-visible': isDailyExpandedMode }"
              @click.stop
            >
              <div class="hero-expanded-gallery-track">
                <div
                  v-for="song in store.recommendSongs.slice(0, 3)"
                  :key="song.hash"
                  class="lib-gallery-track-pill"
                  :class="{ playing: store.currentSong?.hash === song.hash }"
                  @click.stop="store.playSongFromList(song, store.recommendSongs, '每日推荐')"
                >
                  <img :src="song.coverUrl" class="lgtp-cover" draggable="false" alt="" />
                  <div class="lgtp-info">
                    <div class="lgtp-title">{{ song.title }}</div>
                    <div class="lgtp-artist">{{ song.artist }}</div>
                  </div>
                </div>
              </div>
            </div>

            <!-- 右侧控制区（已移除日推刷新与卡片内定位按钮） -->
            <div class="hero-card-actions" @click.stop>
              <button
                class="hero-stereo-play-fab"
                data-tip="立即播放今日全部推荐"
                @click.stop="playAllDaily"
              >
                <PlumpIcon name="play" :size="20" color="currentColor" />
              </button>
            </div>
          </div>
        </div>

        <!-- 右卡：猜你喜欢 · 私人漫游 (翡翠松石绿 · 黑胶唱片机视觉 + 队列隔离联动) -->
        <div
          class="hero-immersive-card fm-immersive-card"
          :class="{
            'is-expanded-banner': isFmExpandedMode,
            'is-collapsed-right': isDailyExpandedMode,
          }"
          @click="handleFmCardClick"
        >
          <div class="hero-card-inner">
            <div class="hero-card-main-left">
              <!-- 方形唱片封套 + 右侧半露旋转黑胶视觉组件 -->
              <div
                class="fm-vinyl-turntable fm-sleeve-vinyl-stage"
                :class="{
                  'is-playing': isFmPlaying,
                  'is-swapping': isFmVinylSwapping,
                  enlarged: isFmExpandedMode,
                }"
              >
                <div class="fm-protruding-vinyl-slide" aria-hidden="true">
                  <div class="fm-protruding-vinyl">
                    <div class="fm-vinyl-grooves"></div>
                    <img
                      v-if="fmDisplayCover"
                      :src="fmDisplayCover"
                      class="fm-vinyl-label-img"
                      draggable="false"
                      alt=""
                    />
                    <div class="fm-vinyl-spindle"></div>
                  </div>
                </div>

                <transition name="fm-sleeve-swap" mode="out-in">
                  <div
                    :key="fmPreviewSong?.hash || fmDisplayCover || 'fm-sleeve-empty'"
                    class="fm-sleeve-cover-box"
                  >
                    <img
                      v-if="fmDisplayCover"
                      :src="fmDisplayCover"
                      class="fm-sleeve-cover-img"
                      draggable="false"
                      alt="fm-vinyl"
                    />
                    <div v-else class="fm-sleeve-cover-fallback">
                      <PlumpIcon name="radio" :size="26" color="#fff" />
                    </div>
                    <div class="fm-sleeve-gloss" aria-hidden="true"></div>
                  </div>
                </transition>
              </div>

              <!-- 猜你喜欢文案区 -->
              <div class="hero-card-copy">
                <div class="hero-eyebrow-line fm-eyebrow">
                  <span>PERSONAL FM</span>
                </div>

                <div class="hero-title-row">
                  <h2 class="hero-main-title">猜你喜欢</h2>
                  <span v-if="isFmPlaying" class="hero-count-pill fm-pill">
                    漫游中
                  </span>
                </div>

                <div class="hero-sub-stack">
                  <div
                    class="hero-sub-layer preview-layer"
                    :class="{ 'layer-hidden': isFmExpandedMode }"
                  >
                    <div v-if="fmPreviewSong" class="hero-preview-line">
                      <span class="preview-song">{{ fmPreviewSong.title }}</span>
                      <span class="preview-artist">— {{ fmPreviewSong.artist }}</span>
                    </div>
                    <div v-else class="hero-preview-line secondary">
                      <span class="preview-artist">专属声学漫游电台</span>
                    </div>
                  </div>

                  <div
                    class="hero-sub-layer stats-layer"
                    :class="{ 'layer-active': isFmExpandedMode }"
                  >
                    <span class="h-stat">专属声学漫游电台</span>
                  </div>
                </div>
              </div>
            </div>

            <!-- 展开态平滑浮现的漫游曲目画廊 -->
            <div
              class="hero-expanded-gallery"
              :class="{ 'is-visible': isFmExpandedMode }"
              @click.stop
            >
              <div class="hero-expanded-gallery-track">
                <div
                  v-for="song in store.personalFmSongs.slice(0, 3)"
                  :key="song.hash"
                  class="lib-gallery-track-pill"
                  :class="{ playing: store.currentSong?.hash === song.hash }"
                  @click.stop="store.playSongFromList(song, store.personalFmSongs, '猜你喜欢')"
                >
                  <img :src="song.coverUrl" class="lgtp-cover" draggable="false" alt="" />
                  <div class="lgtp-info">
                    <div class="lgtp-title">{{ song.title }}</div>
                    <div class="lgtp-artist">{{ song.artist }}</div>
                  </div>
                </div>
              </div>
            </div>

            <!-- 右侧横向一体化播控簇 -->
            <div class="hero-card-actions" @click.stop>
              <div
                class="hero-expanded-extra-btns"
                :class="{ 'is-visible': isFmExpandedMode }"
              >
                <div class="hero-expanded-extra-track">
                  <button
                    class="hero-circle-icon-btn"
                    :class="{ 'is-busy': store.isAppendingFmQueue }"
                    :disabled="store.isAppendingFmQueue || store.isLoadingFm"
                    data-tip="追加到播放队列"
                    @click.stop="handleFmAppendToQueue"
                  >
                    <PlumpIcon name="plus" :size="17" color="currentColor" />
                  </button>
                  <button
                    class="hero-circle-icon-btn"
                    :disabled="store.isLoadingFm"
                    data-tip="换一批漫游推荐"
                    @click.stop="handleFmRefreshBatch"
                  >
                    <PlumpIcon
                      name="refresh"
                      :size="17"
                      color="currentColor"
                      :class="{ 'is-spinning': store.isLoadingFm }"
                    />
                  </button>
                  <button
                    v-if="fmPreviewSong"
                    class="hero-circle-icon-btn"
                    :class="{ liked: store.isSongLiked(fmPreviewSong.hash) }"
                    data-tip="喜欢当前漫游歌曲"
                    @click.stop="store.toggleFavoriteSong(fmPreviewSong)"
                  >
                    <PlumpIcon
                      :name="store.isSongLiked(fmPreviewSong.hash) ? 'heart-filled' : 'heart'"
                      :size="17"
                      :color="store.isSongLiked(fmPreviewSong.hash) ? '#ef4444' : 'currentColor'"
                    />
                  </button>
                  <button
                    class="hero-circle-icon-btn"
                    data-tip="跳至下一首漫游"
                    @click.stop="store.nextPersonalFmTrack(true)"
                  >
                    <PlumpIcon name="next" :size="17" color="currentColor" />
                  </button>
                </div>
              </div>

              <button
                class="hero-stereo-play-fab fm-fab"
                :data-tip="isFmPlaying ? '暂停漫游' : '立即播放猜你喜欢'"
                @click.stop="toggleFmPlay"
              >
                <PlumpIcon :name="isFmPlaying ? 'pause' : 'play'" :size="20" color="currentColor" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- ==================== 点击日推或猜你喜欢后：自然浮现的独立无框双行歌曲列表 ==================== -->
      <transition name="hero-songlist-enter" mode="out-in">
        <div
          v-if="store.discoverSubView === 'daily' || store.discoverSubView === 'fm_detail'"
          :key="store.discoverSubView"
          class="hero-expanded-song-stage"
          :class="store.discoverSubView === 'fm_detail' ? 'theme-fm' : 'theme-daily'"
        >
          <div class="frameless-sublist-toolbar">
            <div class="frameless-sublist-info">
              <span class="sublist-badge">
                {{ store.discoverSubView === "daily" ? "DAILY RECOMMEND" : "PERSONAL ROAMING" }}
              </span>
              <span class="sublist-desc">
                {{
                  heroSongFilter.trim()
                    ? `${filteredHeroSongs.length} / ${activeHeroSongs.length} 首`
                    : `${activeHeroSongs.length} 首精选`
                }}
              </span>
            </div>
            <div style="display: flex; align-items: center; gap: 10px">
              <div class="frameless-sublist-search">
                <PlumpIcon name="search" :size="15" />
                <input
                  v-model="heroSongFilter"
                  type="text"
                  placeholder="在当前列表中筛选歌名 / 歌手 / 专辑..."
                />
                <button v-if="heroSongFilter" class="clear-inline-btn" data-tip="清空筛选" @click="heroSongFilter = ''">
                  <PlumpIcon name="close" :size="12" />
                </button>
              </div>
            </div>
          </div>

          <div ref="heroSongGridRef" class="frameless-songs-grid-2col">
            <div
              v-for="(song, index) in filteredHeroSongs"
              :key="song.hash + '_' + index"
              :data-song-hash="song.hash"
              :data-locate-song-key="song.hash || song.id"
              class="unified-song-row"
              :class="[
                { active: store.currentSong?.hash === song.hash },
                heroStagger.getStaggerClass(index),
              ]"
              :style="heroStagger.getStaggerStyle(index)"
              @click="store.playSongFromList(song, activeHeroSongs, activeHeroQueueTitle)"
              @contextmenu.prevent="store.openSongContextMenu($event, song, activeHeroSongs)"
            >
              <div class="usr-index-slot">
                <span
                  v-if="store.currentSong?.hash === song.hash && store.isPlaying"
                  class="mini-eq-bars"
                >
                  <i></i><i></i><i></i>
                </span>
                <span v-else class="usr-index-num">
                  {{ String(index + 1).padStart(2, "0") }}
                </span>
              </div>

              <div class="usr-cover-wrap">
                <img v-if="song.coverUrl" :src="song.coverUrl" loading="lazy" draggable="false" alt="" />
              </div>

              <div class="usr-meta-stack">
                <div class="usr-line-1">
                  <span class="usr-title">{{ song.title }}</span>
                </div>
                <div class="usr-line-2">
                  <span class="usr-badges">
                    <span
                      v-if="resolveSongQualityBadge(song)"
                      class="usr-badge-quality"
                      :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                    >
                      {{ resolveSongQualityBadge(song)!.label }}
                    </span>
                    <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                  </span>
                  <span class="usr-subtext">
                    <span
                      class="usr-artist is-clickable"
                      data-tip="查看歌手主页"
                      @click.stop="store.navigateToSongArtist(song)"
                    >
                      {{ song.artist }}
                    </span>
                    <template v-if="song.album">
                      <span class="usr-sep"> · </span>
                      <span
                        class="usr-album is-clickable"
                        data-tip="查看专辑详情"
                        @click.stop="store.navigateToSongAlbum(song)"
                      >
                        {{ song.album }}
                      </span>
                    </template>
                  </span>
                </div>
              </div>

              <div class="usr-right-zone">
                <div class="usr-hover-actions" @click.stop>
                  <button
                    type="button"
                    class="usr-action-btn"
                    :class="{ liked: store.isSongLiked(song.hash) }"
                    :data-tip="store.isSongLiked(song.hash) ? '取消喜欢' : '收藏到我喜欢'"
                    @click="store.toggleFavoriteSong(song)"
                  >
                    <PlumpIcon
                      :name="store.isSongLiked(song.hash) ? 'heart-filled' : 'heart'"
                      :size="15"
                    />
                  </button>
                  <button
                    type="button"
                    class="usr-action-btn"
                    data-tip="添加到歌单"
                    @click="store.addToPlaylistModalSong = song"
                  >
                    <PlumpIcon name="plus" :size="15" />
                  </button>
                  <button
                    type="button"
                    class="usr-action-btn"
                    data-tip="更多操作"
                    @click="store.openSongContextMenu($event, song, activeHeroSongs)"
                  >
                    <PlumpIcon name="more" :size="15" />
                  </button>
                </div>
                <span class="usr-duration">{{ formatDuration(song) }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== 发现页下半部分主体内容（作为 v-else 分支，离开发现首页去二级页时绝不提前消失） ==================== -->
        <div
          v-else
          key="discover-home-lower"
          class="discover-home-lower"
          :class="{
            'lower-blur-exit': store.discoverHeroExpanding !== 'none',
          }"
        >
        <!-- 1. 新歌速递（支持页内左右翻页突破 12 首限制 + 紧凑行日期小标） -->
        <section class="discover-block">
          <div class="block-header newsong-header">
            <div class="block-title-with-tabs">
              <h3 class="block-title">新歌速递</h3>

              <div class="app-sliding-tabs newsong-sliding-tabs">
                <button
                  v-for="area in newSongAreas"
                  :key="area.id"
                  class="app-sliding-tab-btn"
                  :class="{ active: store.newSongPeriod === area.id }"
                  @click="store.loadNewSongExpress(area.id)"
                >
                  <span class="tab-label">{{ area.label }}</span>
                </button>
              </div>
            </div>

            <div class="block-right-frameless-actions">
              <div v-if="homeNewSongTotalPages > 1" class="ns-home-pager">
                <button
                  type="button"
                  class="ns-pager-btn"
                  :disabled="safeHomeNewSongPage <= 0"
                  data-tip="上一页新歌"
                  @click="prevHomeNewSongPage"
                >
                  <PlumpIcon name="chevron-right" :size="13" class="icon-flip-x" />
                </button>
                <span class="ns-pager-indicator">
                  {{ safeHomeNewSongPage + 1 }} / {{ homeNewSongTotalPages }}
                </span>
                <button
                  type="button"
                  class="ns-pager-btn"
                  :disabled="safeHomeNewSongPage >= homeNewSongTotalPages - 1"
                  data-tip="下一页新歌"
                  @click="nextHomeNewSongPage"
                >
                  <PlumpIcon name="chevron-right" :size="13" />
                </button>
              </div>

              <button
                class="frameless-icon-square-btn discover-refresh-icon-btn"
                :disabled="store.isLoadingNewSongs"
                data-tip="刷新全部新歌分类"
                @click="store.refreshAllNewSongTags(true)"
              >
                <PlumpIcon
                  name="refresh"
                  :size="18"
                  :class="{ 'is-spinning': store.isLoadingNewSongs }"
                />
              </button>
              <button
                class="frameless-icon-square-btn"
                data-tip="播放当前分类全部新歌"
                @click="playAllNewSongs"
              >
                <PlumpIcon name="play" :size="18" />
              </button>
              <button
                type="button"
                class="discover-view-all-pill"
                @click="openNewSongsSubpage"
              >
                <span>查看全部</span>
                <PlumpIcon name="chevron-right" :size="13" />
              </button>
            </div>
          </div>

          <div ref="newSongGridShellRef" class="newsong-grid-shell">
          <transition name="tag-content-morph" mode="out-in">
            <div
              :key="`${store.newSongPeriod}_${safeHomeNewSongPage}_${homeNewSongCols}`"
              class="newsong-frameless-grid-3x4"
              :class="{ 'is-ns-compact': isNewSongCompactMode }"
              :style="{ '--ns-cols': homeNewSongCols, '--ns-rows': homeNewSongRows }"
            >
              <div
                v-for="(song, idx) in pagedHomeNewSongs"
                :key="song.hash + '_' + (safeHomeNewSongPage * homeNewSongPageSize + idx)"
                class="newsong-frameless-item bm-stagger-item"
                :style="{ '--bm-stagger-idx': Math.min(idx, 14) }"
                :class="{ active: store.currentSong?.hash === song.hash }"
                @click="store.playSongFromList(song, sortedNewSongsAll)"
                @contextmenu.prevent="store.openSongContextMenu($event, song, sortedNewSongsAll)"
              >
                <span class="ns-col-rank">
                  <template v-if="store.currentSong?.hash === song.hash">
                    <span class="mini-eq-bars" :class="{ paused: !store.isPlaying }"><i></i><i></i><i></i></span>
                  </template>
                  <template v-else>
                    {{ String(safeHomeNewSongPage * homeNewSongPageSize + idx + 1).padStart(2, "0") }}
                  </template>
                </span>

                <div class="ns-cover-box">
                  <img :src="song.coverUrl" class="ns-cover-img" loading="lazy" alt="" />
                  <div class="ns-cover-overlay">
                    <PlumpIcon
                      :name="store.currentSong?.hash === song.hash && store.isPlaying ? 'pause' : 'play'"
                      :size="15"
                      color="#fff"
                    />
                  </div>
                </div>

                <div class="ns-meta-col">
                  <div class="ns-song-title-row">
                    <span class="ns-song-title">{{ song.title }}</span>
                    <span
                      v-if="resolveSongQualityBadge(song)"
                      class="fs-badge"
                      :class="resolveSongQualityBadge(song)!.tier"
                    >
                      {{ resolveSongQualityBadge(song)!.label }}
                    </span>
                    <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                  </div>
                  <div class="ns-song-sub-row">
                    <button
                      class="inline-meta-btn ns-song-artist"
                      data-tip="查看歌手主页"
                      @click.stop="store.navigateToSongArtist(song)"
                    >
                      {{ song.artist }}
                    </button>
                  </div>
                </div>

                <span
                  v-if="song.publishDate"
                  class="ns-song-date-pill"
                  :class="{ 'is-today': formatRelativePublishDate(song.publishDate) === '今日' }"
                >
                  {{ formatRelativePublishDate(song.publishDate) }}
                </span>

                <button
                  class="ns-like-btn"
                  :class="{ liked: store.isSongLiked(song.hash) }"
                  :data-tip="store.isSongLiked(song.hash) ? '取消喜欢' : '收藏到我喜欢'"
                  @click.stop="store.toggleFavoriteSong(song)"
                >
                  <PlumpIcon
                    :name="store.isSongLiked(song.hash) ? 'heart-filled' : 'heart'"
                    :size="15"
                  />
                </button>
              </div>
            </div>
          </transition>
          </div>
        </section>

        <!-- 2. 巅峰排行榜（重构概念版多彩渐变榜单卡 + 顶部分类快选 + 规范化前三名预览） -->
        <section class="discover-block">
          <div class="block-header rank-home-header">
            <div class="block-title-with-tabs">
              <h3 class="block-title">巅峰排行榜</h3>

              <div class="app-sliding-tabs rank-sliding-tabs" role="tablist">
                <button
                  v-for="cat in rankHomeCategoryTabs"
                  :key="cat.id"
                  type="button"
                  role="tab"
                  class="app-sliding-tab-btn"
                  :class="{ active: homeRankCategory === cat.id }"
                  :aria-selected="homeRankCategory === cat.id"
                  @click="homeRankCategory = cat.id"
                >
                  <span class="tab-label">{{ cat.label }}</span>
                </button>
              </div>
            </div>
            <div class="block-right-frameless-actions">
              <button
                class="frameless-icon-square-btn discover-refresh-icon-btn"
                :disabled="store.isLoadingRanks"
                data-tip="刷新榜单"
                @click="store.loadRankList(true)"
              >
                <PlumpIcon
                  name="refresh"
                  :size="18"
                  :class="{ 'is-spinning': store.isLoadingRanks }"
                />
              </button>
              <button
                class="frameless-icon-square-btn"
                :data-tip="`查看全部榜单 (${store.rankList.length || 57})`"
                @click="openAllRanksOverview"
              >
                <PlumpIcon name="grid" :size="18" />
              </button>
            </div>
          </div>

          <!-- 巅峰榜单概念版卡片矩阵（2行 × 4列 = 8张卡片） -->
          <transition name="tag-content-morph" mode="out-in">
            <div :key="homeRankCategory" class="rank-double-row-grid">
              <div
                v-for="(rk, rIdx) in homeDoubleRowRanks"
                :key="rk.rankId"
                v-tilt="{ maxDeg: 3 }"
                class="rank-concept-card bm-stagger-item"
                :style="{ '--bm-stagger-idx': Math.min(rIdx, 14) }"
                :class="getRankThemeClass(rk, rIdx)"
                @click="store.openRankDetail(rk)"
              >
                <div class="rcc-top-header">
                  <div class="rcc-cover-wrap">
                    <img
                      v-if="rk.coverUrl"
                      :src="rk.coverUrl"
                      class="rcc-cover-img"
                      loading="lazy"
                      draggable="false"
                      alt=""
                    />
                    <div v-else class="rcc-cover-fallback">
                      <PlumpIcon name="trophy" :size="24" color="#fff" />
                    </div>
                  </div>

                  <div class="rcc-head-meta">
                    <h4 class="rcc-title">{{ rk.name || rk.rankName }}</h4>
                    <div class="rcc-sub-line">
                      <span class="rcc-cat-badge">{{ rk.category || "官方热榜" }}</span>
                      <span class="rcc-sub-text">
                        {{ rk.updateFrequency || "每天更新" }}
                      </span>
                    </div>
                  </div>

                  <button
                    type="button"
                    class="rcc-play-circle-btn"
                    data-tip="直接播放该榜单"
                    @click.stop="store.playRankDirectly(rk)"
                  >
                    <PlumpIcon name="play" :size="14" color="currentColor" />
                  </button>
                </div>

                <div class="rcc-divider"></div>

                <div class="rcc-tracks-list">
                  <div
                    v-for="(item, lIdx) in getRankPreviewStructured(rk)"
                    :key="lIdx"
                    class="rcc-track-row"
                  >
                    <span class="rcc-rank-num" :class="'top-' + (lIdx + 1)">
                      {{ lIdx + 1 }}
                    </span>
                    <div class="rcc-track-main">
                      <span class="rcc-song-name">{{ item.title }}</span>
                      <span v-if="item.artist" class="rcc-song-artist"> - {{ item.artist }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </transition>
        </section>

        <!-- 3. 精选场景电台 -->
        <section class="discover-block">
          <div class="block-header">
            <div class="block-title-with-tabs">
              <h3 class="block-title">精选场景电台</h3>

              <div class="app-sliding-tabs rank-sliding-tabs" role="tablist">
                <button
                  v-for="cat in homeFmCategoryTabs"
                  :key="cat.id"
                  type="button"
                  role="tab"
                  class="app-sliding-tab-btn"
                  :class="{ active: homeFmCategory === cat.id }"
                  :aria-selected="homeFmCategory === cat.id"
                  @click="switchHomeFmCategory(cat.id)"
                >
                  <span class="tab-label">{{ cat.label }}</span>
                </button>
              </div>
            </div>
            <div class="block-right-frameless-actions">
              <button
                type="button"
                class="frameless-icon-square-btn discover-refresh-icon-btn"
                data-tip="换一批场景电台"
                @click="rotateHomeFmPage"
              >
                <PlumpIcon name="refresh" :size="18" :class="{ 'is-spinning': isRotatingHomeFm }" />
              </button>
              <button
                type="button"
                class="frameless-icon-square-btn"
                :data-tip="`进入电台广场 (${store.fmChannels.length || 325})`"
                @click="openRadioPlaza"
              >
                <PlumpIcon name="radio" :size="18" />
              </button>
            </div>
          </div>

          <transition name="tag-content-morph" mode="out-in">
            <div :key="`${homeFmCategory}_${homeFmPage}`" class="radio-single-row-corridor">
              <div
                v-for="(ch, idx) in singleRowFmChannels"
                :key="ch.id"
                class="radio-cover-card bm-stagger-item"
                :style="{ '--bm-stagger-idx': Math.min(idx, 14) }"
                :class="{
                  active: isChannelActivelyPlaying(ch.id) || (radioModalOpen && radioModalChannel?.id === ch.id),
                  playing: isChannelActivelyPlaying(ch.id),
                }"
                @click="openRadioPreviewModal(ch)"
              >
                <div class="rc-cover-stage">
                  <img
                    v-if="getStationCover(ch, idx)"
                    :src="getStationCover(ch, idx)"
                    class="rc-cover-img"
                    loading="lazy"
                    draggable="false"
                    alt=""
                  />
                  <div v-else class="rc-cover-fallback">
                    <PlumpIcon name="radio" :size="28" color="#fff" />
                  </div>
                  <div class="rc-cover-shade"></div>
                  <div class="rc-top-badges">
                    <span class="rc-freq-tag">{{ ch.freqLabel }}</span>
                    <span class="rc-view-hint-pill">查看队列</span>
                  </div>
                  <span class="rc-listeners-tag">{{ ch.listenerCountText }}</span>
                  <button
                    type="button"
                    class="rc-play-btn"
                    :data-tip="isChannelActivelyPlaying(ch.id) ? '暂停当前电台' : '立即调频播放'"
                    @click.stop="toggleOrPlayFmChannel(ch)"
                  >
                    <PlumpIcon
                      :name="isChannelActivelyPlaying(ch.id) ? 'pause' : 'play'"
                      :size="15"
                      color="#fff"
                    />
                  </button>
                </div>
                <div class="rc-meta">
                  <div class="rc-name" :title="ch.name">{{ ch.name }}</div>
                  <div class="rc-desc" :title="ch.description">{{ ch.description }}</div>
                </div>
              </div>
            </div>
          </transition>
        </section>
      </div>
      </transition>
    </div>

    <!-- ==================== 二级页面 A：新歌速递 · 极简时间轴折叠视图 ==================== -->
    <div
      v-else-if="store.discoverSubView === 'new_songs'"
      key="discover-sub-newsongs"
      class="discover-subpage frameless-subpage ns-standalone-page"
    >
      <div class="subpage-frameless-header ns-clean-header">
        <div class="sfh-left">
          <div class="ns-header-title-group">
            <h2 class="sfh-title">新歌速递</h2>
            <span class="ns-header-date-pill">{{ newSongDateRangeLabel }}</span>
          </div>

          <div class="app-sliding-tabs">
            <button
              v-for="area in newSongAreas"
              :key="area.id"
              type="button"
              class="app-sliding-tab-btn"
              :class="{ active: store.newSongPeriod === area.id }"
              @click="store.loadNewSongExpress(area.id)"
            >
              <span class="tab-label">{{ area.label }}</span>
            </button>
          </div>
        </div>

        <div class="sfh-right">
          <button
            v-if="groupedNewSongsByDate.length > 1"
            type="button"
            class="frameless-icon-square-btn"
            :data-tip="isAllDateGroupsCollapsed ? '展开全部日期分组' : '收起全部日期分组'"
            @click="toggleAllDateGroups"
          >
            <PlumpIcon :name="isAllDateGroupsCollapsed ? 'expand-all' : 'collapse-all'" :size="18" />
          </button>
          <button
            type="button"
            class="frameless-icon-square-btn discover-refresh-icon-btn"
            :disabled="store.isLoadingNewSongs"
            data-tip="刷新全部新歌分类"
            @click="store.refreshAllNewSongTags(true)"
          >
            <PlumpIcon
              name="refresh"
              :size="18"
              :class="{ 'is-spinning': store.isLoadingNewSongs }"
            />
          </button>
          <button
            type="button"
            class="frameless-icon-square-btn primary-tint"
            :disabled="sortedNewSongsAll.length === 0"
            :data-tip="`播放全部 (${sortedNewSongsAll.length} 首)`"
            @click="playAllNewSongs"
          >
            <PlumpIcon name="play" :size="18" />
          </button>
        </div>
      </div>

      <transition name="tag-content-morph" mode="out-in">
        <div
          v-if="store.isLoadingNewSongs && groupedNewSongsByDate.length === 0"
          key="ns-loading"
          class="ns-empty-hint"
        >
          <PlumpIcon name="refresh" :size="20" class="is-spinning" />
          <span>正在同步最近一周发行的歌曲...</span>
        </div>

        <div
          v-else-if="groupedNewSongsByDate.length === 0"
          key="ns-empty"
          class="ns-empty-hint"
        >
          <span>暂无最近一周发行的曲目，可点击右上角刷新重试</span>
        </div>

        <div v-else :key="store.newSongPeriod" ref="newSongTimelineRef" class="ns-frameless-timeline">
          <section
            v-for="(group, gIdx) in groupedNewSongsByDate"
            :key="group.dateKey"
            :data-ns-date-group="group.dateKey"
            class="ns-frameless-date-group"
            :class="{ 'is-collapsed': isDateGroupCollapsed(group.dateKey) }"
          >
            <div
              class="ns-flow-date-header"
              role="button"
              tabindex="0"
              :aria-expanded="!isDateGroupCollapsed(group.dateKey)"
              @click="toggleDateGroup(group.dateKey)"
              @keydown.enter.prevent="toggleDateGroup(group.dateKey)"
              @keydown.space.prevent="toggleDateGroup(group.dateKey)"
            >
              <div class="ns-flow-date-left">
                <span class="ns-flow-accent-bar"></span>
                <h3 class="ns-flow-date-title">{{ group.displayLabel }}</h3>
                <span class="ns-flow-date-tag">{{ group.songs.length }} 首</span>
              </div>

              <div class="ns-flow-date-right">
                <button
                  type="button"
                  class="ns-flow-play-pill"
                  data-tip="播放该日全部新歌"
                  @click.stop="playDateGroupSongs(group.songs)"
                >
                  <PlumpIcon name="play" :size="12" color="currentColor" />
                  <span>播放本日 ({{ group.songs.length }})</span>
                </button>
                <span
                  class="ns-collapse-chevron"
                  :class="{ collapsed: isDateGroupCollapsed(group.dateKey) }"
                >
                  <PlumpIcon name="chevron-down" :size="16" />
                </span>
              </div>
            </div>

            <div
              class="ns-date-collapse-body"
              :class="{ collapsed: isDateGroupCollapsed(group.dateKey) }"
              :inert="isDateGroupCollapsed(group.dateKey) ? true : undefined"
            >
              <div class="ns-date-collapse-inner">
                <div class="frameless-songs-grid-2col">
                  <div
                    v-for="(song, sIdx) in group.songs"
                    :key="song.hash + '_' + sIdx"
                    :data-locate-song-key="song.hash || song.id"
                    class="unified-song-row"
                    :class="[
                      { active: store.currentSong?.hash === song.hash },
                      gIdx === 0 ? newSongStagger.getStaggerClass(sIdx) : '',
                    ]"
                    :style="gIdx === 0 ? newSongStagger.getStaggerStyle(sIdx) : undefined"
                    @click="store.playSongFromList(song, sortedNewSongsAll, '新歌速递')"
                    @contextmenu.prevent="store.openSongContextMenu($event, song, sortedNewSongsAll)"
                  >
                    <div class="usr-index-slot">
                      <span
                        v-if="store.currentSong?.hash === song.hash && store.isPlaying"
                        class="mini-eq-bars"
                      >
                        <i></i><i></i><i></i>
                      </span>
                      <span v-else class="usr-index-num">{{ String(sIdx + 1).padStart(2, "0") }}</span>
                    </div>

                    <div class="usr-cover-wrap">
                      <img v-if="song.coverUrl" :src="song.coverUrl" loading="lazy" draggable="false" alt="" />
                    </div>

                    <div class="usr-meta-stack">
                      <div class="usr-line-1">
                        <span class="usr-title">{{ song.title }}</span>
                      </div>
                      <div class="usr-line-2">
                        <span class="usr-badges">
                          <span
                            v-if="resolveSongQualityBadge(song)"
                            class="usr-badge-quality"
                            :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                          >
                            {{ resolveSongQualityBadge(song)!.label }}
                          </span>
                          <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                        </span>
                        <span class="usr-subtext">
                          <span
                            class="usr-artist is-clickable"
                            data-tip="查看歌手主页"
                            @click.stop="store.navigateToSongArtist(song)"
                          >
                            {{ song.artist }}
                          </span>
                          <template v-if="song.album">
                            <span class="usr-sep"> · </span>
                            <span
                              class="usr-album is-clickable"
                              data-tip="查看专辑详情"
                              @click.stop="store.navigateToSongAlbum(song)"
                            >
                              {{ song.album }}
                            </span>
                          </template>
                          <template v-if="formatNewSongShortDate(song.publishDate || group.dateKey)">
                            <span class="usr-sep"> · </span>
                            <span class="usr-date">{{ formatNewSongShortDate(song.publishDate || group.dateKey) }}</span>
                          </template>
                        </span>
                      </div>
                    </div>

                    <div class="usr-right-zone">
                      <div class="usr-hover-actions" @click.stop>
                        <button
                          type="button"
                          class="usr-action-btn"
                          :class="{ liked: store.isSongLiked(song.hash) }"
                          :data-tip="store.isSongLiked(song.hash) ? '取消喜欢' : '收藏到我喜欢'"
                          @click="store.toggleFavoriteSong(song)"
                        >
                          <PlumpIcon
                            :name="store.isSongLiked(song.hash) ? 'heart-filled' : 'heart'"
                            :size="15"
                          />
                        </button>
                        <button
                          type="button"
                          class="usr-action-btn"
                          data-tip="查看评论"
                          @click="store.openCommentsModal(song)"
                        >
                          <PlumpIcon name="comment" :size="14" />
                        </button>
                        <button
                          type="button"
                          class="usr-action-btn"
                          data-tip="添加到歌单"
                          @click="store.addToPlaylistModalSong = song"
                        >
                          <PlumpIcon name="plus" :size="15" />
                        </button>
                      </div>
                      <span class="usr-duration">{{ formatDuration(song) }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </section>
        </div>
      </transition>
    </div>

    <!-- ==================== 二级页面 B：巅峰排行榜（左定右流去嵌套工作台 · 4格精准分类 + 双列统一歌曲行） ==================== -->
    <div
      v-else-if="store.discoverSubView === 'rank'"
      key="discover-sub-rank"
      class="discover-subpage frameless-subpage rank-standalone-page"
    >
      <div class="rank-split-workspace">
        <!-- 左侧无框吸顶控制栏：4等分分类标签 + 紧凑活跃榜单卡 + 榜单快切列表 -->
        <aside class="rank-left-sidebar">
          <div class="rank-cat-pills" role="tablist">
            <button
              v-for="cat in rankQuickSwitchCategories"
              :key="cat.key"
              type="button"
              role="tab"
              class="rank-cat-pill"
              :class="{ active: rankPaneCategory === cat.key }"
              :aria-selected="rankPaneCategory === cat.key"
              @click="rankPaneCategory = cat.key"
            >
              {{ cat.label }}
            </button>
          </div>

          <div class="rank-active-hero-compact" :class="getRankThemeClass(store.activeRank)">
            <div class="rahc-top">
              <div class="rahc-cover-box">
                <img
                  v-if="activeRankCover"
                  :src="activeRankCover"
                  class="rahc-cover-img"
                  draggable="false"
                  alt=""
                />
                <div v-else class="rahc-cover-fallback">
                  <PlumpIcon name="trophy" :size="26" color="#fff" />
                </div>
              </div>
              <div class="rahc-meta">
                <h2 class="rahc-title" :title="store.activeRank?.name || store.activeRank?.rankName">
                  {{ store.activeRank?.name || store.activeRank?.rankName || "巅峰排行榜" }}
                </h2>
                <div class="rahc-sub">
                  <span>{{ store.activeRank?.updateFrequency || "每天更新" }}</span>
                  <span>·</span>
                  <span>{{ store.activeRankSongs.length || store.activeRank?.totalSongs || 0 }} 首</span>
                  <template v-if="activeRankHiResCount > 0">
                    <span>·</span>
                    <span>{{ activeRankHiResCount }} 无损</span>
                  </template>
                </div>
              </div>
            </div>

            <div class="rahc-actions">
              <button
                type="button"
                class="rahc-play-btn"
                :disabled="filteredRankSongsWithIndex.length === 0"
                @click="playAllActiveRank"
              >
                <PlumpIcon name="play" :size="14" color="currentColor" />
                <span>播放全部</span>
              </button>
              <button
                type="button"
                class="rahc-icon-btn"
                :disabled="store.isLoadingRankSongs || store.isLoadingMoreRankSongs"
                data-tip="刷新当前榜单"
                @click="store.refreshActiveRank()"
              >
                <PlumpIcon
                  name="refresh"
                  :size="14"
                  :class="{ 'is-spinning': store.isLoadingRankSongs || store.isLoadingMoreRankSongs }"
                />
              </button>
            </div>
          </div>

          <div class="rank-switcher-list">
            <div
              v-for="rk in filteredRankQuickSwitchList"
              :key="rk.rankId"
              class="rank-switch-row"
              :class="{ active: store.activeRank?.rankId === rk.rankId }"
              @click="selectRankFromLeftPane(rk)"
            >
              <img
                v-if="rk.coverUrl"
                :src="rk.coverUrl"
                class="rsr-cover"
                loading="lazy"
                draggable="false"
                alt=""
              />
              <div v-else class="rsr-cover-fallback">
                <PlumpIcon name="trophy" :size="14" color="#fff" />
              </div>
              <div class="rsr-meta">
                <div class="rsr-name">{{ rk.name || rk.rankName }}</div>
                <div class="rsr-freq">{{ rk.updateFrequency || rk.category || "官方榜单" }}</div>
              </div>
              <button
                type="button"
                class="rsr-play-mini"
                data-tip="直接播放该榜单"
                @click.stop="store.playRankDirectly(rk)"
              >
                <PlumpIcon name="play" :size="12" />
              </button>
            </div>
          </div>
        </aside>

        <!-- 右侧歌曲流：去冗余计数/表头，统一 2 列双行歌曲列表 -->
        <div class="rank-right-stream">
          <div
            v-if="rankRangeSegments.length > 1 || store.isLoadingMoreRankSongs"
            class="rank-right-topbar"
          >
            <div class="rank-right-topbar-left">
              <div
                v-if="rankRangeSegments.length > 1"
                class="rank-range-pills"
                role="tablist"
              >
                <button
                  v-for="seg in rankRangeSegments"
                  :key="seg.key"
                  type="button"
                  role="tab"
                  class="rank-range-pill"
                  :class="{ active: activeRankRangeKey === seg.key }"
                  :aria-selected="activeRankRangeKey === seg.key"
                  @click="activeRankRangeKey = seg.key"
                >
                  {{ seg.label }}
                </button>
              </div>

              <span v-if="store.isLoadingMoreRankSongs" class="stream-sync-badge">
                <PlumpIcon name="refresh" :size="12" class="is-spinning" />
                <span>同步全量榜单中</span>
              </span>
            </div>
          </div>

          <transition name="tag-content-morph" mode="out-in">
            <div
              v-if="store.isLoadingRankSongs && store.activeRankSongs.length === 0"
              key="rank-loading"
              class="ns-empty-hint"
            >
              <PlumpIcon name="refresh" :size="20" class="is-spinning" />
              <span>正在同步「{{ store.activeRank?.name || "巅峰榜单" }}」实时权威曲库...</span>
            </div>

            <div
              v-else-if="filteredRankSongsWithIndex.length === 0"
              key="rank-empty"
              class="ns-empty-hint"
            >
              <span>当前榜单暂无曲目，可点击刷新重试</span>
            </div>

            <div
              v-else
              :key="`${store.activeRank?.rankId || 'rank'}_${activeRankRangeKey}`"
              class="rank-fullwidth-tracks"
            >
              <div ref="rankSongGridRef" class="frameless-songs-grid-2col">
                <div
                  v-for="(item, rIdx) in filteredRankSongsWithIndex"
                  :key="item.song.hash + '_' + item.rankNo"
                  :data-rank-song-hash="item.song.hash"
                  :data-locate-song-key="item.song.hash || item.song.id"
                  class="unified-song-row"
                  :class="[
                    { active: store.currentSong?.hash === item.song.hash },
                    rankStagger.getStaggerClass(rIdx),
                  ]"
                  :style="rankStagger.getStaggerStyle(rIdx)"
                  @click="store.playSongFromList(item.song, store.activeRankSongs, store.activeRank?.name || '巅峰榜单')"
                  @contextmenu.prevent="store.openSongContextMenu($event, item.song, store.activeRankSongs)"
                >
                  <div class="usr-index-slot">
                    <span
                      v-if="store.currentSong?.hash === item.song.hash && store.isPlaying"
                      class="mini-eq-bars"
                    >
                      <i></i><i></i><i></i>
                    </span>
                    <span
                      v-else
                      class="usr-index-num"
                      :class="item.rankNo <= 3 ? `rank-top-${item.rankNo}` : ''"
                    >
                      {{ String(item.rankNo).padStart(2, "0") }}
                    </span>
                  </div>

                  <div class="usr-cover-wrap">
                    <img
                      v-if="item.song.coverUrl"
                      :src="item.song.coverUrl"
                      loading="lazy"
                      draggable="false"
                      alt=""
                    />
                  </div>

                  <div class="usr-meta-stack">
                    <div class="usr-line-1">
                      <span class="usr-title">{{ item.song.title }}</span>
                    </div>
                    <div class="usr-line-2">
                      <span class="usr-badges">
                        <span
                          v-if="resolveSongQualityBadge(item.song)"
                          class="usr-badge-quality"
                          :class="`tier-${resolveSongQualityBadge(item.song)!.tier}`"
                        >
                          {{ resolveSongQualityBadge(item.song)!.label }}
                        </span>
                        <span v-if="item.song.isVip" class="usr-badge-vip">VIP</span>
                      </span>
                      <span class="usr-subtext">
                        <span
                          class="usr-artist is-clickable"
                          data-tip="查看歌手主页"
                          @click.stop="store.navigateToSongArtist(item.song)"
                        >
                          {{ item.song.artist }}
                        </span>
                        <template v-if="item.song.album">
                          <span class="usr-sep"> · </span>
                          <span
                            class="usr-album is-clickable"
                            data-tip="查看专辑详情"
                            @click.stop="store.navigateToSongAlbum(item.song)"
                          >
                            {{ item.song.album }}
                          </span>
                        </template>
                      </span>
                    </div>
                  </div>

                  <div class="usr-right-zone">
                    <div class="usr-hover-actions" @click.stop>
                      <button
                        type="button"
                        class="usr-action-btn"
                        :class="{ liked: store.isSongLiked(item.song.hash) }"
                        :data-tip="store.isSongLiked(item.song.hash) ? '取消喜欢' : '收藏到我喜欢'"
                        @click="store.toggleFavoriteSong(item.song)"
                      >
                        <PlumpIcon
                          :name="store.isSongLiked(item.song.hash) ? 'heart-filled' : 'heart'"
                          :size="15"
                        />
                      </button>
                      <button
                        type="button"
                        class="usr-action-btn"
                        data-tip="查看评论"
                        @click="store.openCommentsModal(item.song)"
                      >
                        <PlumpIcon name="comment" :size="14" />
                      </button>
                      <button
                        type="button"
                        class="usr-action-btn"
                        data-tip="添加到歌单"
                        @click="store.addToPlaylistModalSong = item.song"
                      >
                        <PlumpIcon name="plus" :size="15" />
                      </button>
                      <button
                        type="button"
                        class="usr-action-btn"
                        data-tip="更多操作"
                        @click="store.openSongContextMenu($event, item.song, store.activeRankSongs)"
                      >
                        <PlumpIcon name="more" :size="14" />
                      </button>
                    </div>
                    <span class="usr-duration">{{ formatDuration(item.song) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </transition>
        </div>
      </div>
    </div>

    <!-- ==================== 二级页面 C：电台广场（情景分类胶囊 + 电台画廊 + 右侧 380px 即时调频试听舱） ==================== -->
    <div
      v-else-if="store.discoverSubView === 'radio'"
      key="discover-sub-radio"
      class="discover-subpage frameless-subpage radio-plaza-page"
    >
      <div class="radio-plaza-redesign">
        <div class="radio-plaza-workspace">
          <!-- 左侧自适应电台画廊 -->
          <div class="radio-station-gallery-col">
            <div class="radio-plaza-col-head">
              <h2 class="radio-plaza-heading">电台广场</h2>
              <span class="radio-plaza-total-hint">
                {{ filteredPlazaChannels.length }} / {{ store.fmChannels.length }} 台
              </span>
            </div>

            <div class="radio-plaza-cats" role="tablist">
              <button
                v-for="cat in radioPlazaCategoriesWithCount"
                :key="cat.id"
                type="button"
                role="tab"
                class="rpc-cat-pill"
                :class="{ active: store.selectedFmCategory === cat.id }"
                :aria-selected="store.selectedFmCategory === cat.id"
                @click="selectRadioPlazaCategory(cat.id)"
              >
                <span>{{ cat.label }}</span>
                <span class="rpc-cat-count">{{ cat.count }}</span>
              </button>
            </div>

            <div v-if="primaryPlazaChannels.length === 0" class="ns-empty-hint">
              <span>当前分类暂无电台频道，可切换其它场景频道</span>
            </div>

            <template v-else>
              <div class="radio-station-gallery">
                <div
                  v-for="(ch, idx) in primaryPlazaChannels"
                  :key="ch.id"
                  class="radio-channel-card"
                  :class="{
                    active: store.activeFmChannel?.id === ch.id,
                    playing: isChannelActivelyPlaying(ch.id),
                  }"
                  @click="selectAndPlayPlazaChannel(ch, false)"
                >
                  <div class="rplc-cover-wrap">
                    <img
                      v-if="getStationCover(ch, idx)"
                      :src="getStationCover(ch, idx)"
                      class="rplc-cover-img"
                      loading="lazy"
                      draggable="false"
                      alt=""
                    />
                    <div v-else class="rplc-cover-fallback">
                      <PlumpIcon name="radio" :size="26" color="#fff" />
                    </div>
                    <span class="rplc-fm-badge">{{ ch.freqLabel || "FM" }}</span>
                    <span
                      v-if="store.activeFmChannel?.id === ch.id || isChannelActivelyPlaying(ch.id)"
                      class="rplc-active-ring"
                      aria-hidden="true"
                    ></span>
                    <button
                      type="button"
                      class="rplc-play-orb"
                      :data-tip="isChannelActivelyPlaying(ch.id) ? '暂停播放' : '立即调频播放'"
                      @click.stop="toggleOrPlayFmChannel(ch)"
                    >
                      <PlumpIcon
                        :name="isChannelActivelyPlaying(ch.id) ? 'pause' : 'play'"
                        :size="15"
                        color="currentColor"
                      />
                    </button>
                  </div>

                  <div class="rplc-meta">
                    <div class="rplc-name" :title="ch.name">{{ ch.name }}</div>
                    <div class="rplc-desc" :title="ch.description">
                      {{ ch.description || ch.listenerCountText || ch.category }}
                    </div>
                    <div class="rplc-sub-meta">
                      <span class="rplc-cat-tag">{{ ch.category || "调频" }}</span>
                      <span v-if="ch.listenerCountText" class="rplc-listeners">{{ ch.listenerCountText }}</span>
                    </div>
                  </div>
                </div>
              </div>

              <div
                v-if="extraPlazaChannels.length > 0"
                class="plaza-extra-collapse"
                :class="{ open: isPlazaExpandedAll }"
                :inert="!isPlazaExpandedAll ? true : undefined"
              >
                <div class="plaza-extra-collapse-inner">
                  <div class="radio-station-gallery plaza-extra-gallery">
                    <div
                      v-for="(ch, eIdx) in extraPlazaChannels"
                      :key="ch.id"
                      class="radio-channel-card"
                      :class="{
                        active: store.activeFmChannel?.id === ch.id,
                        playing: isChannelActivelyPlaying(ch.id),
                      }"
                      @click="selectAndPlayPlazaChannel(ch, false)"
                    >
                      <div class="rplc-cover-wrap">
                        <img
                          v-if="getStationCover(ch, primaryPlazaChannels.length + eIdx)"
                          :src="getStationCover(ch, primaryPlazaChannels.length + eIdx)"
                          class="rplc-cover-img"
                          loading="lazy"
                          draggable="false"
                          alt=""
                        />
                        <div v-else class="rplc-cover-fallback">
                          <PlumpIcon name="radio" :size="26" color="#fff" />
                        </div>
                        <span class="rplc-fm-badge">{{ ch.freqLabel || "FM" }}</span>
                        <span
                          v-if="store.activeFmChannel?.id === ch.id || isChannelActivelyPlaying(ch.id)"
                          class="rplc-active-ring"
                          aria-hidden="true"
                        ></span>
                        <button
                          type="button"
                          class="rplc-play-orb"
                          :data-tip="isChannelActivelyPlaying(ch.id) ? '暂停播放' : '立即调频播放'"
                          @click.stop="toggleOrPlayFmChannel(ch)"
                        >
                          <PlumpIcon
                            :name="isChannelActivelyPlaying(ch.id) ? 'pause' : 'play'"
                            :size="15"
                            color="currentColor"
                          />
                        </button>
                      </div>

                      <div class="rplc-meta">
                        <div class="rplc-name" :title="ch.name">{{ ch.name }}</div>
                        <div class="rplc-desc" :title="ch.description">
                          {{ ch.description || ch.listenerCountText || ch.category }}
                        </div>
                        <div class="rplc-sub-meta">
                          <span class="rplc-cat-tag">{{ ch.category || "调频" }}</span>
                          <span v-if="ch.listenerCountText" class="rplc-listeners">{{ ch.listenerCountText }}</span>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </template>

            <button
              v-if="extraPlazaChannels.length > 0"
              type="button"
              class="radio-plaza-expand-btn"
              :aria-expanded="isPlazaExpandedAll"
              @click="isPlazaExpandedAll = !isPlazaExpandedAll"
            >
              <span>
                {{
                  isPlazaExpandedAll
                    ? "收起多余频道"
                    : `显示全部 ${filteredPlazaChannels.length} 个频道`
                }}
              </span>
              <PlumpIcon
                name="chevron-down"
                :size="14"
                :class="{ 'icon-rotate-180': isPlazaExpandedAll }"
              />
            </button>
          </div>

          <!-- 右侧 380px 即时调频试听舱 -->
          <aside id="rp-live-stream-anchor" class="radio-live-tuner-dock">
            <div class="rltd-dial-header">
              <div class="rltd-dial-readout">
                <span>{{ store.activeFmChannel?.freqLabel || "FM 98.6" }}</span>
                <span>
                  {{ isChannelActivelyPlaying(store.activeFmChannel?.id || "") ? "ON AIR · 正在调频" : "STANDBY · 待命" }}
                </span>
              </div>
              <div class="rltd-dial-scale">
                <span class="rltd-dial-needle" :style="{ left: `${activeFmDialPercent}%` }"></span>
              </div>
            </div>

            <div class="rltd-station-head">
              <div class="rltd-cover-box">
                <img
                  v-if="activePlazaCover"
                  :src="activePlazaCover"
                  class="rltd-cover-img"
                  draggable="false"
                  alt=""
                />
              </div>
              <div class="rltd-station-info">
                <h3 class="rltd-station-name" :title="store.activeFmChannel?.name || '精选电台'">
                  {{ store.activeFmChannel?.name || "咖啡厅" }}
                </h3>
                <p class="rltd-station-sub">
                  {{ store.activeFmChannel?.category || "精选" }} ·
                  {{ store.activeFmChannel?.listenerCountText || "实时漫游" }} ·
                  {{ store.fmStationSongs.length }} 首
                </p>
              </div>
            </div>

            <div class="rltd-controls">
              <button
                type="button"
                class="rltd-btn-primary"
                :disabled="store.isLoadingFmStation && store.fmStationSongs.length === 0"
                @click="toggleOrPlayActiveStation"
              >
                <PlumpIcon
                  :name="isChannelActivelyPlaying(store.activeFmChannel?.id || '') ? 'pause' : 'play'"
                  :size="14"
                  color="currentColor"
                />
                <span>{{ isChannelActivelyPlaying(store.activeFmChannel?.id || "") ? "暂停" : "播放" }}</span>
              </button>
              <button
                type="button"
                class="rltd-btn-sub"
                :disabled="store.isLoadingFmStation"
                @click="refreshCurrentRadioStation"
              >
                <PlumpIcon
                  name="refresh"
                  :size="13"
                  :class="{ 'is-spinning': store.isLoadingFmStation }"
                />
                <span>刷新</span>
              </button>
              <button
                type="button"
                class="rltd-btn-sub"
                :disabled="store.isLoadingFmStation"
                @click="appendCurrentRadioStation"
              >
                <PlumpIcon name="plus" :size="13" />
                <span>追加</span>
              </button>
            </div>

            <div class="rltd-queue-scroll">
              <div
                v-if="store.isLoadingFmStation && store.fmStationSongs.length === 0"
                class="ns-empty-hint"
              >
                <PlumpIcon name="refresh" :size="18" class="is-spinning" />
                <span>正在调频加载「{{ store.activeFmChannel?.name || "电台" }}」...</span>
              </div>

              <div v-else-if="store.fmStationSongs.length === 0" class="ns-empty-hint">
                <span>暂无曲目，可点击「刷新」获取新曲</span>
              </div>

              <div v-else class="rltd-unified-list">
                <div
                  v-for="(song, idx) in store.fmStationSongs"
                  :key="song.hash + '_' + idx"
                  :data-radio-song-hash="song.hash"
                  :data-locate-song-key="song.hash || song.id"
                  class="unified-song-row"
                  :class="{ active: store.currentSong?.hash === song.hash }"
                  @click="store.playSongFromList(song, store.fmStationSongs, `FM · ${store.activeFmChannel?.name || '电台'}`)"
                  @contextmenu.prevent="store.openSongContextMenu($event, song, store.fmStationSongs)"
                >
                  <div class="usr-index-slot">
                    <span
                      v-if="store.currentSong?.hash === song.hash && store.isPlaying"
                      class="mini-eq-bars"
                    >
                      <i></i><i></i><i></i>
                    </span>
                    <span v-else class="usr-index-num">{{ String(idx + 1).padStart(2, "0") }}</span>
                  </div>

                  <div class="usr-cover-wrap">
                    <img
                      v-if="song.coverUrl"
                      :src="song.coverUrl"
                      loading="lazy"
                      draggable="false"
                      alt=""
                    />
                  </div>

                  <div class="usr-meta-stack">
                    <div class="usr-line-1">
                      <span class="usr-title">{{ song.title }}</span>
                    </div>
                    <div class="usr-line-2">
                      <span class="usr-badges">
                        <span
                          v-if="resolveSongQualityBadge(song)"
                          class="usr-badge-quality"
                          :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                        >
                          {{ resolveSongQualityBadge(song)!.label }}
                        </span>
                        <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                      </span>
                      <span class="usr-subtext">
                        <span
                          class="usr-artist is-clickable"
                          data-tip="查看歌手主页"
                          @click.stop="store.navigateToSongArtist(song)"
                        >
                          {{ song.artist }}
                        </span>
                        <template v-if="song.album">
                          <span class="usr-sep"> · </span>
                          <span
                            class="usr-album is-clickable"
                            data-tip="查看专辑详情"
                            @click.stop="store.navigateToSongAlbum(song)"
                          >
                            {{ song.album }}
                          </span>
                        </template>
                      </span>
                    </div>
                  </div>

                  <div class="usr-right-zone">
                    <div class="usr-hover-actions" @click.stop>
                      <button
                        type="button"
                        class="usr-action-btn"
                        :class="{ liked: store.isSongLiked(song.hash) }"
                        :data-tip="store.isSongLiked(song.hash) ? '取消喜欢' : '收藏到我喜欢'"
                        @click="store.toggleFavoriteSong(song)"
                      >
                        <PlumpIcon
                          :name="store.isSongLiked(song.hash) ? 'heart-filled' : 'heart'"
                          :size="14"
                        />
                      </button>
                      <button
                        type="button"
                        class="usr-action-btn"
                        data-tip="添加到播放队列末尾"
                        @click="store.appendSongToQueue(song)"
                      >
                        <PlumpIcon name="plus" :size="14" />
                      </button>
                    </div>
                    <span class="usr-duration">{{ formatDuration(song) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </aside>
        </div>
      </div>
    </div>
    </transition>

    <!-- 全部榜单分类总览弹窗（Teleport 顶层 + 固定舒适尺寸 + 切换平滑过渡） -->
    <Teleport to="body">
      <transition name="fade-pop">
        <div
          v-if="store.isRankGridModalOpen"
          class="modal-backdrop rank-overview-backdrop"
          :class="[
            `scheme-${store.resolvedColorScheme}`,
            store.resolvedColorScheme === 'watercolor_light'
              ? `light-variant-${store.lightThemeVariant}`
              : `dark-variant-${store.darkThemeVariant}`,
          ]"
          @click.self="store.isRankGridModalOpen = false"
        >
          <div class="rank-overview-dialog glass-popover">
            <div class="rom-header">
              <div class="rom-title-group">
                <h3>全部官方巅峰榜单</h3>
                <span class="rom-total-pill">共 {{ store.rankList.length }} 个权威榜单</span>
              </div>

              <div class="rom-header-right">
                <div class="rom-search-box">
                  <PlumpIcon name="search" :size="14" class="rom-search-icon" />
                  <input
                    v-model="rankModalSearchKeyword"
                    type="text"
                    placeholder="搜索榜单名称或热歌..."
                    class="rom-search-input"
                  />
                  <button
                    v-if="rankModalSearchKeyword"
                    class="rom-search-clear"
                    @click="rankModalSearchKeyword = ''"
                  >
                    <PlumpIcon name="close" :size="11" />
                  </button>
                </div>
                <button class="frameless-hover-btn rom-close-btn" @click="store.isRankGridModalOpen = false" data-tip="关闭">
                  <PlumpIcon name="close" :size="16" />
                </button>
              </div>
            </div>

            <!-- 分类导航栏 -->
            <div class="rom-category-tabs" role="tablist">
              <button
                v-for="cat in rankOverviewCategories"
                :key="cat.key"
                type="button"
                role="tab"
                class="rom-cat-pill"
                :class="{ active: rankModalCategory === cat.key }"
                :aria-selected="rankModalCategory === cat.key"
                @click="rankModalCategory = cat.key"
              >
                <span>{{ cat.label }}</span>
                <span class="rom-cat-count">{{ cat.count }}</span>
              </button>
            </div>

            <!-- 分类榜单分组滚动区（固定尺寸容器内平滑过渡切换，分类标签位置绝不漂移） -->
            <div ref="romScrollBodyRef" class="rom-scroll-body">
              <transition name="rom-tab-fade" mode="out-in">
                <div :key="`${rankModalCategory}::${rankModalSearchKeyword.trim()}`">
                  <div
                    v-if="groupedModalRankSections.length === 0"
                    class="rom-empty-state"
                  >
                    没有找到匹配的榜单
                  </div>

                  <section
                    v-for="group in groupedModalRankSections"
                    :key="group.category"
                    class="rom-category-section"
                  >
                    <div class="rom-sec-header">
                      <div class="rom-sec-title-wrap">
                        <span class="rom-sec-accent-bar"></span>
                        <h4 class="rom-sec-title">{{ group.category }}</h4>
                        <span class="rom-sec-sub">{{ group.subtitle }}</span>
                      </div>
                      <span class="rom-sec-badge">{{ group.ranks.length }} 个榜单</span>
                    </div>

                    <div class="rom-cards-grid">
                      <div
                        v-for="(rk, rIdx) in group.ranks"
                        :key="rk.rankId"
                        class="rank-overview-card"
                        :class="[
                          getRankThemeClass(rk, rIdx),
                          { 'is-active-rank': store.activeRank?.rankId === rk.rankId }
                        ]"
                        @click="store.openRankDetail(rk)"
                      >
                        <div class="roc-cover-wrap">
                          <img
                            v-if="rk.coverUrl"
                            :src="rk.coverUrl"
                            class="roc-cover-img"
                            loading="lazy"
                            draggable="false"
                            alt=""
                          />
                          <div v-else class="roc-cover-fallback">
                            <span>{{ (rk.name || rk.rankName || "榜单").slice(0, 4) }}</span>
                          </div>
                        </div>

                        <div class="roc-info">
                          <div class="roc-title-row">
                            <h5 class="roc-title" :title="rk.name || rk.rankName">{{ rk.name || rk.rankName }}</h5>
                          </div>
                          <div class="roc-meta-row">
                            <span class="roc-freq-tag">{{ rk.updateFrequency || "定期更新" }}</span>
                            <span v-if="rk.totalSongs > 0" class="roc-count-tag">{{ rk.totalSongs }}首</span>
                          </div>
                          <p
                            v-if="getRankFirstPreviewText(rk)"
                            class="roc-top1-preview"
                            :title="getRankFirstPreviewText(rk)"
                          >
                            TOP1 · {{ getRankFirstPreviewText(rk) }}
                          </p>
                        </div>

                        <button
                          class="roc-play-btn"
                          data-tip="直接播放此榜单"
                          @click.stop="store.playRankDirectly(rk)"
                        >
                          <PlumpIcon name="play" :size="13" color="currentColor" />
                        </button>
                      </div>
                    </div>
                  </section>
                </div>
              </transition>
            </div>
          </div>
        </div>
      </transition>
    </Teleport>

    <RadioQueueModal
      :open="radioModalOpen"
      :channel="radioModalChannel"
      :songs="radioModalSongs"
      :loading="radioModalLoading"
      :cover-url="radioModalCover"
      :is-playing-channel="Boolean(radioModalChannel && isChannelActivelyPlaying(radioModalChannel.id))"
      @close="radioModalOpen = false"
      @play="handleRadioModalPlay"
      @refresh="handleRadioModalRefresh"
      @append="handleRadioModalAppend"
      @go-plaza="handleRadioModalGoPlaza"
      @play-song="handleRadioModalPlaySong"
    />

    <FloatingLocateFab
      :songs="discoverLocateSongs"
      :scroll-container-selector="
        store.discoverSubView === 'radio'
          ? '.rltd-queue-scroll'
          : '.skyline-stage-viewport'
      "
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { NEW_SONG_AREAS, useAppStore } from "../stores/appStore";
import { formatDuration, kugouClient, resolveSongQualityBadge } from "../api/kugou";
import { useViewportStagger } from "../composables/useViewportStagger";
import type { FmChannel, RankItem, Song } from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";
import RadioQueueModal from "../components/RadioQueueModal.vue";
import FloatingLocateFab from "../components/FloatingLocateFab.vue";

const store = useAppStore();

const heroSongFilter = ref("");
const heroSongGridRef = ref<HTMLElement | null>(null);
const newSongTimelineRef = ref<HTMLElement | null>(null);
const rankSongGridRef = ref<HTMLElement | null>(null);
const discoverSubTransition = ref("subpage-drill-in");

const heroStagger = useViewportStagger(heroSongGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 300,
});
const newSongStagger = useViewportStagger(newSongTimelineRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 220,
});
const rankStagger = useViewportStagger(rankSongGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 160,
});

function notifyStageSubpageBeforeEnter() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("breeze:subpage-before-enter"));
  }
}

watch(
  () => store.discoverSubView,
  (newVal, oldVal) => {
    const isHomeLike = (v?: string) => !v || v === "home" || v === "daily" || v === "fm_detail";
    if (isHomeLike(newVal) && !isHomeLike(oldVal)) {
      discoverSubTransition.value = "subpage-drill-out";
    } else {
      discoverSubTransition.value = "subpage-drill-in";
    }
  }
);

function openNewSongsSubpage() {
  store.openNewSongsPage();
}

const isDailyExpandedMode = computed(
  () => store.discoverHeroExpanding === "daily" || store.discoverSubView === "daily"
);
const isFmExpandedMode = computed(
  () => store.discoverHeroExpanding === "fm" || store.discoverSubView === "fm_detail"
);

watch(
  () => isDailyExpandedMode.value || isFmExpandedMode.value,
  (expanded) => {
    if (!expanded) return;
    nextTick(() => {
      const viewport = document.querySelector(".skyline-stage-viewport") as HTMLElement | null;
      if (viewport && viewport.scrollTop > 0) {
        viewport.scrollTop = 0;
      }
    });
  }
);

const monthNames = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
const weekNames = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
const currentClockDate = computed(() => new Date(store.nowClockTick));
const todayDayNum = computed(() => String(currentClockDate.value.getDate()).padStart(2, "0"));
const todayMonthEng = computed(() => monthNames[currentClockDate.value.getMonth()]);
const todayDateLabel = computed(
  () =>
    `${currentClockDate.value.getMonth() + 1}月${currentClockDate.value.getDate()}日 · ${weekNames[currentClockDate.value.getDay()]}`
);

const newSongDateRangeLabel = computed(() => {
  const end = currentClockDate.value;
  const start = new Date(end.getTime() - 7 * 86400000);
  const fmt = (d: Date) =>
    `${String(d.getMonth() + 1).padStart(2, "0")}.${String(d.getDate()).padStart(2, "0")}`;
  return `${fmt(start)} - ${fmt(end)}`;
});

const dailyCoverTriple = computed(() => {
  const covers = store.recommendSongs
    .map((s) => s.coverUrl)
    .filter((u) => Boolean(u));
  return [
    covers[0] || store.currentSong?.coverUrl || "",
    covers[1] || covers[0] || "",
    covers[2] || covers[1] || covers[0] || "",
  ];
});

const dailyHiResCount = computed(
  () => store.recommendSongs.filter((s) => s.hasHiRes || s.hasSq || s.isLossless || s.isHiRes).length
);

const isGuessYouLikeQueueActive = computed(() => {
  const title = store.currentQueueTitle || "";
  return title.includes("猜你喜欢") || title.includes("私人漫游");
});

const isFmPlaying = computed(() =>
  Boolean(store.isPlaying && isGuessYouLikeQueueActive.value)
);

const fmPreviewSong = computed<Song | null>(() => {
  if (isGuessYouLikeQueueActive.value && store.currentSong) {
    return store.currentSong;
  }
  return store.currentFmSong;
});

const fmDisplayCover = ref<string>(store.currentFmSong?.coverUrl || "");
const isFmVinylSwapping = ref<boolean>(false);
let fmVinylSwapTimer: ReturnType<typeof setTimeout> | null = null;
let fmVinylSwapCoverTimer: ReturnType<typeof setTimeout> | null = null;

function clearFmVinylTimers() {
  if (fmVinylSwapTimer) {
    clearTimeout(fmVinylSwapTimer);
    fmVinylSwapTimer = null;
  }
  if (fmVinylSwapCoverTimer) {
    clearTimeout(fmVinylSwapCoverTimer);
    fmVinylSwapCoverTimer = null;
  }
}

watch(
  () => store.currentFmSong?.coverUrl || "",
  (cover) => {
    if (!fmDisplayCover.value && cover) {
      fmDisplayCover.value = cover;
    }
  },
  { immediate: true }
);

watch(
  () =>
    [
      isGuessYouLikeQueueActive.value,
      fmPreviewSong.value?.hash || "",
      fmPreviewSong.value?.coverUrl || "",
    ] as const,
  ([_inFmQueue, nextHash, nextCover], oldTuple) => {
    const prevHash = oldTuple?.[1] ?? "";
    if (!nextCover && !nextHash) return;
    if (!fmDisplayCover.value && nextCover) {
      fmDisplayCover.value = nextCover;
      return;
    }
    if (nextHash === prevHash && nextCover === fmDisplayCover.value) return;
    clearFmVinylTimers();
    isFmVinylSwapping.value = true;
    fmVinylSwapCoverTimer = setTimeout(() => {
      if (nextCover) fmDisplayCover.value = nextCover;
    }, 180);
    fmVinylSwapTimer = setTimeout(() => {
      isFmVinylSwapping.value = false;
    }, 480);
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  clearFmVinylTimers();
});

/**
 * 猜你喜欢「+」追加到播放队列：
 * - 严格不刷新当前猜你喜欢页面列表（保持页面原有 24 首不动、不触发换一批按钮旋转）；
 * - 若当前已经在猜你喜欢播放队列中：向服务端单独拉取一批去重新曲直接追加进 playQueue；
 * - 若当前处于其它歌单/队列中：直接将页面展示的 24 首猜你喜欢追加到当前播放队列末尾。
 */
async function handleFmAppendToQueue() {
  if (store.isAppendingFmQueue || store.isLoadingFm) return;
  if (!isGuessYouLikeQueueActive.value) {
    if (store.personalFmSongs.length > 0) {
      store.batchAppendSongsToQueue(store.personalFmSongs);
      return;
    }
    const fetched = await store.fetchMorePersonalFmForQueue();
    if (!fetched || fetched.length === 0) {
      store.notify("暂未获取到新的猜你喜欢曲目，请稍后重试");
      return;
    }
    store.batchAppendSongsToQueue(fetched);
    return;
  }

  const added = await store.fetchMorePersonalFmForQueue();
  if (!added || added.length === 0) {
    store.notify("暂未获取到更多新曲，请稍后重试");
    return;
  }
  store.batchAppendSongsToQueue(added);
}

/**
 * 猜你喜欢「换一批」：显式刷新页面展示的 24 首漫游候选曲
 */
async function handleFmRefreshBatch() {
  if (store.isLoadingFm) return;
  const batch = await store.loadPersonalFm(false, false);
  if (!batch || batch.length === 0) {
    store.notify("暂未获取到新的猜你喜欢曲目，请稍后重试");
    return;
  }
  if (isGuessYouLikeQueueActive.value) {
    store.playSongFromList(batch[0], store.personalFmSongs, "猜你喜欢");
    return;
  }
  store.notify(`已换一批 ${batch.length} 首猜你喜欢推荐`);
}

function handleDailyCardClick() {
  if (isDailyExpandedMode.value) {
    collapseToDiscoverHome();
    return;
  }
  heroSongFilter.value = "";
  store.openDailyRecommendPage();
}

function handleFmCardClick() {
  if (isFmExpandedMode.value) {
    collapseToDiscoverHome();
    return;
  }
  heroSongFilter.value = "";
  store.openGuessYouLikePage();
}

function collapseToDiscoverHome() {
  store.discoverHeroExpanding = "none";
  if (store.navHistoryStack.length > 0) {
    store.navigateBack();
    return;
  }
  store.discoverSubView = "home";
}

function playAllDaily() {
  if (store.recommendSongs.length > 0) {
    store.playSongFromList(store.recommendSongs[0], store.recommendSongs, "每日推荐");
  } else {
    store.loadDailyRecommend();
  }
}

function toggleFmPlay() {
  if (isFmPlaying.value) {
    store.togglePlayPause();
    return;
  }
  if (isGuessYouLikeQueueActive.value && store.currentSong) {
    store.togglePlayPause();
    return;
  }
  if (store.currentFmSong) {
    store.playSongFromList(store.currentFmSong, store.personalFmSongs, "猜你喜欢");
  } else {
    store.nextPersonalFmTrack(true);
  }
}

const activeHeroSongs = computed<Song[]>(() => {
  if (store.discoverSubView === "fm_detail") {
    return store.personalFmSongs;
  }
  return store.recommendSongs;
});

const activeHeroQueueTitle = computed(() =>
  store.discoverSubView === "fm_detail" ? "猜你喜欢" : "每日推荐"
);

const filteredHeroSongs = computed(() => {
  const kw = heroSongFilter.value.trim().toLowerCase();
  if (!kw) return activeHeroSongs.value;
  return activeHeroSongs.value.filter(
    (s) =>
      s.title.toLowerCase().includes(kw) ||
      s.artist.toLowerCase().includes(kw) ||
      (s.album && s.album.toLowerCase().includes(kw))
  );
});

function isCurrentSongInList(list: Song[]) {
  if (!store.currentSong) return false;
  return list.some((s) => s.hash === store.currentSong?.hash);
}

async function scrollToCurrentPlayingSong() {
  if (!store.currentSong) return;
  const targetHash = store.currentSong.hash;
  const findTargetRow = () => {
    const selector = `[data-song-hash="${CSS.escape(targetHash)}"]`;
    return (
      heroSongGridRef.value?.querySelector<HTMLElement>(selector) ??
      document.querySelector<HTMLElement>(`.hero-expanded-song-stage ${selector}`)
    );
  };
  let el = findTargetRow();
  if (!el && heroSongFilter.value.trim()) {
    heroSongFilter.value = "";
    await nextTick();
    el = findTargetRow();
  }
  if (el) {
    el.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}

function scrollToCurrentPlayingRankSong() {
  if (!store.currentSong) return;
  const el = document.querySelector(`[data-rank-song-hash="${store.currentSong.hash}"]`);
  if (el) {
    el.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}

// ==================== 新歌速递（涵盖六大分类：华语、粤语、欧美、日韩、ACG、影视 + 容器自适应列数与动态分页） ====================
const newSongAreas = NEW_SONG_AREAS;
const homeNewSongPage = ref(0);
const collapsedDateKeys = ref<Record<string, boolean>>({});
const newSongGridShellRef = ref<HTMLElement | null>(null);
const newSongShellWidth = ref<number>(
  typeof window !== "undefined" ? Math.max(640, window.innerWidth - 56) : 1184
);

let newSongShellRo: ResizeObserver | null = null;
watch(
  newSongGridShellRef,
  (el) => {
    if (newSongShellRo) {
      newSongShellRo.disconnect();
      newSongShellRo = null;
    }
    if (!el) return;
    const syncWidth = (w: number) => {
      if (w > 120) newSongShellWidth.value = Math.round(w);
    };
    syncWidth(el.getBoundingClientRect().width || el.clientWidth);
    if (typeof ResizeObserver !== "undefined") {
      newSongShellRo = new ResizeObserver((entries) => {
        for (const entry of entries) {
          syncWidth(entry.contentRect.width);
        }
      });
      newSongShellRo.observe(el);
    }
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  if (newSongShellRo) {
    newSongShellRo.disconnect();
    newSongShellRo = null;
  }
});

const homeNewSongCols = computed<number>(() => {
  const w = newSongShellWidth.value;
  if (w >= 1460) return 4;
  if (w >= 1040) return 3;
  if (w >= 680) return 2;
  return 1;
});

const homeNewSongRows = computed<number>(() =>
  homeNewSongCols.value === 1 ? 6 : 3
);

const homeNewSongPageSize = computed<number>(
  () => homeNewSongCols.value * homeNewSongRows.value
);

const isNewSongCompactMode = computed<boolean>(() => {
  const cols = homeNewSongCols.value;
  if (cols <= 1) return false;
  const colW = (newSongShellWidth.value - (cols - 1) * 18) / cols;
  return colW < 350;
});

watch(homeNewSongPageSize, (newSize, oldSize) => {
  if (!oldSize || newSize === oldSize) return;
  const firstIdx = homeNewSongPage.value * oldSize;
  homeNewSongPage.value = Math.floor(firstIdx / newSize);
});

watch(
  () => store.newSongPeriod,
  () => {
    homeNewSongPage.value = 0;
    collapsedDateKeys.value = {};
  }
);

const COLLATOR_LETTERS = "ABCDEFGHJKLMNOPQRSTWXYZ".split("");
const COLLATOR_BOUNDARIES = "阿八嚓哒妸发旮哈讥咔垃妈拿噢啪期然撒塌挖昔压匝".split("");
const KR_CHOSEONG_INITIALS = [
  "G", "K", "N", "D", "T", "R", "M", "B", "P",
  "S", "S", "A", "J", "C", "C", "K", "T", "P", "H",
];
const pinyinCollator =
  typeof Intl !== "undefined"
    ? new Intl.Collator("zh-CN-u-co-pinyin", { numeric: true, sensitivity: "base" })
    : null;

function stripLeadingPunctuation(text: string): string {
  return (text || "")
    .replace(/^[\s《〈「『【〔（(["'‘“·~!@#$%^&*_\-+=:;,.?/\\|<>]+/u, "")
    .trim();
}

function extractFirstLetter(text: string): string {
  const cleaned = stripLeadingPunctuation(text);
  if (!cleaned) return "#";
  const firstChar = cleaned[0];
  const norm = firstChar.normalize("NFD")[0]?.toUpperCase() || firstChar.toUpperCase();
  if (norm >= "A" && norm <= "Z") return norm;
  if (norm >= "0" && norm <= "9") return "#";

  const code = firstChar.codePointAt(0) || 0;
  // 日文平假名 / 片假名五十音罗马字首字母映射
  if ((code >= 0x3041 && code <= 0x3096) || (code >= 0x30a1 && code <= 0x30fa)) {
    const h = code >= 0x30a1 && code <= 0x30fa ? code - 0x60 : code;
    if (h <= 0x304a) return "A";
    if (h <= 0x3054) return "K";
    if (h <= 0x305e) return "S";
    if (h <= 0x3069) return "T";
    if (h <= 0x306e) return "N";
    if (h <= 0x307d) return "H";
    if (h <= 0x3082) return "M";
    if (h <= 0x3088) return "Y";
    if (h <= 0x308d) return "R";
    return "W";
  }
  // 韩文谚文音节初声罗马字首字母映射
  if (code >= 0xac00 && code <= 0xd7a3) {
    const choseongIdx = Math.floor((code - 0xac00) / 588);
    return KR_CHOSEONG_INITIALS[choseongIdx] || "#";
  }
  // 中文汉字拼音首字母映射
  if (code >= 0x4e00 && code <= 0x9fff) {
    for (let i = COLLATOR_BOUNDARIES.length - 1; i >= 0; i--) {
      const cmp = pinyinCollator
        ? pinyinCollator.compare(firstChar, COLLATOR_BOUNDARIES[i])
        : firstChar.localeCompare(COLLATOR_BOUNDARIES[i], "zh-CN");
      if (cmp >= 0) {
        return COLLATOR_LETTERS[i];
      }
    }
  }
  return "#";
}

function compareSongsSameDateAlpha(a: Song, b: Song): number {
  const la = extractFirstLetter(a.title);
  const lb = extractFirstLetter(b.title);
  if (la !== lb) {
    if (la === "#") return 1;
    if (lb === "#") return -1;
    return la.localeCompare(lb);
  }
  const cleanA = stripLeadingPunctuation(a.title) || a.title;
  const cleanB = stripLeadingPunctuation(b.title) || b.title;
  return pinyinCollator
    ? pinyinCollator.compare(cleanA, cleanB)
    : cleanA.localeCompare(cleanB, "zh-CN");
}

function normalizeDateKey(raw: string): string {
  const cleaned = (raw || "").trim();
  if (/^\d{4}-\d{2}-\d{2}/.test(cleaned)) {
    return cleaned.slice(0, 10);
  }
  return "";
}

function formatNewSongShortDate(raw?: string): string {
  const key = normalizeDateKey(raw || "");
  if (!key) return "";
  return key.slice(5);
}

function formatRelativePublishDate(raw: string): string {
  const key = normalizeDateKey(raw);
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(key);
  if (!m) {
    if (!raw || raw.length < 10) return "";
    return raw.slice(5, 10);
  }
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const da = Number(m[3]);
  const dObj = new Date(y, mo - 1, da);
  const cur = new Date();
  const todayMidnight = new Date(cur.getFullYear(), cur.getMonth(), cur.getDate()).getTime();
  const diffDays = Math.round((todayMidnight - dObj.getTime()) / 86400000);
  if (diffDays === 0) return "今日";
  if (diffDays === 1) return "昨日";
  return `${String(mo).padStart(2, "0")}-${String(da).padStart(2, "0")}`;
}

function formatDateGroupLabel(dateKey: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(dateKey);
  if (!m) return dateKey;
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const da = Number(m[3]);
  const dObj = new Date(y, mo - 1, da);
  const cur = new Date();
  const todayMidnight = new Date(cur.getFullYear(), cur.getMonth(), cur.getDate()).getTime();
  const targetMidnight = dObj.getTime();
  const diffDays = Math.round((todayMidnight - targetMidnight) / 86400000);
  const mmdd = `${String(mo).padStart(2, "0")}月${String(da).padStart(2, "0")}日`;
  if (diffDays === 0) return `${mmdd} · 今日`;
  if (diffDays === 1) return `${mmdd} · 昨日`;
  return `${mmdd} · ${weekNames[dObj.getDay()]}`;
}

const sortedNewSongsAll = computed<Song[]>(() => {
  const list = store.newSongs
    .map((s) => ({
      ...s,
      publishDate: normalizeDateKey(s.publishDate),
    }))
    .filter((s) => Boolean(s.publishDate));

  return list.sort((a, b) => {
    if (a.publishDate !== b.publishDate) {
      return b.publishDate.localeCompare(a.publishDate);
    }
    return compareSongsSameDateAlpha(a, b);
  });
});

const homeNewSongTotalPages = computed(() =>
  Math.max(1, Math.ceil(sortedNewSongsAll.value.length / homeNewSongPageSize.value))
);

const safeHomeNewSongPage = computed(() =>
  Math.min(homeNewSongPage.value, homeNewSongTotalPages.value - 1)
);

const pagedHomeNewSongs = computed(() => {
  const size = homeNewSongPageSize.value;
  const start = safeHomeNewSongPage.value * size;
  return sortedNewSongsAll.value.slice(start, start + size);
});

function prevHomeNewSongPage() {
  if (safeHomeNewSongPage.value > 0) {
    homeNewSongPage.value = safeHomeNewSongPage.value - 1;
  }
}

function nextHomeNewSongPage() {
  if (safeHomeNewSongPage.value < homeNewSongTotalPages.value - 1) {
    homeNewSongPage.value = safeHomeNewSongPage.value + 1;
  }
}

function playAllNewSongs() {
  if (sortedNewSongsAll.value.length > 0) {
    store.playSongFromList(sortedNewSongsAll.value[0], sortedNewSongsAll.value, "新歌速递");
  }
}

function playDateGroupSongs(songs: Song[]) {
  if (songs.length > 0) {
    store.playSongFromList(songs[0], songs, "新歌速递");
  }
}

const groupedNewSongsByDate = computed(() => {
  const map = new Map<string, Song[]>();
  for (const s of sortedNewSongsAll.value) {
    const d = s.publishDate;
    if (!map.has(d)) map.set(d, []);
    map.get(d)!.push(s);
  }
  return Array.from(map.entries()).map(([dateKey, songs]) => ({
    dateKey,
    displayLabel: formatDateGroupLabel(dateKey),
    songs: [...songs].sort(compareSongsSameDateAlpha),
  }));
});

function getStageViewport(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  return document.querySelector(".skyline-stage-viewport") as HTMLElement | null;
}

function isDateGroupCollapsed(dateKey: string): boolean {
  return Boolean(collapsedDateKeys.value[dateKey]);
}

function toggleDateGroup(dateKey: string) {
  collapsedDateKeys.value = {
    ...collapsedDateKeys.value,
    [dateKey]: !collapsedDateKeys.value[dateKey],
  };
}

const isAllDateGroupsCollapsed = computed(() => {
  const groups = groupedNewSongsByDate.value;
  if (groups.length === 0) return false;
  return groups.every((g) => Boolean(collapsedDateKeys.value[g.dateKey]));
});

function toggleAllDateGroups() {
  const shouldCollapse = !isAllDateGroupsCollapsed.value;
  const nextMap: Record<string, boolean> = {};
  for (const g of groupedNewSongsByDate.value) {
    nextMap[g.dateKey] = shouldCollapse;
  }
  collapsedDateKeys.value = nextMap;
}

// ==================== 巅峰排行榜：五维分类、概念版卡片、全量榜单总览与详情页 ====================
const RANK_CATEGORY_METAS = [
  { key: "官方热榜", label: "官方热榜", subtitle: "全站潮流核心风向标，实时捕捉热播单曲" },
  { key: "特色音乐", label: "特色音乐", subtitle: "短视频、影视综艺、ACG 与世代热歌精选" },
  { key: "曲风流派", label: "曲风流派", subtitle: "电音、说唱、摇滚、民谣、国风与纯音乐垂类榜单" },
  { key: "地区语种", label: "地区语种", subtitle: "华语、粤语、欧美、日韩及小语种地区热歌榜" },
  { key: "全球权威", label: "全球权威", subtitle: "Billboard、Oricon、UK、Melon 等国际权威榜单" },
] as const;

const homeRankCategory = ref<string>("featured");
const rankHomeCategoryTabs = [
  { id: "featured", key: "featured", label: "精选推荐", color: "var(--accent-coral)" },
  { id: "官方热榜", key: "官方热榜", label: "官方热榜", color: "var(--accent-coral)" },
  { id: "特色音乐", key: "特色音乐", label: "特色音乐", color: "var(--accent-mint)" },
  { id: "曲风流派", key: "曲风流派", label: "曲风流派", color: "var(--accent-violet)" },
  { id: "地区语种", key: "地区语种", label: "地区语种", color: "var(--accent-cyan)" },
  { id: "全球权威", key: "全球权威", label: "全球权威", color: "var(--accent-gold)" },
];

const homeDoubleRowRanks = computed(() => {
  if (homeRankCategory.value === "featured") {
    return store.rankList.slice(0, 8);
  }
  const matched = store.rankList.filter((r) => (r.category || "特色音乐") === homeRankCategory.value);
  return (matched.length > 0 ? matched : store.rankList).slice(0, 8);
});

const rankModalCategory = ref<string>("all");
const rankModalSearchKeyword = ref<string>("");
const romScrollBodyRef = ref<HTMLElement | null>(null);

watch(rankModalCategory, () => {
  nextTick(() => {
    if (romScrollBodyRef.value) {
      romScrollBodyRef.value.scrollTop = 0;
    }
  });
});

function openAllRanksOverview() {
  rankModalSearchKeyword.value = "";
  store.isRankGridModalOpen = true;
}

const rankOverviewCategories = computed(() => {
  const total = store.rankList.length;
  const counts: Record<string, number> = {};
  for (const r of store.rankList) {
    const c = r.category || "特色音乐";
    counts[c] = (counts[c] || 0) + 1;
  }
  return [
    { key: "all", label: "全部榜单", count: total },
    ...RANK_CATEGORY_METAS.map((m) => ({
      key: m.key,
      label: m.label,
      count: counts[m.key] || 0,
    })).filter((item) => item.count > 0),
  ];
});

const groupedModalRankSections = computed(() => {
  const kw = rankModalSearchKeyword.value.trim().toLowerCase();
  const targetCat = rankModalCategory.value;

  const filtered = store.rankList.filter((rk) => {
    if (targetCat !== "all" && (rk.category || "特色音乐") !== targetCat) {
      return false;
    }
    if (!kw) return true;
    const displayName = (rk.name || rk.rankName || "").toLowerCase();
    const inName = displayName.includes(kw);
    const inCat = (rk.category || "").toLowerCase().includes(kw);
    const previews = rk.songPreviews || rk.topSongsPreview || [];
    const inPreview = previews.some((p) => p.toLowerCase().includes(kw));
    return inName || inCat || inPreview;
  });

  const sections: Array<{ category: string; subtitle: string; ranks: RankItem[] }> = [];
  for (const meta of RANK_CATEGORY_METAS) {
    const items = filtered.filter((r) => (r.category || "特色音乐") === meta.key);
    if (items.length > 0) {
      sections.push({
        category: meta.label,
        subtitle: meta.subtitle,
        ranks: items,
      });
    }
  }

  // 兜底：若存在未归入预设五类的榜单，追加显示
  const knownKeys = new Set<string>(RANK_CATEGORY_METAS.map((m) => m.key));
  const others = filtered.filter((r) => !knownKeys.has(r.category || "特色音乐"));
  if (others.length > 0) {
    sections.push({
      category: "其他榜单",
      subtitle: "更多垂直领域特色榜单",
      ranks: others,
    });
  }

  return sections;
});

function getRankThemeClass(rk: RankItem | null | undefined, fallbackIdx = 0): string {
  if (!rk) return "rank-theme-coral";
  const name = rk.name || rk.rankName || "";
  const cat = rk.category || "";
  if (name.includes("飙升") || name.includes("TOP500") || name.includes("热歌")) {
    return "rank-theme-coral";
  }
  if (name.includes("新歌") || cat === "地区语种") {
    return "rank-theme-cyan";
  }
  if (cat === "曲风流派" || name.includes("电音") || name.includes("说唱") || name.includes("ACG")) {
    return "rank-theme-violet";
  }
  if (cat === "全球权威" || name.includes("Billboard") || name.includes("UK") || name.includes("金曲")) {
    return "rank-theme-amber";
  }
  if (cat === "特色音乐") {
    return "rank-theme-emerald";
  }
  const themes = [
    "rank-theme-coral",
    "rank-theme-cyan",
    "rank-theme-violet",
    "rank-theme-amber",
    "rank-theme-emerald",
  ];
  return themes[fallbackIdx % themes.length];
}

function sanitizePreviewRawLine(raw: string): string {
  return (raw || "")
    .trim()
    .replace(/^\d+\s*[.、．\-\s]\s*/, "")
    .trim();
}

function getRankPreviewLines(rk: RankItem): string[] {
  const previews = rk.songPreviews && rk.songPreviews.length > 0 ? rk.songPreviews : rk.topSongsPreview;
  if (previews && previews.length > 0) {
    return previews.slice(0, 3).map(sanitizePreviewRawLine).filter(Boolean);
  }
  const fallbackPool = store.recommendSongs.length > 0 ? store.recommendSongs : store.newSongs;
  if (fallbackPool.length >= 3) {
    const offset = rk.rankId % Math.max(1, fallbackPool.length - 3);
    return fallbackPool.slice(offset, offset + 3).map((s) => `${s.title} - ${s.artist}`);
  }
  return ["概念版实时热播曲目", "无损母带音源飙升中", "点击查看完整权威榜单"];
}

function getRankPreviewStructured(rk: RankItem): Array<{ title: string; artist: string }> {
  const lines = getRankPreviewLines(rk);
  return lines.map((line) => {
    const clean = sanitizePreviewRawLine(line);
    const sepIdx = clean.indexOf(" - ");
    if (sepIdx > 0) {
      return {
        title: clean.slice(0, sepIdx).trim(),
        artist: clean.slice(sepIdx + 3).trim(),
      };
    }
    return { title: clean, artist: "" };
  });
}

function getRankFirstPreviewText(rk: RankItem): string {
  const previews = rk.songPreviews && rk.songPreviews.length > 0 ? rk.songPreviews : rk.topSongsPreview;
  if (previews && previews.length > 0) {
    return sanitizePreviewRawLine(previews[0]);
  }
  return "";
}

const activeRankRangeKey = ref<string>("all");
const rankPaneCategory = ref<string>("官方热榜");

const rankQuickSwitchCategories = [
  { key: "官方热榜", label: "官方榜" },
  { key: "特色音乐", label: "特色" },
  { key: "曲风流派", label: "曲风" },
  { key: "地区语种", label: "语种" },
  { key: "全球权威", label: "全球" },
];

const filteredRankQuickSwitchList = computed(() => {
  const key = rankPaneCategory.value;
  const knownKeys = new Set(rankQuickSwitchCategories.map((c) => c.key));
  const matched = store.rankList.filter((r: RankItem) => {
    const cat = r.category || "特色音乐";
    if (key === "特色音乐") {
      return cat === "特色音乐" || !knownKeys.has(cat);
    }
    return cat === key;
  });
  return matched.length > 0 ? matched : store.rankList;
});

function selectRankFromLeftPane(rk: RankItem) {
  store.openRankDetail(rk);
  const viewport = getStageViewport();
  if (viewport) {
    viewport.scrollTo({ top: 0, behavior: "smooth" });
  }
}

watch(
  () => store.activeRank?.rankId,
  () => {
    activeRankRangeKey.value = "all";
    const cat = store.activeRank?.category;
    if (cat && rankQuickSwitchCategories.some((c) => c.key === cat)) {
      rankPaneCategory.value = cat;
    }
  },
  { immediate: true }
);

const rankRangeSegments = computed(() => {
  const total = store.activeRankSongs.length;
  if (total <= 100) return [];
  const segs: Array<{ key: string; label: string; start: number; end: number }> = [
    { key: "all", label: `全部 (${total})`, start: 1, end: total },
  ];
  const step = 100;
  for (let start = 1; start <= total; start += step) {
    const end = Math.min(total, start + step - 1);
    segs.push({
      key: `${start}-${end}`,
      label: `${start}-${end}`,
      start,
      end,
    });
  }
  return segs;
});

const activeRankCover = computed(() => {
  if (store.activeRank?.coverUrl) return store.activeRank.coverUrl;
  const firstWithCover = store.activeRankSongs.find((s) => Boolean(s.coverUrl));
  return firstWithCover?.coverUrl || "";
});

const activeRankHiResCount = computed(
  () =>
    store.activeRankSongs.filter(
      (s) => s.hasHiRes || s.hasSq || s.isLossless || s.isHiRes
    ).length
);

const filteredRankSongsWithIndex = computed<Array<{ song: Song; rankNo: number }>>(() => {
  const allWithRank = store.activeRankSongs.map((song, idx) => ({
    song,
    rankNo: idx + 1,
  }));

  if (activeRankRangeKey.value !== "all") {
    const parts = activeRankRangeKey.value.split("-").map(Number);
    if (parts.length === 2 && !Number.isNaN(parts[0]) && !Number.isNaN(parts[1])) {
      const [start, end] = parts;
      return allWithRank.filter(({ rankNo }) => rankNo >= start && rankNo <= end);
    }
  }

  return allWithRank;
});

const discoverLocateSongs = computed<Song[]>(() => {
  switch (store.discoverSubView) {
    case "daily":
    case "fm_detail":
      return filteredHeroSongs.value;
    case "new_songs":
      return sortedNewSongsAll.value;
    case "rank":
      return filteredRankSongsWithIndex.value.map((x) => x.song);
    case "radio":
      return store.fmStationSongs;
    default:
      return [];
  }
});

function playAllActiveRank() {
  if (store.activeRankSongs.length > 0) {
    store.playSongFromList(
      store.activeRankSongs[0],
      store.activeRankSongs,
      store.activeRank?.name || store.activeRank?.rankName || "巅峰榜单"
    );
  }
}

// ==================== 精选场景电台 & 电台广场 ====================
const homeFmCategory = ref<string>("推荐");
const homeFmPage = ref<number>(0);

const homeFmCategoryTabs = [
  { id: "推荐", label: "推荐", color: "var(--accent-coral)" },
  { id: "场景", label: "场景", color: "var(--accent-mint)" },
  { id: "心情", label: "心情", color: "var(--accent-gold)" },
  { id: "风格", label: "风格", color: "var(--accent-violet)" },
  { id: "语言", label: "语言", color: "var(--accent-cyan)" },
  { id: "国风", label: "国风", color: "var(--accent-mint)" },
];

const homeCategoryFilteredFmChannels = computed(() => {
  const cat = homeFmCategory.value;
  if (!cat || cat === "全部") return store.fmChannels;
  if (cat === "推荐") {
    const rec = store.fmChannels.filter((c) => c.isRecommended || c.category === "推荐");
    return rec.length >= 6 ? rec : store.fmChannels;
  }
  const matched = store.fmChannels.filter((c) => c.category === cat || c.category.includes(cat));
  return matched.length > 0 ? matched : store.fmChannels;
});

const singleRowFmChannels = computed(() => {
  const pool = homeCategoryFilteredFmChannels.value;
  if (pool.length <= 6) return pool;
  const pageSize = 6;
  const totalPages = Math.max(1, Math.ceil(pool.length / pageSize));
  const page = homeFmPage.value % totalPages;
  const start = page * pageSize;
  const slice = pool.slice(start, start + pageSize);
  if (slice.length < pageSize) {
    return [...slice, ...pool.slice(0, pageSize - slice.length)];
  }
  return slice;
});

function switchHomeFmCategory(catId: string) {
  homeFmCategory.value = catId;
  homeFmPage.value = 0;
}

const isRotatingHomeFm = ref(false);
let rotateHomeFmTimer: ReturnType<typeof setTimeout> | null = null;

function rotateHomeFmPage() {
  homeFmPage.value += 1;
  isRotatingHomeFm.value = true;
  if (rotateHomeFmTimer) clearTimeout(rotateHomeFmTimer);
  rotateHomeFmTimer = setTimeout(() => {
    isRotatingHomeFm.value = false;
    rotateHomeFmTimer = null;
  }, 420);
}

onBeforeUnmount(() => {
  if (rotateHomeFmTimer) {
    clearTimeout(rotateHomeFmTimer);
    rotateHomeFmTimer = null;
  }
});

const radioCoverFallbackPool = computed(() => {
  const pool: string[] = [];
  for (const ch of store.fmChannels) {
    if (ch.coverUrl && !pool.includes(ch.coverUrl)) pool.push(ch.coverUrl);
  }
  for (const s of store.newSongs) {
    if (s.coverUrl && !pool.includes(s.coverUrl)) pool.push(s.coverUrl);
  }
  for (const s of store.recommendSongs) {
    if (s.coverUrl && !pool.includes(s.coverUrl)) pool.push(s.coverUrl);
  }
  return pool;
});

function getStationCover(ch: FmChannel, idx: number): string {
  if (ch.coverUrl && ch.coverUrl.trim().length > 0) {
    return ch.coverUrl;
  }
  const pool = radioCoverFallbackPool.value;
  if (pool.length > 0) {
    return pool[idx % pool.length];
  }
  return store.currentSong?.coverUrl || "";
}

function isChannelActivelyPlaying(channelId: string): boolean {
  if (!store.isPlaying || !store.currentSong || !channelId) return false;
  if (store.activeFmChannel?.id !== channelId) return false;
  return store.fmStationSongs.some((s) => s.hash === store.currentSong?.hash);
}

function toggleOrPlayFmChannel(ch: FmChannel) {
  if (store.activeFmChannel?.id === ch.id && store.fmStationSongs.length > 0) {
    const isPlayingThisStation = store.fmStationSongs.some(
      (s) => s.hash === store.currentSong?.hash
    );
    if (isPlayingThisStation) {
      store.togglePlayPause();
      return;
    }
    store.playSongFromList(
      store.fmStationSongs[0],
      store.fmStationSongs,
      `FM · ${ch.name}`
    );
    return;
  }
  store.activeFmChannel = ch;
  store.loadFmStationSongs(ch.id, false, true);
}

const radioModalOpen = ref(false);
const radioModalChannel = ref<FmChannel | null>(null);
const radioModalLoading = ref(false);
const radioModalSongsMap = ref<Record<string, Song[]>>({});

const radioModalSongs = computed<Song[]>(() => {
  const ch = radioModalChannel.value;
  if (!ch) return [];
  if (store.activeFmChannel?.id === ch.id && store.fmStationSongs.length > 0) {
    return store.fmStationSongs;
  }
  return radioModalSongsMap.value[ch.id] || [];
});

const radioModalCover = computed(() => {
  const ch = radioModalChannel.value;
  if (!ch) return "";
  return getStationCover(ch, 0) || radioModalSongs.value[0]?.coverUrl || "";
});

async function openRadioPreviewModal(ch: FmChannel) {
  radioModalChannel.value = ch;
  radioModalOpen.value = true;
  if (store.activeFmChannel?.id === ch.id && store.fmStationSongs.length > 0) {
    radioModalSongsMap.value = {
      ...radioModalSongsMap.value,
      [ch.id]: store.fmStationSongs,
    };
    return;
  }
  const cached = radioModalSongsMap.value[ch.id];
  if (cached && cached.length > 0) return;

  radioModalLoading.value = true;
  try {
    const songs = await kugouClient.getFmSongs(ch.id);
    radioModalSongsMap.value = {
      ...radioModalSongsMap.value,
      [ch.id]: songs,
    };
  } catch (e) {
    store.notify(`获取电台队列失败：${String(e)}`);
  } finally {
    radioModalLoading.value = false;
  }
}

async function handleRadioModalPlay() {
  const ch = radioModalChannel.value;
  if (!ch) return;
  if (isChannelActivelyPlaying(ch.id)) {
    store.togglePlayPause();
    return;
  }
  let songs = radioModalSongs.value;
  if (songs.length === 0 && !radioModalLoading.value) {
    await handleRadioModalRefresh();
    songs = radioModalSongs.value;
  }
  if (songs.length > 0) {
    store.activeFmChannel = ch;
    store.fmStationSongs = [...songs];
    store.playSongFromList(songs[0], store.fmStationSongs, `FM · ${ch.name}`);
  }
}

async function handleRadioModalRefresh() {
  const ch = radioModalChannel.value;
  if (!ch || radioModalLoading.value) return;
  radioModalLoading.value = true;
  try {
    const batch = await kugouClient.getFmSongs(ch.id);
    radioModalSongsMap.value = {
      ...radioModalSongsMap.value,
      [ch.id]: batch,
    };
    if (store.activeFmChannel?.id === ch.id) {
      store.fmStationSongs = batch;
    }
    store.notify(`已刷新「${ch.name}」电台待播队列（${batch.length} 首）`);
  } catch (e) {
    store.notify(`刷新电台队列失败：${String(e)}`);
  } finally {
    radioModalLoading.value = false;
  }
}

async function handleRadioModalAppend() {
  const ch = radioModalChannel.value;
  if (!ch) return;
  let songs = radioModalSongs.value;
  if (songs.length === 0 && !radioModalLoading.value) {
    await handleRadioModalRefresh();
    songs = radioModalSongs.value;
  }
  if (songs.length > 0) {
    store.batchAppendSongsToQueue(songs);
  }
}

function handleRadioModalGoPlaza() {
  const ch = radioModalChannel.value;
  radioModalOpen.value = false;
  if (ch) {
    store.activeFmChannel = ch;
    const modalList = radioModalSongsMap.value[ch.id];
    if (modalList && modalList.length > 0) {
      store.fmStationSongs = [...modalList];
    } else {
      store.loadFmStationSongs(ch.id, false, false);
    }
    store.openRadioPlazaPage();
    const viewport = getStageViewport();
    if (viewport) {
      viewport.scrollTo({ top: 0, behavior: "smooth" });
    }
  } else {
    openRadioPlaza();
  }
}

function handleRadioModalPlaySong(song: Song) {
  const ch = radioModalChannel.value;
  if (!ch) return;
  const list = radioModalSongs.value.length > 0 ? radioModalSongs.value : [song];
  store.activeFmChannel = ch;
  store.fmStationSongs = [...list];
  store.playSongFromList(song, store.fmStationSongs, `FM · ${ch.name}`);
}

const activeFmDialPercent = computed(() => {
  const freqStr = store.activeFmChannel?.freqLabel || "FM 98.6";
  const m = /(\d+(?:\.\d+)?)/.exec(freqStr);
  const val = m ? parseFloat(m[1]) : 98.6;
  const clamped = Math.min(108, Math.max(87.5, val));
  return Math.round(((clamped - 87.5) / (108 - 87.5)) * 100);
});

function toggleOrPlayActiveStation() {
  if (!store.activeFmChannel) {
    if (store.fmChannels.length > 0) {
      toggleOrPlayFmChannel(store.fmChannels[0]);
    }
    return;
  }
  toggleOrPlayFmChannel(store.activeFmChannel);
}

function openRadioPlaza() {
  store.openRadioPlazaPage();
  if (!store.activeFmChannel && store.fmChannels.length > 0) {
    store.activeFmChannel = store.fmChannels[0];
    store.loadFmStationSongs(store.fmChannels[0].id, false, false);
  } else if (store.activeFmChannel && store.fmStationSongs.length === 0 && !store.isLoadingFmStation) {
    store.loadFmStationSongs(store.activeFmChannel.id, false, false);
  }
}

const isPlazaExpandedAll = ref(false);
const plazaVisibleLimit = 24;

const baseRadioPlazaCategories = [
  { id: "全部", label: "全部频率" },
  { id: "推荐", label: "推荐" },
  { id: "场景", label: "场景" },
  { id: "心情", label: "心情" },
  { id: "风格", label: "风格" },
  { id: "语言", label: "语言" },
  { id: "主题", label: "主题" },
  { id: "DJ", label: "DJ" },
  { id: "运动", label: "运动" },
  { id: "国风", label: "国风" },
  { id: "乐器", label: "乐器" },
  { id: "人群综艺", label: "人群综艺" },
  { id: "儿童", label: "儿童" },
];

function matchChannelToCategory(ch: FmChannel, catId: string): boolean {
  if (!catId || catId === "全部") return true;
  if (catId === "推荐" || catId === "推荐电台") {
    return Boolean(ch.isRecommended) || ch.category === "推荐";
  }
  return ch.category === catId || ch.category.includes(catId) || catId.includes(ch.category);
}

const radioPlazaCategoriesWithCount = computed(() => {
  const all = store.fmChannels;
  return baseRadioPlazaCategories
    .map((cat) => {
      const count =
        cat.id === "全部"
          ? all.length
          : all.filter((c) => matchChannelToCategory(c, cat.id)).length;
      return {
        ...cat,
        count,
      };
    })
    .filter((cat) => cat.id === "全部" || cat.count > 0);
});

function selectRadioPlazaCategory(catId: string) {
  store.selectedFmCategory = catId;
  isPlazaExpandedAll.value = false;
}

const filteredPlazaChannels = computed(() => {
  const cat = store.selectedFmCategory || "全部";
  let list = store.fmChannels;

  if (cat && cat !== "全部") {
    const byCat = list.filter((c) => matchChannelToCategory(c, cat));
    if (byCat.length > 0) {
      list = byCat;
    }
  }

  return list;
});

const primaryPlazaChannels = computed(() => {
  const list = filteredPlazaChannels.value;
  const cat = store.selectedFmCategory || "全部";
  if (cat !== "全部" && list.length <= 36) {
    return list;
  }
  return list.slice(0, plazaVisibleLimit);
});

const extraPlazaChannels = computed(() => {
  const list = filteredPlazaChannels.value;
  const primaryLen = primaryPlazaChannels.value.length;
  return primaryLen < list.length ? list.slice(primaryLen) : [];
});

const displayedPlazaChannels = computed(() =>
  isPlazaExpandedAll.value
    ? filteredPlazaChannels.value
    : primaryPlazaChannels.value
);

const activePlazaCover = computed(() => {
  if (store.activeFmChannel) {
    const c = getStationCover(store.activeFmChannel, 0);
    if (c) return c;
  }
  return store.fmStationSongs[0]?.coverUrl || store.currentSong?.coverUrl || "";
});

function selectAndPlayPlazaChannel(ch: FmChannel, autoPlay = false) {
  store.activeFmChannel = ch;
  store.loadFmStationSongs(ch.id, false, autoPlay);
}

function refreshCurrentRadioStation() {
  const cid = store.activeFmChannel?.id || store.fmChannels[0]?.id || "9";
  store.loadFmStationSongs(cid, false, false);
}

function appendCurrentRadioStation() {
  const cid = store.activeFmChannel?.id || store.fmChannels[0]?.id || "9";
  store.loadFmStationSongs(cid, true, false);
}

function playActiveStationAll() {
  if (store.fmStationSongs.length > 0) {
    store.playSongFromList(
      store.fmStationSongs[0],
      store.fmStationSongs,
      `FM · ${store.activeFmChannel?.name || "电台"}`
    );
  } else if (store.activeFmChannel) {
    store.loadFmStationSongs(store.activeFmChannel.id, false, true);
  }
}

function scrollToCurrentPlayingRadioSong() {
  if (!store.currentSong?.hash) return;
  const safeHash =
    typeof CSS !== "undefined" && typeof CSS.escape === "function"
      ? CSS.escape(store.currentSong.hash)
      : store.currentSong.hash.replace(/["\\]/g, "\\$&");
  const el = document.querySelector(`[data-radio-song-hash="${safeHash}"]`);
  if (el) {
    el.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}
</script>

<style scoped>
.inline-meta-btn {
  background: none;
  border: none;
  padding: 1px 5px;
  margin: -1px -5px;
  border-radius: var(--bm-r-xs, 6px);
  color: inherit;
  font: inherit;
  cursor: pointer;
  text-decoration: none;
  transition: color 0.16s ease, background-color 0.16s ease;
}
.inline-meta-btn:hover {
  color: var(--primary);
  background: color-mix(in srgb, var(--primary) 11%, transparent);
}

.ns-clean-header {
  padding-bottom: 14px;
}

.ns-header-title-group {
  display: inline-flex;
  align-items: baseline;
  gap: 10px;
}

.ns-header-date-pill {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-semibold, 600);
  font-variant-numeric: tabular-nums;
  color: var(--text-secondary);
}

.ns-empty-hint {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 64px 20px;
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-secondary);
}

.ns-frameless-date-group {
  margin-bottom: 16px;
}

.ns-frameless-date-group.is-collapsed {
  margin-bottom: 8px;
}

.icon-flip-x {
  display: inline-flex;
  transform: scaleX(-1);
}

.icon-rotate-180 {
  transform: rotate(180deg);
}

.ns-collapse-chevron {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: var(--bm-r-sm, 10px);
  color: var(--text-secondary);
  transition: transform 0.26s cubic-bezier(0.22, 1, 0.36, 1), color 0.18s ease;
}

.ns-flow-date-header:hover .ns-collapse-chevron {
  color: var(--text-primary);
}

.ns-collapse-chevron.collapsed {
  transform: rotate(-90deg);
}

.ns-date-collapse-body {
  display: grid;
  grid-template-rows: 1fr;
  opacity: 1;
  margin-top: 8px;
  transition:
    grid-template-rows 0.28s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.22s ease,
    margin-top 0.24s ease;
}

.ns-date-collapse-body.collapsed {
  grid-template-rows: 0fr;
  opacity: 0;
  margin-top: 0;
  pointer-events: none;
}

.ns-date-collapse-inner {
  overflow: hidden;
  min-height: 0;
  container-type: inline-size;
}

.plaza-extra-collapse {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transition:
    grid-template-rows 0.32s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.24s ease;
}

.plaza-extra-collapse.open {
  grid-template-rows: 1fr;
  opacity: 1;
}

.plaza-extra-collapse-inner {
  overflow: hidden;
  min-height: 0;
}

.plaza-extra-gallery {
  padding-top: 14px;
}

@media (prefers-reduced-motion: reduce) {
  .ns-collapse-chevron,
  .ns-date-collapse-body,
  .plaza-extra-collapse {
    transition: none !important;
  }
}
</style>
