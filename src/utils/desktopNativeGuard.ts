/**
 * 桌面原生客户端防 Web 泄漏守卫
 * - 拦截空白区 / 非交互区的浏览器默认右键菜单；输入框区域转交自定义主题编辑菜单
 * - 拦截浏览器专属快捷键（刷新/打印/保存/查看源码/缩放/查找/发行版开发者工具），并将 Ctrl+F 重定向至当前视图筛选框
 * - 屏蔽 Ctrl+滚轮网页缩放、外部文件拖拽整页跳转、以及图片/链接原生幽灵拖拽
 */

export interface InputContextMenuDetail {
  x: number;
  y: number;
  target: HTMLInputElement | HTMLTextAreaElement | HTMLElement;
}

export const INPUT_CONTEXT_MENU_EVENT = "breeze:input-contextmenu";

export function isEditableElement(el: EventTarget | null): el is HTMLInputElement | HTMLTextAreaElement | HTMLElement {
  if (!(el instanceof HTMLElement)) return false;
  if (el instanceof HTMLTextAreaElement) return !el.disabled;
  if (el instanceof HTMLInputElement) {
    const type = (el.type || "text").toLowerCase();
    const textLike = new Set([
      "text",
      "search",
      "password",
      "email",
      "url",
      "tel",
      "number",
    ]);
    return textLike.has(type) && !el.disabled;
  }
  return Boolean(el.isContentEditable);
}

function focusNearestVisibleSearchInput(): boolean {
  const selectors = [
    ".lmv-search-input",
    ".cdv-search-input",
    ".pl-search-input",
    ".hero-filter-input",
    "#independent-search-input",
    ".rom-search-input",
    'input[type="search"]',
    'input[placeholder*="搜索"]',
    'input[placeholder*="筛选"]',
    'input[placeholder*="检索"]',
  ];
  for (const sel of selectors) {
    const nodes = document.querySelectorAll<HTMLInputElement>(sel);
    for (const el of Array.from(nodes)) {
      if (el.disabled || el.offsetParent === null) continue;
      const rect = el.getBoundingClientRect();
      if (rect.width > 20 && rect.height > 12) {
        el.focus();
        el.select();
        return true;
      }
    }
  }
  return false;
}

export function installDesktopNativeGuard(options?: {
  allowCustomInputMenu?: boolean;
}): () => void {
  const allowCustomInputMenu = options?.allowCustomInputMenu ?? true;
  const isProd = Boolean(import.meta.env?.PROD);

  const onContextMenuCapture = (e: MouseEvent) => {
    // 捕获阶段优先接管可编辑输入框，防止父级容器 @contextmenu.prevent 提前吞掉输入框的右键编辑菜单
    const target = e.target as HTMLElement | null;
    if (target && isEditableElement(target)) {
      e.preventDefault();
      e.stopPropagation();
      if (allowCustomInputMenu) {
        window.dispatchEvent(
          new CustomEvent<InputContextMenuDetail>(INPUT_CONTEXT_MENU_EVENT, {
            detail: { x: e.clientX, y: e.clientY, target },
          })
        );
      }
    }
  };

  const onContextMenuBubble = (e: MouseEvent) => {
    // 若具体业务组件（如歌曲行、歌单卡）已经消费并打开自定义右键菜单，直接放行
    if (e.defaultPrevented) return;
    // 其余所有非编辑区一律拦截浏览器默认右键菜单
    e.preventDefault();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const key = e.key.toLowerCase();
    const ctrlOrMeta = e.ctrlKey || e.metaKey;

    // 1. 拦截 F5 / Ctrl+R / Ctrl+Shift+R 刷新
    if (e.key === "F5" || (ctrlOrMeta && key === "r")) {
      e.preventDefault();
      return;
    }

    // 2. 拦截 Ctrl+P (打印) / Ctrl+S (另存为) / Ctrl+U (源码) / Ctrl+G / F3 (浏览器查找) / Ctrl+J (下载) / Ctrl+H (历史)
    if (
      e.key === "F3" ||
      (ctrlOrMeta && !e.altKey && (key === "p" || key === "s" || key === "u" || key === "g" || key === "j" || key === "h"))
    ) {
      e.preventDefault();
      return;
    }

    // 3. Ctrl+F：拦截 WebView 查找栏，优雅聚焦当前页面内建筛选/搜索输入框
    if (ctrlOrMeta && !e.shiftKey && !e.altKey && key === "f") {
      e.preventDefault();
      focusNearestVisibleSearchInput();
      return;
    }

    // 4. 拦截网页缩放快捷键 (Ctrl + / - / = / 0)
    if (
      ctrlOrMeta &&
      !e.altKey &&
      (key === "+" || key === "-" || key === "=" || key === "0" || e.code === "NumpadAdd" || e.code === "NumpadSubtract" || e.code === "Numpad0")
    ) {
      e.preventDefault();
      return;
    }

    // 5. 拦截 Alt+Right 浏览器前进（Alt+Left 由应用内返回历史接管）
    if (e.altKey && e.key === "ArrowRight") {
      e.preventDefault();
      return;
    }

    // 6. 发行版屏蔽浏览器开发者工具快捷键 (F12 / Ctrl+Shift+I / Ctrl+Shift+C / Ctrl+Shift+J)
    if (
      isProd &&
      (e.key === "F12" ||
        (ctrlOrMeta && e.shiftKey && (key === "i" || key === "c" || key === "j")))
    ) {
      e.preventDefault();
    }
  };

  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey) {
      e.preventDefault();
    }
  };

  const onDragStart = (e: DragEvent) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.tagName === "IMG" || target.tagName === "A") {
      e.preventDefault();
    }
  };

  const onDragOver = (e: DragEvent) => {
    if (e.defaultPrevented) return;
    const types = e.dataTransfer?.types;
    if (types && (Array.from(types).includes("Files") || Array.from(types).includes("text/uri-list"))) {
      e.preventDefault();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = "none";
      }
    }
  };

  const onDrop = (e: DragEvent) => {
    if (e.defaultPrevented) return;
    const types = e.dataTransfer?.types;
    if (types && (Array.from(types).includes("Files") || Array.from(types).includes("text/uri-list"))) {
      e.preventDefault();
    }
  };

  document.addEventListener("contextmenu", onContextMenuCapture, true);
  document.addEventListener("contextmenu", onContextMenuBubble, false);
  window.addEventListener("keydown", onKeyDown, true);
  window.addEventListener("wheel", onWheel, { passive: false, capture: true });
  document.addEventListener("dragstart", onDragStart, true);
  window.addEventListener("dragover", onDragOver, false);
  window.addEventListener("drop", onDrop, false);

  return () => {
    document.removeEventListener("contextmenu", onContextMenuCapture, true);
    document.removeEventListener("contextmenu", onContextMenuBubble, false);
    window.removeEventListener("keydown", onKeyDown, true);
    window.removeEventListener("wheel", onWheel, true);
    document.removeEventListener("dragstart", onDragStart, true);
    window.removeEventListener("dragover", onDragOver, false);
    window.removeEventListener("drop", onDrop, false);
  };
}
