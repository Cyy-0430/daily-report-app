<script lang="ts">
  import type { Snippet } from 'svelte';

  // 报告编辑器左侧输入面板(日报/周报共用骨架):panel-head(label + head snippet)+
  // extra snippet 中部扩展区 + textarea + panel-foot(字数 + 生成按钮)。
  // head/extra snippet 内容按父页面作用域编译 —— snippet 里用到的类(.collect-bar/.meta 等)
  // 必须在 report-shared.css 全局生效,不能 scoped 进本组件(见该文件头注释)。
  let {
    label,
    value = $bindable(''),
    placeholder = '',
    generateLabel,
    busy = false,
    disabled = false,
    ongenerate,
    head,
    extra,
  }: {
    /** 面板标签(如 "01 — 今日要点")。 */
    label: string;
    value?: string;
    placeholder?: string;
    /** 生成按钮文案(非生成中时)。 */
    generateLabel: string;
    /** 生成中态:按钮文案切换为「生成中…」。 */
    busy?: boolean;
    /** 生成按钮禁用条件(周报 = busy || collecting,日报 = busy)。 */
    disabled?: boolean;
    ongenerate: () => void;
    /** head 右区:日期(区间)选择 + 清空按钮,由页面注入。 */
    head?: Snippet;
    /** 中部扩展区:采集条 / 预览 / 日列表 / 进度条,由页面注入。 */
    extra?: Snippet;
  } = $props();
</script>

<section class="panel">
  <div class="panel-head">
    <span class="panel-label">{label}</span>
    {#if head}{@render head()}{/if}
  </div>

  {#if extra}{@render extra()}{/if}

  <textarea bind:value {placeholder} class="editor-textarea"></textarea>

  <div class="panel-foot">
    <span class="meta">{value.length} 字</span>
    <button class="btn btn-primary" onclick={ongenerate} {disabled}>
      {busy ? '生成中…' : generateLabel}<span class="arrow">→</span>
    </button>
  </div>
</section>

<style>
  .panel {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* head 只覆盖 flex/gap,其余(padding/min-height/边框)走 app.css 全局 .panel-head;
     label 不覆盖,走全局 .panel-label。把日期控件推到右侧的 margin-left:auto 由
     head snippet 内元素承担(类在各页 scoped)。panel-foot 不覆盖,走 app.css 全局。 */
  .panel-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .editor-textarea {
    flex: 1;
    min-height: 0;
    width: 100%;
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    padding: 1.05rem 1.15rem;
    font-family: var(--sans);
    font-size: 0.9rem;
    line-height: 1.75;
    color: var(--ink);
  }
  .editor-textarea::placeholder {
    color: var(--ink-faint);
  }
  .arrow {
    margin-left: 0.35rem;
  }
</style>
