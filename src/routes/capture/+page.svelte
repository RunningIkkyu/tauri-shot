<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  type CapturePayload = {
    pngBase64: string;
    x: number;
    y: number;
    width: number;
    height: number;
    scaleFactor: number;
  };

  type Rect = { x: number; y: number; w: number; h: number };

  type Annotation =
    | { kind: "arrow"; x1: number; y1: number; x2: number; y2: number; color: string }
    | { kind: "rect"; x: number; y: number; w: number; h: number; color: string }
    | {
        kind: "ellipse";
        x1: number;
        y1: number;
        x2: number;
        y2: number;
        color: string;
      };

  type Tool = "arrow" | "rect" | "ellipse";

  let payload = $state<CapturePayload | null>(null);
  let phase = $state<"select" | "edit">("select");
  let imgUrl = $state<string | null>(null);

  let sel = $state<Rect | null>(null);
  let dragStart = $state<{ x: number; y: number } | null>(null);
  let dragCurrent = $state<{ x: number; y: number } | null>(null);

  let activeTool = $state<Tool | null>(null);
  let strokeColor = $state("#ef4444");
  let annotations = $state<Annotation[]>([]);

  let arrowDraft = $state<{ x1: number; y1: number; x2: number; y2: number } | null>(
    null,
  );
  let boxDraft = $state<{ x1: number; y1: number; x2: number; y2: number } | null>(
    null,
  );

  let exportFormat = $state<"png" | "jpeg" | "webp">("webp");

  let stageEl = $state<HTMLDivElement | null>(null);
  let imgEl = $state<HTMLImageElement | null>(null);
  let annotateCanvas = $state<HTMLCanvasElement | null>(null);

  function maskRects(s: Rect, W: number, H: number) {
    return {
      top: { left: 0, top: 0, width: W, height: s.y },
      bottom: {
        left: 0,
        top: s.y + s.h,
        width: W,
        height: Math.max(0, H - s.y - s.h),
      },
      left: { left: 0, top: s.y, width: s.x, height: s.h },
      right: {
        left: s.x + s.w,
        top: s.y,
        width: Math.max(0, W - s.x - s.w),
        height: s.h,
      },
    };
  }

  function previewRect(): Rect | null {
    if (!dragStart || !dragCurrent) return null;
    const x = Math.min(dragStart.x, dragCurrent.x);
    const y = Math.min(dragStart.y, dragCurrent.y);
    const w = Math.abs(dragCurrent.x - dragStart.x);
    const h = Math.abs(dragCurrent.y - dragStart.y);
    if (w < 2 || h < 2) return null;
    return { x, y, w, h };
  }

  function insideSel(selR: Rect, x: number, y: number) {
    return x >= selR.x && x <= selR.x + selR.w && y >= selR.y && y <= selR.y + selR.h;
  }

  async function closeWin() {
    try {
      await invoke("close_capture_window");
    } catch {
      /* */
    }
    try {
      await getCurrentWindow().close();
    } catch {
      /* */
    }
  }

  function mapSettingsFormat(s: string): "png" | "jpeg" | "webp" {
    const v = s.toLowerCase();
    if (v === "jpeg" || v === "jpg") return "jpeg";
    if (v === "webp") return "webp";
    return "png";
  }

  onMount(() => {
    void (async () => {
      const p = await invoke<CapturePayload | null>("take_pending_capture");
      if (!p) {
        await closeWin();
        return;
      }
      payload = p;
      imgUrl = `data:image/png;base64,${p.pngBase64}`;
      try {
        const s = await invoke<{ defaultExportFormat: string }>("get_settings");
        exportFormat = mapSettingsFormat(s.defaultExportFormat);
      } catch {
        /* */
      }
      await new Promise<void>((resolve) => {
        if (imgEl && imgEl.complete && imgEl.naturalWidth > 0) {
          requestAnimationFrame(() => resolve());
        } else {
          const onLoad = () => {
            imgEl?.removeEventListener("load", onLoad);
            requestAnimationFrame(() => resolve());
          };
          imgEl?.addEventListener("load", onLoad);
          setTimeout(resolve, 800);
        }
      });
      try {
        await invoke("show_capture_window");
      } catch {
        /* */
      }
    })();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void closeWin();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function onSelectPointerDown(e: PointerEvent) {
    if (phase !== "select" || !stageEl) return;
    const r = stageEl.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    dragStart = { x, y };
    dragCurrent = { x, y };
    stageEl.setPointerCapture(e.pointerId);
  }

  function onSelectPointerMove(e: PointerEvent) {
    if (phase !== "select" || !dragStart || !stageEl) return;
    const r = stageEl.getBoundingClientRect();
    dragCurrent = { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  function onSelectPointerUp(e: PointerEvent) {
    if (phase !== "select" || !stageEl) return;
    stageEl.releasePointerCapture(e.pointerId);
    if (!dragStart || !dragCurrent) {
      dragStart = null;
      dragCurrent = null;
      return;
    }
    const x = Math.min(dragStart.x, dragCurrent.x);
    const y = Math.min(dragStart.y, dragCurrent.y);
    const w = Math.abs(dragCurrent.x - dragStart.x);
    const h = Math.abs(dragCurrent.y - dragStart.y);
    dragStart = null;
    dragCurrent = null;
    if (w < 4 || h < 4) return;
    sel = { x, y, w, h };
    phase = "edit";
    queueMicrotask(() => redrawAnnotations());
  }

  function drawArrow(
    ctx: CanvasRenderingContext2D,
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    head: number,
    color: string,
  ) {
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len = Math.hypot(dx, dy) || 1;
    const ux = dx / len;
    const uy = dy / len;
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = 3;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2 - ux * head * 0.6, y2 - uy * head * 0.6);
    ctx.stroke();
    const bx = x2 - ux * head;
    const by = y2 - uy * head;
    const px = -uy;
    const py = ux;
    ctx.beginPath();
    ctx.moveTo(x2, y2);
    ctx.lineTo(bx + px * (head / 2), by + py * (head / 2));
    ctx.lineTo(bx - px * (head / 2), by - py * (head / 2));
    ctx.closePath();
    ctx.fill();
  }

  function drawStrokeEllipse(
    ctx: CanvasRenderingContext2D,
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    color: string,
    lineWidth = 3,
  ) {
    const left = Math.min(x1, x2);
    const top = Math.min(y1, y2);
    const rw = Math.abs(x2 - x1) / 2;
    const rh = Math.abs(y2 - y1) / 2;
    const cx = left + rw;
    const cy = top + rh;
    ctx.strokeStyle = color;
    ctx.lineWidth = lineWidth;
    ctx.beginPath();
    ctx.ellipse(cx, cy, Math.max(rw, 0.5), Math.max(rh, 0.5), 0, 0, 2 * Math.PI);
    ctx.stroke();
  }

  function resizeAnnotateCanvas() {
    const c = annotateCanvas;
    if (!c || !sel) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(sel.w * dpr);
    const h = Math.round(sel.h * dpr);
    if (c.width !== w) c.width = w;
    if (c.height !== h) c.height = h;
    c.style.width = `${sel.w}px`;
    c.style.height = `${sel.h}px`;
  }

  function redrawAnnotations() {
    const c = annotateCanvas;
    if (!c || !sel) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    resizeAnnotateCanvas();
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, sel.w, sel.h);
    const headLen = Math.max(14, Math.min(sel.w, sel.h) * 0.04);

    for (const ann of annotations) {
      if (ann.kind === "arrow") {
        drawArrow(
          ctx,
          ann.x1 - sel.x,
          ann.y1 - sel.y,
          ann.x2 - sel.x,
          ann.y2 - sel.y,
          headLen,
          ann.color,
        );
      } else if (ann.kind === "rect") {
        ctx.strokeStyle = ann.color;
        ctx.lineWidth = 3;
        ctx.strokeRect(ann.x - sel.x, ann.y - sel.y, ann.w, ann.h);
      } else if (ann.kind === "ellipse") {
        drawStrokeEllipse(
          ctx,
          ann.x1 - sel.x,
          ann.y1 - sel.y,
          ann.x2 - sel.x,
          ann.y2 - sel.y,
          ann.color,
        );
      }
    }

    if (arrowDraft) {
      drawArrow(
        ctx,
        arrowDraft.x1 - sel.x,
        arrowDraft.y1 - sel.y,
        arrowDraft.x2 - sel.x,
        arrowDraft.y2 - sel.y,
        headLen,
        strokeColor,
      );
    }
    if (boxDraft && activeTool === "rect") {
      const x = Math.min(boxDraft.x1, boxDraft.x2) - sel.x;
      const y = Math.min(boxDraft.y1, boxDraft.y2) - sel.y;
      const w = Math.abs(boxDraft.x2 - boxDraft.x1);
      const h = Math.abs(boxDraft.y2 - boxDraft.y1);
      ctx.strokeStyle = strokeColor;
      ctx.lineWidth = 3;
      ctx.strokeRect(x, y, w, h);
    }
    if (boxDraft && activeTool === "ellipse") {
      drawStrokeEllipse(
        ctx,
        boxDraft.x1 - sel.x,
        boxDraft.y1 - sel.y,
        boxDraft.x2 - sel.x,
        boxDraft.y2 - sel.y,
        strokeColor,
      );
    }
  }

  $effect(() => {
    void annotations;
    void arrowDraft;
    void boxDraft;
    void sel;
    void strokeColor;
    void activeTool;
    if (annotateCanvas && phase === "edit") redrawAnnotations();
  });

  function onEditPointerDown(e: PointerEvent) {
    if (phase !== "edit" || !stageEl || !sel || !activeTool) return;
    const r = stageEl.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    if (!insideSel(sel, x, y)) return;
    if (activeTool === "arrow") {
      arrowDraft = { x1: x, y1: y, x2: x, y2: y };
      stageEl.setPointerCapture(e.pointerId);
    } else {
      boxDraft = { x1: x, y1: y, x2: x, y2: y };
      stageEl.setPointerCapture(e.pointerId);
    }
  }

  function onEditPointerMove(e: PointerEvent) {
    if (!stageEl) return;
    if (arrowDraft) {
      arrowDraft = {
        ...arrowDraft,
        x2: e.clientX - stageEl.getBoundingClientRect().left,
        y2: e.clientY - stageEl.getBoundingClientRect().top,
      };
      return;
    }
    if (boxDraft) {
      boxDraft = {
        ...boxDraft,
        x2: e.clientX - stageEl.getBoundingClientRect().left,
        y2: e.clientY - stageEl.getBoundingClientRect().top,
      };
    }
  }

  function onEditPointerUp(e: PointerEvent) {
    if (!stageEl) return;
    try {
      stageEl.releasePointerCapture(e.pointerId);
    } catch {
      /* */
    }

    if (arrowDraft && activeTool === "arrow") {
      const { x1, y1, x2, y2 } = arrowDraft;
      arrowDraft = null;
      if (Math.hypot(x2 - x1, y2 - y1) > 5) {
        annotations = [...annotations, { kind: "arrow", x1, y1, x2, y2, color: strokeColor }];
      }
      return;
    }

    if (boxDraft && activeTool === "rect") {
      const x = Math.min(boxDraft.x1, boxDraft.x2);
      const y = Math.min(boxDraft.y1, boxDraft.y2);
      const w = Math.abs(boxDraft.x2 - boxDraft.x1);
      const h = Math.abs(boxDraft.y2 - boxDraft.y1);
      boxDraft = null;
      if (w > 5 && h > 5) {
        annotations = [...annotations, { kind: "rect", x, y, w, h, color: strokeColor }];
      }
      return;
    }

    if (boxDraft && activeTool === "ellipse") {
      const { x1, y1, x2, y2 } = boxDraft;
      boxDraft = null;
      if (Math.abs(x2 - x1) > 5 && Math.abs(y2 - y1) > 5) {
        annotations = [...annotations, { kind: "ellipse", x1, y1, x2, y2, color: strokeColor }];
      }
    }
  }

  async function renderExportBase64(): Promise<string | null> {
    if (!imgEl || !sel || !imgEl.complete || imgEl.naturalWidth === 0) return null;
    const scaleX = imgEl.naturalWidth / imgEl.offsetWidth;
    const scaleY = imgEl.naturalHeight / imgEl.offsetHeight;
    const sx = Math.round(sel.x * scaleX);
    const sy = Math.round(sel.y * scaleY);
    const sw = Math.round(sel.w * scaleX);
    const sh = Math.round(sel.h * scaleY);
    const canvas = document.createElement("canvas");
    canvas.width = sw;
    canvas.height = sh;
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    ctx.drawImage(imgEl, sx, sy, sw, sh, 0, 0, sw, sh);
    const head = Math.max(12, Math.round(Math.min(sw, sh) * 0.04));

    for (const ann of annotations) {
      if (ann.kind === "arrow") {
        const ax1 = ann.x1 * scaleX - sx;
        const ay1 = ann.y1 * scaleY - sy;
        const ax2 = ann.x2 * scaleX - sx;
        const ay2 = ann.y2 * scaleY - sy;
        drawArrow(ctx, ax1, ay1, ax2, ay2, head, ann.color);
      } else if (ann.kind === "rect") {
        ctx.strokeStyle = ann.color;
        ctx.lineWidth = Math.max(2, Math.round(3 * ((scaleX + scaleY) / 2)));
        ctx.strokeRect(
          ann.x * scaleX - sx,
          ann.y * scaleY - sy,
          ann.w * scaleX,
          ann.h * scaleY,
        );
      } else if (ann.kind === "ellipse") {
        const lw = Math.max(2, Math.round(3 * ((scaleX + scaleY) / 2)));
        drawStrokeEllipse(
          ctx,
          ann.x1 * scaleX - sx,
          ann.y1 * scaleY - sy,
          ann.x2 * scaleX - sx,
          ann.y2 * scaleY - sy,
          ann.color,
          lw,
        );
      }
    }

    let dataUrl: string;
    if (exportFormat === "png") {
      dataUrl = canvas.toDataURL("image/png");
    } else if (exportFormat === "jpeg") {
      dataUrl = canvas.toDataURL("image/jpeg", 0.92);
    } else {
      dataUrl = canvas.toDataURL("image/webp", 0.92);
    }
    const i = dataUrl.indexOf(",");
    return i >= 0 ? dataUrl.slice(i + 1) : null;
  }

  function extForSave(): string {
    if (exportFormat === "jpeg") return "jpg";
    if (exportFormat === "webp") return "webp";
    return "png";
  }

  function formatForRust(): string {
    if (exportFormat === "jpeg") return "jpeg";
    if (exportFormat === "webp") return "webp";
    return "png";
  }

  async function onSaveToFile() {
    const b64 = await renderExportBase64();
    if (!b64) return;
    const ext = extForSave();
    const path = await save({
      defaultPath: `Screenshot-${Date.now()}.${ext}`,
      filters: [
        {
          name: "Image",
          extensions: [ext],
        },
      ],
    });
    if (!path) return;
    await invoke("write_image_file", {
      path,
      format: formatForRust(),
      imageBase64: b64,
    });
  }

  async function onFinish() {
    const b64 = await renderExportBase64();
    if (!b64) return;
    await invoke("copy_image_to_clipboard", { imageBase64: b64 });
    await closeWin();
  }

  function toggleTool(t: Tool) {
    activeTool = activeTool === t ? null : t;
    arrowDraft = null;
    boxDraft = null;
  }

  let stageW = $derived(payload ? payload.width : 0);
  let stageH = $derived(payload ? payload.height : 0);
  let masks = $derived(sel ? maskRects(sel, stageW, stageH) : null);
  let dragPreview = $derived(phase === "select" ? previewRect() : null);
  let selectMasks = $derived(
    phase === "select" && dragPreview ? maskRects(dragPreview, stageW, stageH) : null,
  );

  let crosshairCursor = $derived(
    phase === "select" ||
      (phase === "edit" && activeTool !== null),
  );

  const TOOLBAR_H = 56;
  const TOOLBAR_GAP = 12;
  const TOOLBAR_MIN_W = 380;

  function toolbarLeft(selR: Rect) {
    const half = TOOLBAR_MIN_W / 2;
    const cx = selR.x + selR.w / 2;
    return Math.min(Math.max(cx, half + 8), stageW - half - 8);
  }

  function toolbarTop(selR: Rect) {
    const below = selR.y + selR.h + TOOLBAR_GAP;
    if (below + TOOLBAR_H + 8 <= stageH) return below;
    const above = selR.y - TOOLBAR_GAP - TOOLBAR_H;
    if (above >= 8) return above;
    return Math.max(8, stageH - TOOLBAR_H - 8);
  }
</script>

{#if payload && imgUrl}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="stage"
    class:crosshair={crosshairCursor}
    bind:this={stageEl}
    style:width="{stageW}px"
    style:height="{stageH}px"
    onpointerdown={phase === "select"
      ? onSelectPointerDown
      : phase === "edit"
        ? onEditPointerDown
        : undefined}
    onpointermove={phase === "select"
      ? onSelectPointerMove
      : phase === "edit" && activeTool
        ? onEditPointerMove
        : undefined}
    onpointerup={phase === "select"
      ? onSelectPointerUp
      : phase === "edit" && activeTool
        ? onEditPointerUp
        : undefined}
    role="application"
    tabindex="-1"
  >
    <img bind:this={imgEl} class="shot" src={imgUrl} alt="" draggable="false" />

    {#if phase === "edit" && sel && masks}
      <div
        class="dim"
        style:left="{masks.top.left}px"
        style:top="{masks.top.top}px"
        style:width="{masks.top.width}px"
        style:height="{masks.top.height}px"
      ></div>
      <div
        class="dim"
        style:left="{masks.bottom.left}px"
        style:top="{masks.bottom.top}px"
        style:width="{masks.bottom.width}px"
        style:height="{masks.bottom.height}px"
      ></div>
      <div
        class="dim"
        style:left="{masks.left.left}px"
        style:top="{masks.left.top}px"
        style:width="{masks.left.width}px"
        style:height="{masks.left.height}px"
      ></div>
      <div
        class="dim"
        style:left="{masks.right.left}px"
        style:top="{masks.right.top}px"
        style:width="{masks.right.width}px"
        style:height="{masks.right.height}px"
      ></div>
      <div
        class="sel-outline"
        style:left="{sel.x}px"
        style:top="{sel.y}px"
        style:width="{sel.w}px"
        style:height="{sel.h}px"
      ></div>
      <canvas
        bind:this={annotateCanvas}
        class="annotate"
        style:left="{sel.x}px"
        style:top="{sel.y}px"
        style:width="{sel.w}px"
        style:height="{sel.h}px"
      ></canvas>
    {/if}

    {#if phase === "select" && !dragPreview}
      <div class="dim full-dim"></div>
      <div class="hint">Drag to select region, Esc to cancel</div>
    {/if}

    {#if phase === "select" && dragPreview && selectMasks}
      <div
        class="dim"
        style:left="{selectMasks.top.left}px"
        style:top="{selectMasks.top.top}px"
        style:width="{selectMasks.top.width}px"
        style:height="{selectMasks.top.height}px"
      ></div>
      <div
        class="dim"
        style:left="{selectMasks.bottom.left}px"
        style:top="{selectMasks.bottom.top}px"
        style:width="{selectMasks.bottom.width}px"
        style:height="{selectMasks.bottom.height}px"
      ></div>
      <div
        class="dim"
        style:left="{selectMasks.left.left}px"
        style:top="{selectMasks.left.top}px"
        style:width="{selectMasks.left.width}px"
        style:height="{selectMasks.left.height}px"
      ></div>
      <div
        class="dim"
        style:left="{selectMasks.right.left}px"
        style:top="{selectMasks.right.top}px"
        style:width="{selectMasks.right.width}px"
        style:height="{selectMasks.right.height}px"
      ></div>
      <div
        class="marquee"
        style:left="{dragPreview.x}px"
        style:top="{dragPreview.y}px"
        style:width="{dragPreview.w}px"
        style:height="{dragPreview.h}px"
      ></div>
    {/if}

    {#if phase === "edit" && sel}
      <div
        class="toolbar"
        style:left="{toolbarLeft(sel)}px"
        style:top="{toolbarTop(sel)}px"
        onclick={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        role="toolbar"
      >
        <button
          type="button"
          class="tb-icon"
          class:tool-on={activeTool === "arrow"}
          onclick={() => toggleTool("arrow")}
          title="Arrow"
          aria-label="Arrow"
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="5" y1="18" x2="17" y2="6" stroke-linecap="round" />
            <polygon points="17,6 13,6.5 15.5,9" fill="currentColor" stroke="none" />
          </svg>
        </button>
        <button
          type="button"
          class="tb-icon"
          class:tool-on={activeTool === "rect"}
          onclick={() => toggleTool("rect")}
          title="Rectangle"
          aria-label="Rectangle"
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="4" y="5" width="16" height="14" rx="1" />
          </svg>
        </button>
        <button
          type="button"
          class="tb-icon"
          class:tool-on={activeTool === "ellipse"}
          onclick={() => toggleTool("ellipse")}
          title="Ellipse"
          aria-label="Ellipse"
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <ellipse cx="12" cy="12" rx="8" ry="6" />
          </svg>
        </button>

        <label class="color-swatch" title="Stroke color">
          <input type="color" bind:value={strokeColor} class="color-input" aria-label="Stroke color" />
          <span class="palette-icon" aria-hidden="true">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
              <path
                d="M12 3c-4.42 0-8 3.13-8 7 0 2.38 1.19 4.47 3 5.74V21h10v-2.26c1.81-1.27 3-3.36 3-5.74 0-3.87-3.58-7-8-7zm-1.31 10.73c-.41.33-.91.54-1.44.61.12-.81.63-1.5 1.36-1.86a.96.96 0 00-.22-1.79c-1.01.31-1.78 1.12-2.06 2.13-.52-.1-1-.39-1.36-.81.45-1.06 1.35-1.87 2.45-2.09.37.55.98.91 1.67.91.28 0 .55-.06.79-.17.15.53.23 1.09.23 1.67 0 .45-.06.88-.17 1.29-.19.16-.41.29-.65.39.15.18.34.33.56.43zM12 5.5c.83 0 1.5.67 1.5 1.5S12.83 8.5 12 8.5 10.5 7.83 10.5 7 11.17 5.5 12 5.5z"
              />
            </svg>
          </span>
        </label>

        <span class="format-wrap" title="Export format">
          <span class="format-label">Format</span>
          <select bind:value={exportFormat} class="tb-select">
            <option value="png">PNG</option>
            <option value="jpeg">JPG</option>
            <option value="webp">WebP</option>
          </select>
        </span>

        <button
          type="button"
          class="tb-icon"
          onclick={() => void onSaveToFile()}
          title="Save to file"
          aria-label="Save to file"
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z" />
            <polyline points="17 21 17 13 7 13 7 21" />
            <polyline points="7 3 7 8 15 8" />
          </svg>
        </button>

        <button
          type="button"
          class="tb-icon tb-done"
          onclick={() => void onFinish()}
          title="Finish and copy to clipboard"
          aria-label="Finish and copy to clipboard"
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <polyline points="20 6 9 17 4 12" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .stage {
    position: relative;
    overflow: hidden;
    background: #000;
    user-select: none;
    touch-action: none;
  }

  .stage.crosshair {
    cursor: crosshair;
  }

  .shot {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: fill;
    pointer-events: none;
  }

  .dim {
    position: absolute;
    background: rgba(0, 0, 0, 0.45);
    pointer-events: none;
    z-index: 1;
  }

  .full-dim {
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .hint {
    position: absolute;
    left: 50%;
    top: 24px;
    transform: translateX(-50%);
    padding: 8px 14px;
    border-radius: 999px;
    background: rgba(26, 29, 36, 0.85);
    border: 1px solid #3c4043;
    color: #e8eaed;
    font-size: 0.85rem;
    pointer-events: none;
    z-index: 5;
  }

  .sel-outline {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid #8ab4f8;
    pointer-events: none;
    z-index: 2;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.35);
  }

  .annotate {
    position: absolute;
    pointer-events: none;
    z-index: 3;
  }

  .marquee {
    position: absolute;
    box-sizing: border-box;
    border: 2px dashed #8ab4f8;
    background: rgba(138, 180, 248, 0.12);
    pointer-events: none;
    z-index: 4;
  }

  .toolbar {
    position: absolute;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 12px;
    background: rgba(26, 29, 36, 0.94);
    border: 1px solid #3c4043;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.45);
    z-index: 20;
    pointer-events: auto;
  }

  .tb-icon {
    width: 40px;
    height: 40px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 10px;
    border: 1px solid #5f6368;
    background: #30343c;
    color: #e8eaed;
    cursor: pointer;
  }

  .tb-icon:hover {
    background: #3d424d;
  }

  .tb-icon.tool-on {
    border-color: #8ab4f8;
    background: #394457;
    color: #aac7fa;
  }

  .tb-done {
    border: none;
    background: linear-gradient(180deg, #81c995, #5bb974);
    color: #0f1115;
  }

  .tb-done:hover {
    filter: brightness(1.06);
  }

  .color-swatch {
    position: relative;
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 10px;
    border: 1px solid #5f6368;
    background: #30343c;
    cursor: pointer;
    overflow: hidden;
  }

  .color-input {
    position: absolute;
    inset: 0;
    opacity: 0;
    width: 100%;
    height: 100%;
    cursor: pointer;
    border: none;
    padding: 0;
  }

  .palette-icon {
    color: #e8eaed;
    pointer-events: none;
    display: flex;
  }

  .format-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 4px;
  }

  .format-label {
    font-size: 0.75rem;
    color: #9aa0a6;
    white-space: nowrap;
  }

  .tb-select {
    font-size: 0.8125rem;
    border-radius: 8px;
    border: 1px solid #5f6368;
    background: #30343c;
    color: #e8eaed;
    padding: 0.35rem 0.5rem;
    cursor: pointer;
    min-width: 5.5rem;
  }
</style>
