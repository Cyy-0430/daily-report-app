<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { generateReport, collectConversations } from '$lib/bindings';
  import { config, history, notify, pendingInput, MSG_API_NOT_CONFIGURED } from '$lib/store';
  import ReportPanel from '$lib/components/ReportPanel.svelte';
  import InputPanel from '$lib/components/report/InputPanel.svelte';
  // 报告编辑器页共享样式(.editor-grid/.collect-bar 等);导入一次全局生效,仅报告页使用。
  import '$lib/components/report/report-shared.css';
  // 页面工作状态(模块级 $state 单例):切到设置/历史再切回,内容原样保留。
  import { daily, enabledToolIdsOf, sourceLabelOf, buildFilter } from '$lib/report-state.svelte';

  // 采集来源标签:依勾选的工具动态展示(id→label),多个用英文逗号隔开。
  // enabledTools 为空时回退到默认四个(与采集逻辑一致)。
  const enabledToolIds = $derived(enabledToolIdsOf($config));
  const collectSourceLabel = $derived(sourceLabelOf(enabledToolIds));

  // 历史记录「复用」回填:回到本页时写入模块状态。
  onMount(() => {
    const p = get(pendingInput);
    if (p) {
      daily.input = p;
      pendingInput.set(null);
    }
  });

  function conversationsText(): string {
    return daily.collectResult?.renderedText ?? '';
  }

  async function onCollect() {
    const tools = enabledToolIds;
    // 路径过滤:从配置读取,缺省等价于空规则(不过滤,向后兼容)。
    const filter = buildFilter($config);
    const toolPaths = $config.collectConfig?.toolPaths ?? {};
    daily.collecting = true;
    daily.showConversations = false;
    try {
      const res = await collectConversations(daily.collectDate, tools, filter, toolPaths);
      daily.collectResult = res;
      if (res.sessions.length === 0) {
        notify('err', `${daily.collectDate} 无对话记录`);
      } else {
        notify('ok', `已采集 ${res.sessions.length} 个会话 · 约 ${res.estTokens} token`);
      }
    } catch (e) {
      notify('err', String(e));
    } finally {
      daily.collecting = false;
    }
  }

  async function onGenerate() {
    if (!$config.apiConfig.baseUrl || !$config.apiConfig.apiKey || !$config.apiConfig.model) {
      notify('err', MSG_API_NOT_CONFIGURED);
      return;
    }
    const conv = conversationsText();
    if (!daily.input.trim() && !conv.trim()) {
      notify('err', '请填写今日要点，或先「采集对话」');
      return;
    }
    daily.busy = true;
    daily.output = '';
    try {
      const item = await generateReport(daily.input, conv, (chunk) => {
        if (chunk.type === 'delta') daily.output += chunk.text;
        else if (chunk.type === 'error') notify('err', chunk.message);
      });
      history.update((h) => [item, ...h]);
      notify('ok', '生成完成');
    } catch (e) {
      notify('err', String(e));
    } finally {
      daily.busy = false;
    }
  }

  function onClear() {
    daily.input = '';
    daily.output = '';
    daily.collectResult = null;
    daily.showConversations = false;
  }
</script>

<div class="editor-grid">
  <!-- 01 · 输入 -->
  <InputPanel
    label="01 — 今日要点"
    bind:value={daily.input}
    placeholder="用要点写下今天做的事，越具体越好…（也可留空，点上方「采集对话」自动汇总）"
    generateLabel="生成日报"
    busy={daily.busy}
    disabled={daily.busy}
    ongenerate={onGenerate}
  >
    {#snippet head()}
      <input
        class="collect-date"
        type="date"
        bind:value={daily.collectDate}
        disabled={daily.busy || daily.collecting}
      />
      <button class="btn btn-ghost btn-sm" onclick={onClear} disabled={daily.busy}> 清空 </button>
    {/snippet}

    {#snippet extra()}
      <div class="collect-bar">
        <span class="collect-src">来源：{collectSourceLabel}</span>
        {#if !daily.collectResult || daily.collectResult.sessions.length === 0}
          <button
            class="btn btn-ghost btn-sm"
            onclick={onCollect}
            disabled={daily.busy || daily.collecting}
          >
            {daily.collecting ? '采集中…' : '采集对话'}
          </button>
        {:else if daily.collectResult}
          <span class="meta collect-meta">
            {#if daily.collectResult.sessions.length}
              {daily.collectResult.sessions.length} 会话 · 约 {daily.collectResult.estTokens} token
            {:else}
              无记录
            {/if}
          </span>
          <button
            class="btn btn-ghost btn-sm"
            onclick={() => (daily.showConversations = !daily.showConversations)}
            disabled={!daily.collectResult.renderedText}
          >
            {daily.showConversations ? '收起' : '查看'}
          </button>
        {/if}
      </div>

      {#if daily.showConversations && daily.collectResult?.renderedText}
        <pre class="collect-preview">{daily.collectResult.renderedText}</pre>
      {/if}
    {/snippet}
  </InputPanel>

  <!-- 02 · 日报 -->
  <ReportPanel
    bind:output={daily.output}
    bind:mode={daily.mode}
    busy={daily.busy}
    label="日报"
    exportName={daily.collectDate}
  />
</div>

<style>
  /* head 里日期控件推到右侧(snippet 内容按本页作用域编译,scoped 即可命中)。 */
  .collect-date {
    margin-left: auto;
  }
  .collect-preview {
    max-height: 160px;
    overflow: auto;
    margin: 0;
    padding: 0.6rem 1.15rem;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 0.74rem;
    line-height: 1.6;
    color: var(--ink-soft);
    background: var(--paper);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
