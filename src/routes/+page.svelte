<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import {
    generateReport,
    collectConversations,
    COLLECT_TOOLS,
    DEFAULT_TOOL_IDS,
    type CollectResult,
  } from '$lib/bindings';
  import { config, history, notify, pendingInput, MSG_API_NOT_CONFIGURED } from '$lib/store';
  import ReportPanel from '$lib/components/ReportPanel.svelte';

  let input = $state('');
  let output = $state('');
  let busy = $state(false);

  // 采集相关
  let collectDate = $state(todayStr());
  let collecting = $state(false);
  let collectResult = $state<CollectResult | null>(null);
  let showConversations = $state(false);

  // 采集来源标签:依勾选的工具动态展示(id→label),多个用英文逗号隔开。
  // enabledTools 为空时回退到默认四个(与采集逻辑一致)。
  const enabledToolIds = $derived.by(() => {
    const t = $config.collectConfig?.enabledTools ?? [];
    return t.length ? t : DEFAULT_TOOL_IDS;
  });
  const collectSourceLabel = $derived(
    enabledToolIds.map((id) => COLLECT_TOOLS.find((t) => t.id === id)?.label ?? id).join(', '),
  );

  function todayStr(): string {
    const d = new Date();
    const mm = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${mm}-${dd}`;
  }

  onMount(() => {
    const p = get(pendingInput);
    if (p) {
      input = p;
      pendingInput.set(null);
    }
  });

  function conversationsText(): string {
    return collectResult?.renderedText ?? '';
  }

  async function onCollect() {
    const cfg = $config.collectConfig;
    const tools = enabledToolIds;
    // 路径过滤:从配置读取,缺省等价于空规则(不过滤,向后兼容)。
    const filter = {
      includePaths: cfg?.includePaths ?? [],
      excludePaths: cfg?.excludePaths ?? [],
    };
    collecting = true;
    showConversations = false;
    try {
      const res = await collectConversations(collectDate, tools, filter, cfg?.toolPaths ?? {});
      collectResult = res;
      if (res.sessions.length === 0) {
        notify('err', `${collectDate} 无对话记录`);
      } else {
        notify('ok', `已采集 ${res.sessions.length} 个会话 · 约 ${res.estTokens} token`);
      }
    } catch (e) {
      notify('err', String(e));
    } finally {
      collecting = false;
    }
  }

  async function onGenerate() {
    if (!$config.apiConfig.baseUrl || !$config.apiConfig.apiKey || !$config.apiConfig.model) {
      notify('err', MSG_API_NOT_CONFIGURED);
      return;
    }
    const conv = conversationsText();
    if (!input.trim() && !conv.trim()) {
      notify('err', '请填写今日要点，或先「采集对话」');
      return;
    }
    busy = true;
    output = '';
    try {
      const item = await generateReport(input, conv, (chunk) => {
        if (chunk.type === 'delta') output += chunk.text;
        else if (chunk.type === 'error') notify('err', chunk.message);
      });
      history.update((h) => [item, ...h]);
      notify('ok', '生成完成');
    } catch (e) {
      notify('err', String(e));
    } finally {
      busy = false;
    }
  }
</script>

<div class="editor-grid">
  <!-- 01 · 输入 -->
  <section class="panel">
    <div class="panel-head">
      <span class="panel-label">01 — 今日要点</span>
      <input
        class="collect-date"
        type="date"
        bind:value={collectDate}
        disabled={busy || collecting}
      />
      <button
        class="btn btn-ghost btn-sm"
        onclick={() => {
          input = '';
          output = '';
          collectResult = null;
          showConversations = false;
        }}
        disabled={busy}
      >
        清空
      </button>
    </div>

    <div class="collect-bar">
      <span class="collect-src">来源：{collectSourceLabel}</span>
      {#if !collectResult || collectResult.sessions.length === 0}
        <button class="btn btn-ghost btn-sm" onclick={onCollect} disabled={busy || collecting}>
          {collecting ? '采集中…' : '采集对话'}
        </button>
      {:else if collectResult}
        <span class="meta collect-meta">
          {#if collectResult.sessions.length}
            {collectResult.sessions.length} 会话 · 约 {collectResult.estTokens} token
          {:else}
            无记录
          {/if}
        </span>
        <button
          class="btn btn-ghost btn-sm"
          onclick={() => (showConversations = !showConversations)}
          disabled={!collectResult.renderedText}
        >
          {showConversations ? '收起' : '查看'}
        </button>
      {/if}
    </div>

    {#if showConversations && collectResult?.renderedText}
      <pre class="collect-preview">{collectResult.renderedText}</pre>
    {/if}

    <textarea
      bind:value={input}
      placeholder="用要点写下今天做的事，越具体越好…（也可留空，点上方「采集对话」自动汇总）"
      class="editor-textarea"></textarea>
    <div class="panel-foot">
      <span class="meta">{input.length} 字</span>
      <button class="btn btn-primary" onclick={onGenerate} disabled={busy}>
        {busy ? '生成中…' : '生成日报'}<span class="arrow">→</span>
      </button>
    </div>
  </section>

  <!-- 02 · 日报 -->
  <ReportPanel bind:output {busy} label="日报" exportName={collectDate} />
</div>

<style>
  .editor-grid {
    height: 100%;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    padding: 1rem;
    align-items: stretch;
  }
  .panel {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .panel:first-child .panel-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .panel:first-child .panel-head .collect-date {
    margin-left: auto;
  }
  .collect-bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1.15rem;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .collect-src {
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--ink-faint);
    margin-right: auto;
  }
  .collect-date {
    font-family: var(--mono);
    font-size: 0.78rem;
    color: var(--ink-soft);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 0.2rem 0.4rem;
    background: var(--paper);
  }
  .collect-meta {
    font-size: 0.72rem;
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
  .meta {
    font-family: var(--mono);
    font-size: 0.74rem;
    color: var(--ink-faint);
  }
  .arrow {
    margin-left: 0.35rem;
  }
</style>
