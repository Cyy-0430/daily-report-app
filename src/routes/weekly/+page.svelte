<script lang="ts">
  import {
    collectConversationsRange,
    generateWeeklyReport,
    COLLECT_TOOLS,
    type RangeCollectResult,
  } from "$lib/bindings";
  import { config, history, notify } from "$lib/store";
  import ReportPanel from "$lib/components/ReportPanel.svelte";

  let startDate = $state(mondayStr());
  let endDate = $state(todayStr());
  let collecting = $state(false);
  let rangeResult = $state<RangeCollectResult | null>(null);

  let weeklyInput = $state("");
  let output = $state("");
  let busy = $state(false);
  // 当前进度(map/reduce);null = 未在生成。
  let progress = $state<{ stage: string; current: number; total: number; message: string } | null>(
    null,
  );

  // 单日 token 超此阈值告警(单日 map 批过大可能撞上下文)。
  const DAY_TOKEN_WARN = 60000;

  const enabledToolIds = $derived.by(() => {
    const t = $config.collectConfig?.enabledTools ?? [];
    return t.length ? t : ["claude-code", "zcode", "codex", "opencode"];
  });
  const collectSourceLabel = $derived(
    enabledToolIds
      .map((id) => COLLECT_TOOLS.find((t) => t.id === id)?.label ?? id)
      .join(", "),
  );
  // 区间内是否有一天 token 超阈值。
  const hasOversizedDay = $derived(
    !!rangeResult && rangeResult.days.some((d) => d.estTokens > DAY_TOKEN_WARN),
  );
  const apiReady = $derived(
    !!$config.apiConfig?.baseUrl && !!$config.apiConfig?.apiKey && !!$config.apiConfig?.model,
  );

  function todayStr(): string {
    return fmt(new Date());
  }
  /** 本周一(Monday)。 */
  function mondayStr(): string {
    const d = new Date();
    const day = d.getDay(); // 0=Sun..6=Sat
    const diff = day === 0 ? 6 : day - 1;
    const mon = new Date(d);
    mon.setDate(d.getDate() - diff);
    return fmt(mon);
  }
  function fmt(d: Date): string {
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${d.getFullYear()}-${mm}-${dd}`;
  }

  async function onCollectRange() {
    const cfg = $config.collectConfig;
    const tools = enabledToolIds;
    const filter = {
      includePaths: cfg?.includePaths ?? [],
      excludePaths: cfg?.excludePaths ?? [],
    };
    collecting = true;
    try {
      const res = await collectConversationsRange(
        startDate,
        endDate,
        tools,
        filter,
        cfg?.toolPaths ?? {},
      );
      rangeResult = res;
      const sessions = res.days.reduce((n, d) => n + d.sessions.length, 0);
      if (sessions === 0) {
        notify("err", `${startDate} ~ ${endDate} 无对话记录`);
      } else {
        notify("ok", `已采集 ${sessions} 个会话 · 约 ${res.totalTokens} token`);
      }
    } catch (e) {
      notify("err", String(e));
    } finally {
      collecting = false;
    }
  }

  async function onGenerate() {
    if (!apiReady) {
      notify("err", "请先在「设置」中配置 API");
      return;
    }
    busy = true;
    output = "";
    progress = null;
    try {
      const cfg = $config.collectConfig;
      const filter = {
        includePaths: cfg?.includePaths ?? [],
        excludePaths: cfg?.excludePaths ?? [],
      };
      const item = await generateWeeklyReport(
        startDate,
        endDate,
        enabledToolIds,
        filter,
        cfg?.toolPaths ?? {},
        weeklyInput,
        (chunk) => {
          if (chunk.type === "delta") output += chunk.text;
          else if (chunk.type === "progress") progress = chunk;
          else if (chunk.type === "error") notify("err", chunk.message);
        },
      );
      history.update((h) => [item, ...h]);
      notify("ok", "周报生成完成");
    } catch (e) {
      notify("err", String(e));
    } finally {
      busy = false;
      progress = null;
    }
  }
</script>

<div class="editor-grid">
  <!-- 01 · 区间输入 -->
  <section class="panel">
    <div class="panel-head">
      <span class="panel-label">01 — 区间采集</span>
      <div class="range-pick">
        <input class="collect-date" type="date" bind:value={startDate} disabled={busy || collecting} />
        <span class="sep">~</span>
        <input class="collect-date" type="date" bind:value={endDate} disabled={busy || collecting} />
      </div>
      <button
        class="btn btn-ghost btn-sm"
        onclick={() => {
          weeklyInput = "";
          output = "";
          rangeResult = null;
        }}
        disabled={busy}
      >
        清空
      </button>
    </div>

    <div class="collect-bar">
      <span class="collect-src">来源：{collectSourceLabel}</span>
      <button class="btn btn-ghost btn-sm" onclick={onCollectRange} disabled={busy || collecting}>
        {collecting ? "采集中…" : "采集区间"}
      </button>
      {#if rangeResult}
        <span class="meta collect-meta">
          {rangeResult.days.reduce((n, d) => n + d.sessions.length, 0)} 会话 · 约 {rangeResult.totalTokens} token
        </span>
      {/if}
    </div>

    {#if rangeResult}
      <div class="day-list">
        {#each rangeResult.days as d (d.date)}
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

    {#if progress}
      <div class="progress-row">
        <span class="progress-msg">
          {progress.stage === "reduce" ? "正在汇总…" : progress.message}
        </span>
        {#if progress.total > 1}
          <span class="progress-count">{progress.current}/{progress.total}</span>
          <div class="progress-bar"><div class="fill" style="width:{(progress.current / progress.total) * 100}%"></div></div>
        {/if}
      </div>
    {/if}

    <textarea
      bind:value={weeklyInput}
      placeholder="本周补充要点（可选）：会议、非编码工作、本周目标等日志里没有的内容……"
      class="editor-textarea"
    ></textarea>

    <div class="panel-foot">
      <span class="meta">{weeklyInput.length} 字</span>
      <button class="btn btn-primary" onclick={onGenerate} disabled={busy || collecting}>
        {busy ? "生成中…" : "生成周报"}<span class="arrow">→</span>
      </button>
    </div>
  </section>

  <!-- 02 · 周报 -->
  <ReportPanel bind:output busy={busy} label="周报" exportName={`${startDate}_${endDate}`} />
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
  /* 与日报页一致的局部覆盖:head 只覆盖 flex/gap,其余(padding/min-height/边框)
     走 app.css 全局 .panel-head;label 不覆盖,走全局 .panel-label。 */
  .panel:first-child .panel-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
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
  /* panel-foot 不覆盖,走 app.css 全局(padding 0 1rem / min-height 48px / space-between)。 */
  .meta {
    font-family: var(--mono);
    font-size: 0.74rem;
    color: var(--ink-faint);
  }
  .arrow {
    margin-left: 0.35rem;
  }
</style>
