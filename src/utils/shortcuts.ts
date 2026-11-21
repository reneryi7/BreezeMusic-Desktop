import type {
  GlobalHotkeyBinding,
  GlobalShortcutActionId,
  InAppShortcutActionId,
} from "../types";

export interface ShortcutMetaItem<T extends string = string> {
  action: T;
  label: string;
  description: string;
  category: "playback" | "window" | "navigation";
  defaultAccelerator: string;
}

export const GLOBAL_SHORTCUT_ITEMS: ReadonlyArray<
  ShortcutMetaItem<GlobalShortcutActionId>
> = [
  {
    action: "toggle_play",
    label: "播放 / 暂停",
    description: "后台任意界面控制播放或暂停（硬件媒体键同时生效）",
    category: "playback",
    defaultAccelerator: "Ctrl+Alt+Space",
  },
  {
    action: "prev_song",
    label: "上一首",
    description: "切换到队列上一首曲目",
    category: "playback",
    defaultAccelerator: "Ctrl+Alt+Left",
  },
  {
    action: "next_song",
    label: "下一首",
    description: "切换到队列下一首曲目",
    category: "playback",
    defaultAccelerator: "Ctrl+Alt+Right",
  },
  {
    action: "volume_up",
    label: "增大音量",
    description: "步进提升播放音量 (+5%)",
    category: "playback",
    defaultAccelerator: "Ctrl+Alt+Up",
  },
  {
    action: "volume_down",
    label: "降低音量",
    description: "步进降低播放音量 (-5%)",
    category: "playback",
    defaultAccelerator: "Ctrl+Alt+Down",
  },
  {
    action: "toggle_like",
    label: "喜欢 / 取消喜欢当前歌曲",
    description: "为正在播放的歌曲快捷点亮或取消红心",
    category: "playback",
    defaultAccelerator: "",
  },
  {
    action: "toggle_desktop_lyric",
    label: "显示 / 隐藏桌面歌词",
    description: "随时呼出或隐藏独立透明桌面悬浮歌词窗",
    category: "window",
    defaultAccelerator: "Ctrl+Alt+D",
  },
  {
    action: "toggle_lyric_lock",
    label: "锁定 / 解锁桌面歌词穿透",
    description: "切换桌面歌词鼠标穿透状态，锁定时按此键随时解锁",
    category: "window",
    defaultAccelerator: "Ctrl+Shift+L",
  },
  {
    action: "toggle_mini_mode",
    label: "切换 Mini 灵动岛小窗",
    description: "在主窗口与置顶透明 Mini 灵动岛之间快速切换",
    category: "window",
    defaultAccelerator: "Ctrl+Alt+M",
  },
];

export const IN_APP_SHORTCUT_ITEMS: ReadonlyArray<
  ShortcutMetaItem<InAppShortcutActionId>
> = [
  {
    action: "toggle_play",
    label: "播放 / 暂停",
    description: "窗口聚焦且非文本输入时快速切换播控状态",
    category: "playback",
    defaultAccelerator: "Space",
  },
  {
    action: "focus_search",
    label: "快速搜索",
    description: "跳转到搜索页并自动聚焦搜索输入框",
    category: "navigation",
    defaultAccelerator: "Ctrl+K",
  },
  {
    action: "prev_song",
    label: "上一首",
    description: "在窗口内快速切回上一首曲目",
    category: "playback",
    defaultAccelerator: "Ctrl+Left",
  },
  {
    action: "next_song",
    label: "下一首",
    description: "在窗口内快速切到下一首曲目",
    category: "playback",
    defaultAccelerator: "Ctrl+Right",
  },
  {
    action: "volume_up",
    label: "增大音量",
    description: "在窗口内微调提升音量 (+5%)",
    category: "playback",
    defaultAccelerator: "Ctrl+Up",
  },
  {
    action: "volume_down",
    label: "降低音量",
    description: "在窗口内微调降低音量 (-5%)",
    category: "playback",
    defaultAccelerator: "Ctrl+Down",
  },
  {
    action: "toggle_like",
    label: "喜欢当前曲目",
    description: "窗口内快捷收藏当前正在播放的歌曲",
    category: "playback",
    defaultAccelerator: "Ctrl+L",
  },
  {
    action: "toggle_lyric_lock",
    label: "锁定 / 解锁桌面歌词",
    description: "窗口内一键切换桌面歌词硬件鼠标穿透",
    category: "window",
    defaultAccelerator: "Ctrl+Shift+L",
  },
  {
    action: "toggle_mini_mode",
    label: "切换 Mini 灵动岛",
    description: "窗口内一键切换至 Mini 迷你悬浮窗",
    category: "window",
    defaultAccelerator: "Ctrl+Alt+M",
  },
  {
    action: "nav_back",
    label: "返回上一页",
    description: "沿页面导航历史栈回退（鼠标侧键同步支持）",
    category: "navigation",
    defaultAccelerator: "Alt+Left",
  },
];

export function createDefaultGlobalHotkeys(): Record<GlobalShortcutActionId, string> {
  const out = {} as Record<GlobalShortcutActionId, string>;
  for (const item of GLOBAL_SHORTCUT_ITEMS) {
    out[item.action] = item.defaultAccelerator;
  }
  return out;
}

export function createDefaultInAppHotkeys(): Record<InAppShortcutActionId, string> {
  const out = {} as Record<InAppShortcutActionId, string>;
  for (const item of IN_APP_SHORTCUT_ITEMS) {
    out[item.action] = item.defaultAccelerator;
  }
  return out;
}

export function toGlobalHotkeyBindingsList(
  map: Record<GlobalShortcutActionId, string>
): GlobalHotkeyBinding[] {
  return GLOBAL_SHORTCUT_ITEMS.map((item) => ({
    action: item.action,
    accelerator: (map[item.action] || "").trim(),
  }));
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "OS"]);

const CODE_TO_TOKEN: Record<string, string> = {
  Space: "Space",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  ArrowDown: "Down",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
  Delete: "Delete",
  Enter: "Enter",
  Tab: "Tab",
  Minus: "Minus",
  Equal: "Equal",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Backquote: "Backquote",
  NumpadAdd: "NumpadAdd",
  NumpadSubtract: "NumpadSubtract",
  NumpadMultiply: "NumpadMultiply",
  NumpadDivide: "NumpadDivide",
  NumpadDecimal: "NumpadDecimal",
};

export function extractPrimaryKeyToken(e: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(e.code) || MODIFIER_KEYS.has(e.key)) {
    return null;
  }
  const code = e.code || "";
  if (/^Key[A-Z]$/.test(code)) {
    return code.slice(3);
  }
  if (/^Digit[0-9]$/.test(code)) {
    return code.slice(5);
  }
  if (/^F([1-9]|1[0-2])$/.test(code)) {
    return code;
  }
  if (/^Numpad[0-9]$/.test(code)) {
    return code;
  }
  if (CODE_TO_TOKEN[code]) {
    return CODE_TO_TOKEN[code];
  }
  // 兜底按 e.key 映射
  const key = (e.key || "").trim();
  if (/^[a-zA-Z0-9]$/.test(key)) {
    return key.toUpperCase();
  }
  if (/^F([1-9]|1[0-2])$/i.test(key)) {
    return key.toUpperCase();
  }
  return null;
}

export function isFunctionKeyToken(token: string): boolean {
  return /^F([1-9]|1[0-2])$/i.test(token);
}

/**
 * 将录制态下的键盘事件转换为标准组合键字符串。
 * 返回 null 表示用户仅按下了修饰键，仍在等待主键输入。
 */
export function recordKeyboardEventToAccelerator(
  e: KeyboardEvent,
  options: { isGlobal: boolean }
): { ok: true; accelerator: string } | { ok: false; reason: string } | null {
  if (MODIFIER_CODES.has(e.code) || MODIFIER_KEYS.has(e.key)) {
    return null;
  }
  const primary = extractPrimaryKeyToken(e);
  if (!primary) {
    return { ok: false, reason: "不支持该按键，请使用字母、数字、方向键、空格或 F1–F12" };
  }

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Win");

  const hasModifier = parts.length > 0;
  if (options.isGlobal && !hasModifier && !isFunctionKeyToken(primary)) {
    return {
      ok: false,
      reason: "系统全局热键需配合 Ctrl / Alt / Shift 修饰键（或单独使用 F1–F12）",
    };
  }
  if (
    !options.isGlobal &&
    !hasModifier &&
    primary !== "Space" &&
    !isFunctionKeyToken(primary)
  ) {
    return {
      ok: false,
      reason: "为避免干扰文本输入，应用内快捷键建议配合修饰键（或使用 Space / F1–F12）",
    };
  }
  // 禁止将单独的 Ctrl+C / Ctrl+V / Ctrl+A / Ctrl+X / Alt+F4 等高危系统保留键绑定为全局热键
  const acc = [...parts, primary].join("+");
  const blockedGlobal = new Set(["Alt+F4", "Ctrl+Alt+Delete", "Win+L"]);
  if (options.isGlobal && blockedGlobal.has(acc)) {
    return { ok: false, reason: `${acc} 为 Windows 系统保留组合键，无法覆盖` };
  }

  return { ok: true, accelerator: acc };
}

export interface ParsedAccelerator {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  key: string;
}

export function parseAcceleratorString(acc: string): ParsedAccelerator | null {
  const trimmed = (acc || "").trim();
  if (!trimmed) return null;
  let ctrl = false;
  let alt = false;
  let shift = false;
  let meta = false;
  let key = "";

  for (const rawPart of trimmed.split("+")) {
    const token = rawPart.trim();
    if (!token) continue;
    const lower = token.toLowerCase();
    if (lower === "ctrl" || lower === "control") ctrl = true;
    else if (lower === "alt" || lower === "option") alt = true;
    else if (lower === "shift") shift = true;
    else if (lower === "win" || lower === "meta" || lower === "cmd" || lower === "super") {
      meta = true;
    } else {
      key = token;
    }
  }
  if (!key) return null;
  return { ctrl, alt, shift, meta, key };
}

export function matchKeyboardEventAccelerator(
  e: KeyboardEvent,
  accelerator: string
): boolean {
  const parsed = parseAcceleratorString(accelerator);
  if (!parsed) return false;
  const eventToken = extractPrimaryKeyToken(e);
  if (!eventToken) return false;

  const ctrlMatch = parsed.ctrl === (e.ctrlKey || (parsed.ctrl && !parsed.meta && e.metaKey));
  // 精准比对修饰键，避免 Ctrl+Alt+Left 误触 Ctrl+Left
  if (parsed.ctrl !== e.ctrlKey) return false;
  if (parsed.alt !== e.altKey) return false;
  if (parsed.shift !== e.shiftKey) return false;
  if (parsed.meta !== e.metaKey) return false;
  void ctrlMatch;

  return eventToken.toUpperCase() === parsed.key.toUpperCase();
}

const KEYCAP_DISPLAY_MAP: Record<string, string> = {
  Ctrl: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Win: "Win",
  Space: "Space",
  Left: "←",
  Right: "→",
  Up: "↑",
  Down: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  ArrowUp: "↑",
  ArrowDown: "↓",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
  PageUp: "PgUp",
  PageDown: "PgDn",
  Insert: "Ins",
  Delete: "Del",
  NumpadAdd: "Num +",
  NumpadSubtract: "Num -",
  NumpadMultiply: "Num *",
  NumpadDivide: "Num /",
  NumpadDecimal: "Num .",
};

export function formatAcceleratorKeycaps(accelerator: string): string[] {
  const trimmed = (accelerator || "").trim();
  if (!trimmed) return [];
  return trimmed
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean)
    .map((token) => {
      if (KEYCAP_DISPLAY_MAP[token]) return KEYCAP_DISPLAY_MAP[token];
      if (/^Numpad[0-9]$/.test(token)) return `Num ${token.slice(6)}`;
      return token.length === 1 ? token.toUpperCase() : token;
    });
}
