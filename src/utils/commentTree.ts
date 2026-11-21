import type { SongComment } from "../types";

export interface CommentTreeNode {
  comment: SongComment;
  depth: number;
  children: CommentTreeNode[];
  descendantCount: number;
  isReplyToSub: boolean;
  replyTargetUserName: string;
  hasDeepChain: boolean;
  chainLengthHint: number;
}

export function isSameCommentAuthor(
  a: SongComment | null | undefined,
  b: SongComment | null | undefined
): boolean {
  if (!a || !b) return false;
  if (a.userId && b.userId && a.userId === b.userId) return true;
  const na = (a.userName || "").trim().toLowerCase();
  const nb = (b.userName || "").trim().toLowerCase();
  return Boolean(na && nb && na === nb);
}

function normalizeTextForMatch(raw: string | null | undefined): string {
  return String(raw || "")
    .replace(/\s+/g, " ")
    .trim();
}

function countDescendants(node: CommentTreeNode): number {
  let total = 1;
  for (const child of node.children) {
    total += countDescendants(child);
  }
  node.descendantCount = total;
  return total;
}

function assignDepthAndMeta(
  node: CommentTreeNode,
  depth: number,
  rootOwner: SongComment,
  parentNode: CommentTreeNode | null
) {
  node.depth = depth;
  const rootName = normalizeTextForMatch(rootOwner.userName);
  const rawTargetName = normalizeTextForMatch(
    node.comment.replyToUser || node.comment.replyUserName
  );
  const resolvedTargetName = parentNode
    ? normalizeTextForMatch(parentNode.comment.userName) || rawTargetName
    : rawTargetName;

  const isReplyToSub =
    Boolean(parentNode) ||
    Boolean(resolvedTargetName && resolvedTargetName !== rootName);

  const hopsCount = (node.comment.replyChainHops || []).filter(
    (h) => normalizeTextForMatch(h.userName) !== rootName
  ).length;
  const effectiveDepth = Math.max(depth, hopsCount + 1);

  node.isReplyToSub = isReplyToSub;
  node.replyTargetUserName = resolvedTargetName;
  node.hasDeepChain = effectiveDepth >= 5;
  node.chainLengthHint = effectiveDepth + 1;

  for (const child of node.children) {
    assignDepthAndMeta(child, depth + 1, rootOwner, node);
  }
}

/**
 * 将平铺的楼中楼回复列表重构为多层级树形对话结构：
 * 1) 优先按 `replyTargetCommentId` / `parentId` 精确挂接；
 * 2) 次选按引用正文片段 `replyQuotedContent` 匹配唯一先序回复；
 * 3) 再按被 `@` 目标用户名匹配最近先序回复（排除直回楼主）；
 * 4) 其余全部作为顶层一级分支直接挂接在楼主树干下。
 */
export function buildReplyClusterTree(
  rootComment: SongComment,
  replies: SongComment[]
): CommentTreeNode[] {
  if (!replies || replies.length === 0) return [];

  const rootId = String(rootComment.id || "").trim();
  const rootName = normalizeTextForMatch(rootComment.userName);
  const rootContent = normalizeTextForMatch(rootComment.content);

  const uniqueReplies: SongComment[] = [];
  const seenIds = new Set<string>();
  for (const item of replies) {
    const id = String(item?.id || "").trim();
    if (!id || id === rootId || seenIds.has(id)) continue;
    seenIds.add(id);
    uniqueReplies.push(item);
  }

  const nodeById = new Map<string, CommentTreeNode>();
  const orderedNodes: CommentTreeNode[] = uniqueReplies.map((comment) => {
    const node: CommentTreeNode = {
      comment,
      depth: 1,
      children: [],
      descendantCount: 1,
      isReplyToSub: false,
      replyTargetUserName: "",
      hasDeepChain: false,
      chainLengthHint: 2,
    };
    nodeById.set(comment.id, node);
    return node;
  });

  const parentMap = new Map<string, string | null>();
  const byAuthor = new Map<string, CommentTreeNode[]>();

  for (let i = 0; i < orderedNodes.length; i++) {
    const cur = orderedNodes[i];
    const cmt = cur.comment;
    const explicitPid = normalizeTextForMatch(
      cmt.replyTargetCommentId || cmt.parentId
    );
    let resolvedParentId: string | null = null;

    if (
      explicitPid &&
      explicitPid !== "0" &&
      explicitPid !== rootId &&
      explicitPid !== cmt.id &&
      nodeById.has(explicitPid)
    ) {
      resolvedParentId = explicitPid;
    }

    if (!resolvedParentId) {
      const quoted = normalizeTextForMatch(cmt.replyQuotedContent);
      const targetUser = normalizeTextForMatch(
        cmt.replyToUser || cmt.replyUserName
      ).toLowerCase();
      if (quoted && quoted !== rootContent) {
        const matchedQuote = orderedNodes.find((cand) => {
          if (cand.comment.id === cmt.id) return false;
          const candUser = normalizeTextForMatch(
            cand.comment.userName
          ).toLowerCase();
          if (targetUser && candUser !== targetUser) return false;
          const candText = normalizeTextForMatch(cand.comment.content);
          return candText === quoted || (quoted.length >= 6 && candText.includes(quoted));
        });
        if (matchedQuote) {
          resolvedParentId = matchedQuote.comment.id;
        }
      }
    }

    if (!resolvedParentId) {
      const targetName = normalizeTextForMatch(
        cmt.replyToUser || cmt.replyUserName
      );
      if (targetName && targetName !== rootName) {
        const key = targetName.toLowerCase();
        const priorFromUser = byAuthor.get(key);
        if (priorFromUser && priorFromUser.length > 0) {
          const latest = priorFromUser[priorFromUser.length - 1];
          if (latest.comment.id !== cmt.id) {
            resolvedParentId = latest.comment.id;
          }
        }
      }
    }

    parentMap.set(cmt.id, resolvedParentId);

    const authorKey = normalizeTextForMatch(cmt.userName).toLowerCase();
    if (authorKey) {
      const arr = byAuthor.get(authorKey);
      if (arr) {
        arr.push(cur);
      } else {
        byAuthor.set(authorKey, [cur]);
      }
    }
  }

  const wouldCreateCycle = (childId: string, candidateParentId: string): boolean => {
    let cursor: string | null | undefined = candidateParentId;
    let guard = 0;
    while (cursor && guard < orderedNodes.length + 2) {
      if (cursor === childId) return true;
      cursor = parentMap.get(cursor) || null;
      guard++;
    }
    return false;
  };

  const roots: CommentTreeNode[] = [];
  for (const node of orderedNodes) {
    const pid = parentMap.get(node.comment.id);
    if (pid && !wouldCreateCycle(node.comment.id, pid)) {
      const parentNode = nodeById.get(pid);
      if (parentNode) {
        parentNode.children.push(node);
        continue;
      }
    }
    roots.push(node);
  }

  for (const r of roots) {
    assignDepthAndMeta(r, 1, rootComment, null);
    countDescendants(r);
  }

  return roots;
}
