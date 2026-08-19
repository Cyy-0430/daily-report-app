<script lang="ts">
  import type { Snippet } from 'svelte';

  // 圆形问号 + 悬浮提示气泡(原设置页 10+ 处复制粘贴的 .help/.tip 结构抽出)。
  // children snippet = 提示正文(可含 <code class="var">{TPL_*}</code> 等富文本;
  // snippet 内容按父组件作用域编译,故 .var 等正文样式必须留在全局 settings-shared.css)。
  let { children }: { children: Snippet } = $props();
</script>

<span class="help" tabindex="0" role="button" aria-label="说明"
  >?<span class="tip">{@render children()}</span></span
>

<style>
  .help {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1px solid var(--ink-faint);
    color: var(--ink-soft);
    font-family: var(--mono);
    font-size: 0.6rem;
    font-weight: 700;
    line-height: 1;
    cursor: help;
    transition:
      color 0.15s,
      border-color 0.15s;
  }
  .help:hover,
  .help:focus-visible {
    border-color: var(--accent);
    color: var(--accent);
    outline: none;
  }
  .tip {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 30;
    width: max-content;
    max-width: 300px;
    padding: 0.7rem 0.85rem;
    background: var(--paper-card);
    color: var(--ink);
    border: 1px solid var(--line);
    border-radius: 8px;
    font-size: 0.76rem;
    font-weight: 400;
    line-height: 1.6;
    letter-spacing: normal;
    white-space: normal;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.1);
    visibility: hidden;
    opacity: 0;
    transform: translateY(-4px);
    transition:
      opacity 0.15s,
      transform 0.15s;
    pointer-events: none;
  }
  .help:hover .tip,
  .help:focus-visible .tip {
    visibility: visible;
    opacity: 1;
    transform: translateY(0);
  }
</style>
