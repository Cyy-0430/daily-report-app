<script lang="ts">
  import ColorPicker from 'svelte-awesome-color-picker';
  import { hexToRgb, rgbToHex } from '$lib/theme';

  /**
   * 通用调色盘(主题 tab 使用,不依赖设置页样式)。
   * 内部为 svelte-awesome-color-picker v4(Svelte 5 重写版)的包装层:
   * 色块按钮 + 弹层(SV 面 / 色相条 / HEX·RGB 输入 / 键盘导航 / 外点·Esc 关闭)由库提供。
   * 对外契约:hex 进(value)、hex 出(onchange),不用 alpha 通道 ——
   * 库输出的 hex 可能携带 8 位 alpha 段(如手输 8 位 HEX),统一归一为 #rrggbb 再对外。
   */
  let {
    value,
    onchange,
    label = '打开调色盘',
  }: {
    /** 当前色("#rrggbb");父级(draft)持有,经 onchange 提交。 */
    value: string;
    onchange: (hex: string) => void;
    /** a11y:色块按钮的语义名(如「调整页面背景颜色」)。 */
    label?: string;
  } = $props();

  // 库绑定值(受控):初始 = value;外部 value 变化(单项重置/主题切换)经 $effect 推进库组件。
  // svelte-ignore state_referenced_locally(有意仅捕获初始值,后续变化走下方 $effect)
  let hex = $state(value);
  // 最近一次对外发出(或外部推入)的归一值,阻断「库归一化回写 → 再对外」的回环。
  // 非响应式:仅在 effect/handler 内部读写,无需触发渲染。
  // svelte-ignore state_referenced_locally(同上,有意仅捕获初始值)
  let lastOut = value;

  /** 库输出归一为 "#rrggbb":剥 8 位 alpha 段、3 位展开、小写化;非法返回 null(不对外)。 */
  function normalizeHex(raw: string | null): string | null {
    if (!raw) return null;
    const rgb = hexToRgb(raw) ?? hexToRgb(raw.slice(0, 7));
    return rgb ? rgbToHex(rgb.r, rgb.g, rgb.b) : null;
  }

  $effect(() => {
    if (value !== lastOut) {
      lastOut = value;
      hex = value;
    }
  });

  function handleInput(e: { hex: string | null }) {
    const n = normalizeHex(e.hex);
    if (n && n !== lastOut) {
      lastOut = n;
      onchange(n);
    }
  }

  // 输入区只保留 HEX / RGB 两类手动输入(PRD R2 范围);文案与 aria 标签中文化(部分覆盖,与库默认合并)。
  const texts = {
    label: { hex: 'HEX 颜色值', r: '红色通道', g: '绿色通道', b: '蓝色通道' },
    color: { hex: 'HEX', rgb: 'RGB' },
    changeTo: '切换为 ',
  };
</script>

<div class="picker-host">
  <ColorPicker
    bind:hex
    {label}
    {texts}
    isAlpha={false}
    sliderDirection="horizontal"
    textInputModes={['hex', 'rgb']}
    onInput={handleInput}
  />
</div>

<style>
  .picker-host {
    display: inline-block;

    /* ---- 库主题变量(--cp-*)→ Editorial Paper 令牌:引用 var(),令牌运行时可变,调色盘自动跟随主题 ---- */
    --cp-bg-color: var(--paper-card);
    --cp-border-color: var(--line);
    --cp-text-color: var(--ink);
    --cp-input-color: var(--paper);
    --cp-button-hover-color: var(--line);
    --focus-color: var(--accent);
    --picker-z-index: 40; /* 与原弹层一致:压过设置页相邻卡片 */

    /* ---- 尺寸:对齐原自研弹层(总宽约 232px) ---- */
    --input-size: 30px; /* 原色块 30px */
    --picker-width: 214px; /* 214 + 8*2 padding + 2 border ≈ 232px */
    --picker-height: 132px; /* 原 SV 面高 */
    --picker-radius: 7px; /* 原 SV 圆角 */
    --picker-indicator-size: 14px; /* 原拖点直径 */
    --slider-width: 14px; /* 原色相条高 */
  }

  /* 触发器:库会把 label 渲染为可见文本 → 字号清零隐藏(保留为 a11y 名、零占位);
     并去掉库默认 4px 外距,避免挤动 ThemeTab 的 var-row 对齐。 */
  .picker-host :global(label) {
    margin: 0;
    gap: 0;
    font-size: 0;
  }

  /* 色块:库默认圆形 → 原样式的圆角方 + 纸感描边;无 alpha,棋盘格底隐藏。 */
  .picker-host :global(.alpha) {
    display: none;
  }
  .picker-host :global(.color) {
    border-radius: 7px;
    border: 1px solid var(--line-strong);
    box-shadow: inset 0 0 0 2px var(--paper-card);
    transition:
      border-color 0.15s,
      box-shadow 0.15s;
  }
  .picker-host :global(label:hover .color) {
    border-color: var(--ink-soft);
    box-shadow:
      inset 0 0 0 2px var(--paper-card),
      0 0 0 3px rgba(31, 28, 24, 0.08);
  }

  /* 竖向堆叠:库把 SV 面(.picker:inline-block)与色相条(.h:inline-flex)按 inline 流排在同一行
     (horizontal 模式下两者并排、弹层被撑成约 450px 宽)→ 改为块级,色相条落到 SV 面正下方。 */
  .picker-host :global(div.picker) {
    display: block;
  }
  .picker-host :global(div.h) {
    display: flex;
  }

  /* 弹层:去掉库默认左移 10px 的 margin(对齐色块左缘);阴影/圆角对齐原样式。 */
  .picker-host :global(div.wrapper) {
    margin: 0;
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.14);
  }

  /* 输入区:等宽字体、收小字号(与原实现一致);隐藏 number 原生 spinner(避免遮挡数字)。 */
  .picker-host :global(.text-input input),
  .picker-host :global(.text-input button),
  .picker-host :global(.text-input .button-like) {
    font-family: var(--mono);
    font-size: 0.72rem;
    letter-spacing: 0.04em;
  }
  .picker-host :global(.input-container input::-webkit-inner-spin-button),
  .picker-host :global(.input-container input::-webkit-outer-spin-button) {
    -webkit-appearance: none;
    margin: 0;
  }
</style>
