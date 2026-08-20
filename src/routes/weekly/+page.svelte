<script lang="ts">
  import { collectConversationsRange, generateWeeklyReport } from '$lib/bindings';
  import { config, history, notify, MSG_API_NOT_CONFIGURED } from '$lib/store';
  import ReportPanel from '$lib/components/ReportPanel.svelte';
  import InputPanel from '$lib/components/report/InputPanel.svelte';
  // 报告编辑器页共享样式(.editor-grid/.collect-bar 等);导入一次全局生效,仅报告页使用。
  import '$lib/components/report/report-shared.css';
  // 页面工作状态(模块级 $state 单例):切到设置/历史再切回,内容原样保留。
  import { weekly, enabledToolIdsOf, sourceLabelOf, buildFilter } from '$lib/report-state.svelte';

  // 单日 token 超此阈值告警(单日 map 批过大可能撞上下文)。
  const DAY_TOKEN_WARN = 60000;

  const enabledToolIds = $derived(enabledToolIdsOf($config));
  const collectSourceLabel = $derived(sourceLabelOf(enabledToolIds));
  // 区间内是否有一天 token 超阈值。
  const hasOversizedDay = $derived(
    !!weekly.rangeResult && weekly.rangeResult.days.some((d) => d.estTokens > DAY_TOKEN_WARN),
  );
  const apiReady = $derived(
    !!$config.apiConfig?.baseUrl && !!$config.apiConfig?.apiKey && !!$config.apiConfig?.model,
  );

  async function onCollectRange() {
    const tools = enabledToolIds;
    const filter = buildFilter($config);
    const toolPaths = $config.collectConfig?.toolPaths ?? {};
    weekly.collecting = true;
    try {
      const res = await collectConversationsRange(
        weekly.startDate,
        weekly.endDate,
        tools,
        filter,
        toolPaths,
      );
      weekly.rangeResult = res;
      const sessions = res.days.reduce((n, d) => n + d.sessions.length, 0);
      if (sessions === 0) {
        notify('err', `${weekly.startDate} ~ ${weekly.endDate} 无对话记录`);
      } else {
        notify('ok', `已采集 ${sessions} 个会话 · 约 ${res.totalTokens} token`);
      }
    } catch (e) {
      notify('err', String(e));
    } finally {
      weekly.collecting = false;
    }
  }

  async function onGenerate() {
    if (!apiReady) {
      notify('err', MSG_API_NOT_CONFIGURED);
      return;
    }
    weekly.busy = true;
    weekly.output = '';
    weekly.progress = null;
    try {
      const cfg = $config.collectConfig;
      const filter = buildFilter($config);
      const item = await generateWeeklyReport(
        weekly.startDate,
        weekly.endDate,
        enabledToolIds,
        filter,
        cfg?.toolPaths ?? {},
        weekly.weeklyInput,
        (chunk) => {
          if (chunk.type === 'delta') weekly.output += chunk.text;
          else if (chunk.type === 'progress') weekly.progress = chunk;
          else if (chunk.type === 'error') notify('err', chunk.message);
        },
      );
      history.update((h) => [item, ...h]);
      notify('ok', '周报生成完成');
    } catch (e) {
      notify('err', String(e));
    } finally {
      weekly.busy = false;
      weekly.progress = null;
    }
  }

  function onClear() {
    weekly.weeklyInput = '';
    weekly.output = '';
    weekly.rangeResult = null;
  }
</script>

<div class="editor-grid">
  <!-- 01 · 区间输入 -->
  <InputPanel
    label="01 — 区间采集"
    bind:value={weekly.weeklyInput}
    placeholder="本周补充要点（可选）：会议、非编码工作、本周目标等日志里没有的内容……"
    generateLabel="生成周报"
    busy={weekly.busy}
    disabled={weekly.busy || weekly.collecting}
    ongenerate={onGenerate}
  >
    {#snippet head()}
      <div class="range-pick">
        <input
          class="collect-date"
          type="date"
          bind:value={weekly.startDate}
          disabled={weekly.busy || weekly.collecting}
        />
        <span class="sep">~</span>
        <input
          class="collect-date"
          type="date"
          bind:value={weekly.endDate}
          disabled={weekly.busy || weekly.collecting}
        />
      </div>
      <button class="btn btn-ghost btn-sm" onclick={onClear} disabled={weekly.busy}> 清空 </button>
    {/snippet}

    {#snippet extra()}
      <div class="collect-bar">
        <span class="collect-src">来源：{collectSourceLabel}</span>
        <button
          class="btn btn-ghost btn-sm"
          onclick={onCollectRange}
          disabled={weekly.busy || weekly.collecting}
        >
          {weekly.collecting ? '采集中…' : '采集区间'}
        </button>
        {#if weekly.rangeResult}
          <span class="meta collect-meta">
            {weekly.rangeResult.days.reduce((n, d) => n + d.sessions.length, 0)} 会话 · 约
            {weekly.rangeResult.totalTokens} token
          </span>
        {/if}
      </div>

      {#if weekly.rangeResult}
        <div class="day-list">
          {#each weekly.rangeResult.days as d (d.date)}
            <div class="day-row" class:warn={d.estTokens > DAY_TOKEN_WARN}>
              <span class="day-date">{d.date.slice(5)}</span>
              <span class="day-meta">{d.sessions.length} 会话 · {d.estTokens} tok</span>
            </div>
          {/each}
        </div>
        {#if hasOversizedDay}
          <p class="warn-note">
            ⚠️ 某天对话量较大(单日 &gt; {DAY_TOKEN_WARN} token)，当日摘要可能不完整。可在「设置」用路径过滤收窄范围。
          </p>
        {/if}
      {/if}

      {#if weekly.progress}
        <div class="progress-row">
          <span class="progress-msg">
            {weekly.progress.stage === 'reduce' ? '正在汇总…' : weekly.progress.message}
          </span>
          {#if weekly.progress.total > 1}
            <span class="progress-count">{weekly.progress.current}/{weekly.progress.total}</span>
            <div class="progress-bar">
              <div
                class="fill"
                style="width:{(weekly.progress.current / weekly.progress.total) * 100}%"
              ></div>
            </div>
          {/if}
        </div>
      {/if}
    {/snippet}
  </InputPanel>

  <!-- 02 · 周报 -->
  <ReportPanel
    bind:output={weekly.output}
    bind:mode={weekly.mode}
    busy={weekly.busy}
    label="周报"
    exportName={`${weekly.startDate}_${weekly.endDate}`}
  />
</div>

<style>
  /* head 里区间选择推到右侧(snippet 内容按本页作用域编译,scoped 即可命中)。 */
  .range-pick {
    margin-left: auto; /* 对应日报 head 里 collect-date 的 margin-left:auto */
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .sep {
    color: var(--ink-faint);
    font-family: var(--mono);
  }
  .day-list {
    overflow: auto;
    max-height: 140px;
    padding: 0.4rem 1.15rem;
    border-bottom: 1px solid var(--line);
  }
  .day-row {
    display: flex;
    justify-content: space-between;
    padding: 0.15rem 0;
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--ink-soft);
  }
  .day-row.warn .day-meta {
    color: var(--accent);
  }
  .warn-note {
    padding: 0.4rem 1.15rem 0.5rem;
    font-size: 0.72rem;
    color: var(--accent);
    line-height: 1.5;
    border-bottom: 1px solid var(--line);
    margin: 0;
  }
  .progress-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1.15rem;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .progress-msg {
    font-family: var(--mono);
    font-size: 0.74rem;
    color: var(--accent);
  }
  .progress-count {
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--ink-faint);
  }
  .progress-bar {
    flex: 1;
    min-width: 80px;
    height: 4px;
    background: var(--line);
    border-radius: 2px;
    overflow: hidden;
  }
  .progress-bar .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s ease;
  }
</style>
