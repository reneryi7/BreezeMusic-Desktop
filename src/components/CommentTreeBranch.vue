<script setup lang="ts">
import { formatCommentTimeText } from "../api/kugou";
import { isSameCommentAuthor, type CommentTreeNode } from "../utils/commentTree";
import type { SongComment } from "../types";
import PlumpIcon from "./PlumpIcon.vue";
import RollingCounter from "./RollingCounter.vue";
import { triggerOneShotClass } from "../utils/microMotionFx";

defineOptions({
  name: "CommentTreeBranch",
});

const props = withDefaults(
  defineProps<{
    nodes: CommentTreeNode[];
    rootOwner: SongComment;
    collapsedIds: Set<string>;
    highlightedId?: string | null;
    replyTargetId?: string | null;
    depth?: number;
  }>(),
  {
    highlightedId: null,
    replyTargetId: null,
    depth: 1,
  }
);

const emit = defineEmits<{
  (e: "toggle-collapse", commentId: string): void;
  (e: "toggle-like", comment: SongComment): void;
  (e: "reply", comment: SongComment): void;
  (e: "open-chain", comment: SongComment): void;
  (e: "preview-image", url: string): void;
}>();

function isCollapsed(commentId: string): boolean {
  return props.collapsedIds.has(commentId);
}

function resolveDisplayTime(cmt: SongComment): string {
  if (cmt.timeText && cmt.timeText.trim().length > 0) return cmt.timeText;
  return formatCommentTimeText(cmt.timestampMs, cmt.createdTime);
}

function formatCompactNum(num: number): string {
  if (!num || num <= 0) return "";
  if (num >= 10000) return `${(num / 10000).toFixed(1)}万`;
  return String(num);
}
</script>

<template>
  <div class="ctb-list" :class="{ 'is-nested': depth > 1 }">
    <div
      v-for="(node, idx) in nodes"
      :key="node.comment.id"
      class="ctb-row"
      :data-reply-id="node.comment.id"
    >
      <!-- 左侧树干连线 + 圆角 SVG 折线 + 可点击 - / + 折叠节点 -->
      <div class="ctb-gutter">
        <!-- 非最后一条直接分支时，持续向下延伸垂直主干 -->
        <svg
          v-if="idx < nodes.length - 1"
          class="ctb-spine-svg"
          viewBox="0 0 2 100"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <line
            x1="1"
            y1="0"
            x2="1"
            y2="100"
            stroke="var(--cm-branch-stroke)"
            stroke-width="1.5"
          />
        </svg>

        <!-- 圆角 L 型水平折线，平滑汇聚至子节点头像中心 -->
        <svg
          class="ctb-elbow-svg"
          viewBox="0 0 28 32"
          fill="none"
          aria-hidden="true"
        >
          <path
            d="M 12 0 L 12 13 Q 12 22 21 22 L 28 22"
            stroke="var(--cm-branch-stroke)"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>

        <!-- 可点击的 - / + 圆形节点，整支子分支折叠/展开 -->
        <button
          type="button"
          class="ctb-node-btn"
          :class="{ 'is-collapsed': isCollapsed(node.comment.id) }"
          :title="
            isCollapsed(node.comment.id)
              ? `展开 ${node.descendantCount} 条回复`
              : '折叠该条回复分支'
          "
          :aria-label="
            isCollapsed(node.comment.id)
              ? `展开 ${node.descendantCount} 条回复`
              : '折叠该条回复分支'
          "
          @click.stop="emit('toggle-collapse', node.comment.id)"
        >
          {{ isCollapsed(node.comment.id) ? "+" : "−" }}
        </button>
      </div>

      <!-- 右侧内容区 -->
      <div class="ctb-content">
        <!-- 折叠态：单行紧凑展开胶囊 -->
        <button
          v-if="isCollapsed(node.comment.id)"
          type="button"
          class="ctb-collapsed-pill"
          @click.stop="emit('toggle-collapse', node.comment.id)"
        >
          <span class="ctb-collapsed-user">@{{ node.comment.userName }}</span>
          <span>展开 {{ node.descendantCount }} 条回复</span>
        </button>

        <!-- 展开态：当前回复节点 + 递归子分支 -->
        <template v-else>
          <div
            class="ctb-node-card cm-reply-card"
            :data-reply-id="node.comment.id"
            :class="{
              'is-highlighted': highlightedId === node.comment.id,
              'is-target': replyTargetId === node.comment.id,
            }"
            @click.stop="emit('reply', node.comment)"
          >
            <div class="ctb-main-line">
              <div class="ctb-avatar-col">
                <img
                  v-if="node.comment.userAvatar"
                  :src="node.comment.userAvatar"
                  class="ctb-avatar"
                  alt=""
                />
                <div v-else class="ctb-avatar ctb-avatar-fallback">
                  {{ (node.comment.userName || "友").slice(0, 1) }}
                </div>
              </div>

              <div class="ctb-body-col">
                <div class="ctb-meta-row">
                  <span class="ctb-author">{{ node.comment.userName }}</span>
                  <span
                    v-if="isSameCommentAuthor(node.comment, rootOwner)"
                    class="ctb-owner-badge"
                  >
                    楼主
                  </span>
                  <span v-if="node.comment.location" class="ctb-badge-chip">
                    {{ node.comment.location }}
                  </span>
                  <span v-if="node.comment.vipLevel" class="ctb-lv-badge">
                    LV{{ node.comment.vipLevel }}
                  </span>

                  <template v-if="node.isReplyToSub && node.replyTargetUserName">
                    <span class="ctb-reply-arrow">回复</span>
                    <span class="ctb-target-user">
                      @{{ node.replyTargetUserName }}
                    </span>
                  </template>
                </div>

                <div class="ctb-text">
                  {{ node.comment.content }}
                </div>

                <div
                  v-if="node.comment.images && node.comment.images.length > 0"
                  class="ctb-images"
                  @click.stop
                >
                  <img
                    v-for="(imgUrl, i) in node.comment.images"
                    :key="i"
                    :src="imgUrl"
                    class="ctb-img"
                    alt="reply"
                    @click.stop="emit('preview-image', imgUrl)"
                  />
                </div>

                <div class="ctb-actions-row">
                  <span class="ctb-time">{{ resolveDisplayTime(node.comment) }}</span>

                  <button
                    type="button"
                    class="ctb-action-btn"
                    :class="{ liked: node.comment.isLiked }"
                    @click.stop="
                      !node.comment.isLiked &&
                        triggerOneShotClass(
                          $event.currentTarget as HTMLElement,
                          'is-bouncing',
                          460
                        );
                      emit('toggle-like', node.comment);
                    "
                  >
                    <PlumpIcon
                      :name="node.comment.isLiked ? 'heart-filled' : 'heart'"
                      :size="13"
                    />
                    <RollingCounter
                      v-if="node.comment.likeCount > 0"
                      :value="node.comment.likeCount"
                      :display-text="formatCompactNum(node.comment.likeCount)"
                    />
                  </button>

                  <button
                    type="button"
                    class="ctb-action-btn"
                    @click.stop="emit('reply', node.comment)"
                  >
                    <span>回复</span>
                  </button>

                  <button
                    v-if="node.hasDeepChain"
                    type="button"
                    class="ctb-chain-btn"
                    @click.stop="emit('open-chain', node.comment)"
                  >
                    <span>查看对话脉络 ({{ node.chainLengthHint }}层)</span>
                    <PlumpIcon name="chevron-right" :size="11" />
                  </button>
                </div>
              </div>
            </div>

            <!-- 递归子回复分支：左侧垂直树干精准对齐当前回复头像正下方中心 (x=12px) -->
            <CommentTreeBranch
              v-if="node.children.length > 0"
              :nodes="node.children"
              :root-owner="rootOwner"
              :collapsed-ids="collapsedIds"
              :highlighted-id="highlightedId"
              :reply-target-id="replyTargetId"
              :depth="depth + 1"
              @toggle-collapse="emit('toggle-collapse', $event)"
              @toggle-like="emit('toggle-like', $event)"
              @reply="emit('reply', $event)"
              @open-chain="emit('open-chain', $event)"
              @preview-image="emit('preview-image', $event)"
            />
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ctb-list {
  display: flex;
  flex-direction: column;
}

.ctb-row {
  display: flex;
  align-items: stretch;
  position: relative;
}

.ctb-gutter {
  width: 28px;
  flex-shrink: 0;
  position: relative;
}

.ctb-spine-svg {
  position: absolute;
  left: 11.25px;
  top: 0;
  width: 2px;
  height: 100%;
  pointer-events: none;
}

.ctb-elbow-svg {
  position: absolute;
  left: 0;
  top: 0;
  width: 28px;
  height: 32px;
  pointer-events: none;
}

.ctb-node-btn {
  position: absolute;
  left: 4px;
  top: 4px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: none;
  background: var(--cm-branch-node-bg);
  color: var(--cm-branch-node-color);
  font-size: 11px;
  font-weight: 800;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  z-index: 2;
  transition: transform 0.16s ease, background 0.16s ease, color 0.16s ease;
}

.ctb-node-btn:hover,
.ctb-node-btn.is-collapsed {
  transform: scale(1.12);
  background: var(--cm-accent);
  color: var(--cm-on-accent);
}

.ctb-content {
  flex: 1;
  min-width: 0;
  padding-top: 10px;
}

.ctb-collapsed-pill {
  height: 26px;
  padding: 0 11px;
  border-radius: 999px;
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  font-size: 12px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 2px;
  margin-bottom: 4px;
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease, transform 0.16s ease;
}

.ctb-collapsed-pill:hover {
  background: var(--cm-accent-soft);
  color: var(--cm-accent);
  transform: translateX(2px);
}

.ctb-collapsed-user {
  color: var(--cm-accent);
  font-weight: 800;
}

.ctb-node-card {
  border-radius: 12px;
  transition: background 0.2s ease, box-shadow 0.2s ease;
}

.ctb-node-card.is-highlighted > .ctb-main-line {
  background: var(--cm-accent-soft);
  border-radius: 12px;
}

.ctb-node-card.is-target > .ctb-main-line {
  background: var(--fm-card-elevated);
  border-radius: 12px;
}

.ctb-main-line {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  padding: 4px 8px 6px 0;
  transition: background 0.2s ease;
}

.ctb-avatar-col {
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  position: relative;
}

.ctb-avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  object-fit: cover;
  display: block;
}

.ctb-avatar-fallback {
  background: var(--cm-accent-soft);
  color: var(--cm-accent);
  font-size: 11px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ctb-body-col {
  flex: 1;
  min-width: 0;
}

.ctb-meta-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 5px;
  font-size: 12px;
  line-height: 1.4;
}

.ctb-author {
  font-weight: 700;
  color: var(--fm-text-2);
}

.ctb-owner-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--cm-accent-soft);
  color: var(--cm-accent);
}

.ctb-badge-chip {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 5px;
  background: var(--fm-chip-bg);
  color: var(--fm-text-3);
}

.ctb-lv-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 1px 5px;
  border-radius: 5px;
  background: linear-gradient(135deg, #f43f5e, #fb7185);
  color: #ffffff;
}

.ctb-reply-arrow {
  color: var(--fm-text-3);
  font-size: 11.5px;
}

.ctb-target-user {
  color: var(--cm-accent);
  font-weight: 700;
  font-size: 12px;
}

.ctb-text {
  margin-top: 3px;
  font-size: 13px;
  line-height: 1.55;
  color: var(--fm-text-1);
  word-break: break-word;
}

.ctb-images {
  margin-top: 6px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.ctb-img {
  width: 68px;
  height: 68px;
  border-radius: 9px;
  object-fit: cover;
  cursor: zoom-in;
}

.ctb-actions-row {
  margin-top: 4px;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 11.5px;
  color: var(--fm-text-3);
}

.ctb-time {
  font-variant-numeric: tabular-nums;
}

.ctb-action-btn {
  border: none;
  background: transparent;
  color: var(--fm-text-3);
  font-size: 11.5px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 6px;
  transition: color 0.16s ease, background 0.16s ease;
}

.ctb-action-btn:hover {
  color: var(--fm-text-1);
  background: var(--fm-chip-bg);
}

.ctb-action-btn.liked {
  color: var(--cm-accent);
}

.ctb-chain-btn {
  border: none;
  background: var(--cm-accent-soft);
  color: var(--cm-accent);
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  cursor: pointer;
  transition: filter 0.16s ease;
}

.ctb-chain-btn:hover {
  filter: brightness(1.1);
}
</style>
