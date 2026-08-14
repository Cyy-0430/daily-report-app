<script lang="ts">
  import { onMount } from 'svelte';
  import {
    loadConfig,
    saveConfig,
    testConnection,
    defaultCollectPaths,
    COLLECT_TOOLS,
    type ApiConfig,
  } from '$lib/bindings';
  import { config, notify } from '$lib/store';
  import {
    DEFAULT_PROMPT_TEMPLATE,
    DEFAULT_WEEKLY_MAP_TEMPLATE,
    DEFAULT_WEEKLY_REDUCE_TEMPLATE,
    TPL_DATE,
    TPL_INPUT,
    TPL_CONV,
    TPL_DATE_RANGE,
    TPL_DAY_SUMMARIES,
  } from '$lib/template';
  import { open } from '@tauri-apps/plugin-dialog';

  type SettingsTab = 'api' | 'prompt' | 'collect';
  const SETTINGS_TABS: { id: SettingsTab; label: string }[] = [
    { id: 'api', label: 'API' },
    { id: 'prompt', label: '提示词' },
    { id: 'collect', label: '采集' },
  ];

  let activeTab = $state<SettingsTab>('api');
  const activeTabLabel = $derived(SETTINGS_TABS.find((t) => t.id === activeTab)?.label ?? '');

  let api = $state<ApiConfig>({ baseUrl: '', apiKey: '', model: '' });
  let template = $state(DEFAULT_PROMPT_TEMPLATE);
  let customDefault = $state('');
  // 周报双模板:每日摘要(map) + 整周汇总(reduce)。weeklyDef* = 各自的「自定义默认」。
  let weeklyMap = $state(DEFAULT_WEEKLY_MAP_TEMPLATE);
  let weeklyReduce = $state(DEFAULT_WEEKLY_REDUCE_TEMPLATE);
  let weeklyDefMap = $state('');
  let weeklyDefReduce = $state('');
  let exportDir = $state('');
  // 各采集工具的勾选状态(按 COLLECT_TOOLS 渲染,id 与 Rust all_collectors() 对齐)。
  let toolEnabled = $state<Record<string, boolean>>({});
  // 路径过滤:排除(黑名单)/ 仅采集(白名单),基于真实工作目录(cwd)。
  let excludePaths = $state<string[]>([]);
  let includePaths = $state<string[]>([]);
  // 各采集工具的数据源路径(可编辑)。defaultPaths = 后端权威默认;
  // toolPaths = 输入框当前值(初值 = 覆盖 ?? 默认,故始终有真实路径)。
  let defaultPaths = $state<Record<string, string>>({});
  let toolPaths = $state<Record<string, string>>({});
  let showKey = $state(false);
  let testing = $state(false);
  let saving = $state(false);

  onMount(async () => {
    const c = await loadConfig();
    api = { ...c.apiConfig };
    template = c.promptTemplate || DEFAULT_PROMPT_TEMPLATE;
    customDefault = c.customDefaultTemplate || '';
    weeklyMap = c.weeklyMapTemplate || DEFAULT_WEEKLY_MAP_TEMPLATE;
    weeklyReduce = c.weeklyReduceTemplate || DEFAULT_WEEKLY_REDUCE_TEMPLATE;
    weeklyDefMap = c.weeklyDefaultMapTemplate || '';
    weeklyDefReduce = c.weeklyDefaultReduceTemplate || '';
    exportDir = c.exportDir;
    const tools = c.collectConfig?.enabledTools ?? [];
    toolEnabled = Object.fromEntries(COLLECT_TOOLS.map((t) => [t.id, tools.includes(t.id)]));
    includePaths = [...(c.collectConfig?.includePaths ?? [])];
    excludePaths = [...(c.collectConfig?.excludePaths ?? [])];
    // 各工具数据源路径:后端给出权威默认(展开 ~),前端用「覆盖 ?? 默认」作为输入框初值。
    defaultPaths = await defaultCollectPaths();
    const saved = c.collectConfig?.toolPaths ?? {};
    toolPaths = Object.fromEntries(
      COLLECT_TOOLS.map((t) => [t.id, (saved[t.id] ?? '').trim() || defaultPaths[t.id] || '']),
    );
  });

  // 按页保存:每次只 load 当前全量配置,overlay 当前 tab 的字段后整份回写。
  // 这样其它 tab 未保存的本地编辑既不会丢失(仍在内存里),也不会被意外写入。
  async function saveApi() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = { ...cur, apiConfig: { ...api }, exportDir };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存 API 与导出设置');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function savePrompt() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = {
        ...cur,
        promptTemplate: template,
        weeklyMapTemplate: weeklyMap,
        weeklyReduceTemplate: weeklyReduce,
      };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存提示词模板');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function saveCollect() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = {
        ...cur,
        collectConfig: {
          enabledTools: COLLECT_TOOLS.filter((t) => toolEnabled[t.id]).map((t) => t.id),
          includePaths: dedupePaths(includePaths),
          excludePaths: dedupePaths(excludePaths),
          toolPaths: Object.fromEntries(COLLECT_TOOLS.map((t) => [t.id, storedPath(t.id)])),
        },
      };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存采集设置');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function saveActive() {
    if (activeTab === 'api') return saveApi();
    if (activeTab === 'prompt') return savePrompt();
    return saveCollect();
  }

  /** 规整路径列表:去空白、丢空串、去重(保留顺序)。 */
  function dedupePaths(paths: string[]): string[] {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const raw of paths) {
      const s = raw.trim();
      if (!s) continue;
      const key = s.toLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(s);
    }
    return out;
  }

  /** 规整某工具数据源路径为待存值:等于默认或为空 → ""(=用默认,保持配置干净);否则存 trim 后的值。 */
  function storedPath(id: string): string {
    const v = (toolPaths[id] ?? '').trim();
    const d = (defaultPaths[id] ?? '').trim();
    return v && v !== d ? v : '';
  }

  async function setAsDefault() {
    try {
      const cur = await loadConfig();
      cur.customDefaultTemplate = template;
      await saveConfig(cur);
      customDefault = template;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function resetTemplate() {
    template = customDefault || DEFAULT_PROMPT_TEMPLATE;
  }

  async function setWeeklyMapDefault() {
    try {
      const cur = await loadConfig();
      cur.weeklyDefaultMapTemplate = weeklyMap;
      await saveConfig(cur);
      weeklyDefMap = weeklyMap;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function resetWeeklyMap() {
    weeklyMap = weeklyDefMap || DEFAULT_WEEKLY_MAP_TEMPLATE;
  }

  async function setWeeklyReduceDefault() {
    try {
      const cur = await loadConfig();
      cur.weeklyDefaultReduceTemplate = weeklyReduce;
      await saveConfig(cur);
      weeklyDefReduce = weeklyReduce;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function resetWeeklyReduce() {
    weeklyReduce = weeklyDefReduce || DEFAULT_WEEKLY_REDUCE_TEMPLATE;
  }

  async function test() {
    testing = true;
    try {
      const msg = await testConnection({ ...api });
      notify('ok', msg);
    } catch (e) {
      notify('err', String(e));
    } finally {
      testing = false;
    }
  }

  async function pickDir() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string') exportDir = dir;
  }

  // ---- 路径过滤(排除 / 仅采集)----
  function addExcludePath() {
    excludePaths = [...excludePaths, ''];
  }
  function addIncludePath() {
    includePaths = [...includePaths, ''];
  }
  function removeExcludePath(i: number) {
    excludePaths = excludePaths.filter((_, idx) => idx !== i);
  }
  function removeIncludePath(i: number) {
    includePaths = includePaths.filter((_, idx) => idx !== i);
  }
  async function pickExcludePath(i: number) {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string' && dir) {
      excludePaths[i] = dir;
      excludePaths = [...excludePaths];
    }
  }
  async function pickIncludePath(i: number) {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string' && dir) {
      includePaths[i] = dir;
      includePaths = [...includePaths];
    }
  }
</script>

<div class="page-scroll">
  <div class="page-inner">
    <nav class="tabs">
      {#each SETTINGS_TABS as t (t.id)}
        <button class="tab" class:active={activeTab === t.id} onclick={() => (activeTab = t.id)}>
          {t.label}
        </button>
      {/each}
    </nav>

    {#if activeTab === 'api'}
      <!-- API 配置 -->
      <section class="panel sec">
        <div class="sec-title">
          API 配置
          <span class="help" tabindex="0" role="button" aria-label="说明"
            >?<span class="tip"
              >填写接口地址、模型与密钥即可连接。兼容 OpenAI 接口格式，可接
              DeepSeek、通义千问、Moonshot、本地 Ollama 等。</span
            ></span
          >
        </div>
        <div class="grid-2">
          <label class="fld">
            <span>BaseURL</span>
            <input class="field" bind:value={api.baseUrl} placeholder="https://api.openai.com/v1" />
          </label>
          <label class="fld">
            <span>模型</span>
            <input class="field" bind:value={api.model} placeholder="gpt-4o-mini" />
          </label>
        </div>
        <label class="fld">
          <span>API Key</span>
          <div class="row-input">
            <input
              class="field"
              type={showKey ? 'text' : 'password'}
              bind:value={api.apiKey}
              placeholder="sk-..."
            />
            <button class="btn btn-ghost" onclick={() => (showKey = !showKey)}>
              {showKey ? '隐藏' : '显示'}
            </button>
          </div>
        </label>
        <div class="sec-actions">
          <button class="btn btn-ghost" onclick={test} disabled={testing}>
            {testing ? '测试中…' : '测试连接'}
          </button>
        </div>
      </section>

      <!-- 导出目录 -->
      <section class="panel sec">
        <div class="sec-title">
          导出目录
          <span class="help" tabindex="0" role="button" aria-label="说明"
            >?<span class="tip"
              >日报导出时默认存到这里。留空则每次导出时手动选择保存位置；文件名默认为当天日期，如
              2025-08-14.md。</span
            ></span
          >
        </div>
        <div class="row-input">
          <input class="field" bind:value={exportDir} placeholder="例如 D:\\Reports" />
          <button class="btn btn-ghost" onclick={pickDir}>选择…</button>
          <button class="btn btn-ghost" onclick={() => (exportDir = '')}>清除</button>
        </div>
      </section>
    {:else if activeTab === 'prompt'}
      <!-- 日报模板 -->
      <section class="panel sec">
        <div class="sec-title-row">
          <div class="sec-title">
            日报模板
            <span class="help" tabindex="0" role="button" aria-label="说明"
              >?<span class="tip"
                >这份提示词决定日报的写作风格与结构。占位符：<code class="var">{TPL_DATE}</code>
                自动填入今天日期，<code class="var">{TPL_INPUT}</code> 填入你在左侧写的今日要点。</span
              ></span
            >
          </div>
          <div class="sec-actions-row">
            <button class="btn btn-ghost btn-sm" onclick={setAsDefault}>设为默认</button>
            <button class="btn btn-ghost btn-sm" onclick={resetTemplate}>恢复默认</button>
          </div>
        </div>
        <textarea bind:value={template} class="field code tmpl"></textarea>
      </section>

      <!-- 周报模板 -->
      <section class="panel sec">
        <div class="sec-title">
          周报模板
          <span class="help" tabindex="0" role="button" aria-label="说明"
            >?<span class="tip"
              >周报分两步生成：第一步用「每日摘要模板」逐日提炼每天的对话，第二步用「整周汇总模板」把每天的摘要归纳成一份完整周报。</span
            ></span
          >
        </div>

        <div class="sec-title-row">
          <div class="sub-title">
            每日摘要模板
            <span class="help" tabindex="0" role="button" aria-label="说明"
              >?<span class="tip"
                >用于提炼单日对话的摘要。占位符：<code class="var">{TPL_DATE}</code> 当天日期，<code
                  class="var">{TPL_CONV}</code
                > 当日对话内容。</span
              ></span
            >
          </div>
          <div class="sec-actions-row">
            <button class="btn btn-ghost btn-sm" onclick={setWeeklyMapDefault}>设为默认</button>
            <button class="btn btn-ghost btn-sm" onclick={resetWeeklyMap}>恢复默认</button>
          </div>
        </div>
        <textarea bind:value={weeklyMap} class="field code tmpl"></textarea>

        <div class="sec-title-row">
          <div class="sub-title">
            整周汇总模板
            <span class="help" tabindex="0" role="button" aria-label="说明"
              >?<span class="tip"
                >用于把各日摘要汇总成周报。占位符：<code class="var">{TPL_DATE_RANGE}</code>
                本周日期范围，<code class="var">{TPL_INPUT}</code> 你补充的本周要点，<code
                  class="var">{TPL_DAY_SUMMARIES}</code
                > 各日摘要。</span
              ></span
            >
          </div>
          <div class="sec-actions-row">
            <button class="btn btn-ghost btn-sm" onclick={setWeeklyReduceDefault}>设为默认</button>
            <button class="btn btn-ghost btn-sm" onclick={resetWeeklyReduce}>恢复默认</button>
          </div>
        </div>
        <textarea bind:value={weeklyReduce} class="field code tmpl"></textarea>
      </section>
    {:else}
      <!-- 采集工具 -->
      <section class="panel sec">
        <div class="sec-title">
          采集工具
          <span class="help" tabindex="0" role="button" aria-label="说明"
            >?<span class="tip"
              >勾选你在用的本地编程工具，生成日报时会自动读取这些工具当天的对话。采集到的对话会作为占位符
              <code class="var">{TPL_CONV}</code> 填入提示词。</span
            ></span
          >
        </div>
        {#each COLLECT_TOOLS as t (t.id)}
          <label class="fld fld-check">
            <input type="checkbox" bind:checked={toolEnabled[t.id]} />
            <span>{t.label} · {t.hint}</span>
          </label>
          <div class="path-row tool-path-row">
            <input
              class="field"
              bind:value={toolPaths[t.id]}
              placeholder={t.kind === 'file'
                ? '数据库文件路径,如 ~/.zcode/cli/db/db.sqlite'
                : '数据目录路径,如 ~/.claude/projects'}
            />
            <button
              class="btn btn-ghost btn-sm"
              onclick={() => (toolPaths[t.id] = defaultPaths[t.id] ?? '')}
              disabled={(toolPaths[t.id] ?? '') === (defaultPaths[t.id] ?? '')}
            >
              恢复默认
            </button>
          </div>
        {/each}

        <div class="sub-title">
          路径过滤
          <span class="help" tabindex="0" role="button" aria-label="说明"
            >?<span class="tip"
              >只想采集（或想跳过）某些项目时，在这里按项目目录过滤。子目录会一并纳入；「排除」优先于「仅采集」，被排除的目录绝不会进入日报。两项都可以留空，表示不过滤。</span
            ></span
          >
        </div>

        <div class="path-group">
          <div class="path-group-label">排除路径（黑名单）</div>
          {#each excludePaths as _, i (i)}
            <div class="path-row">
              <input class="field" bind:value={excludePaths[i]} placeholder="例如 D:\\aaaa" />
              <button class="btn btn-ghost btn-sm" onclick={() => pickExcludePath(i)}>
                选择…
              </button>
              <button class="btn btn-ghost btn-sm" onclick={() => removeExcludePath(i)}> ✕ </button>
            </div>
          {/each}
          <button class="btn btn-ghost btn-sm path-add" onclick={addExcludePath}>
            + 添加排除路径
          </button>
        </div>

        <div class="path-group">
          <div class="path-group-label">仅采集路径（白名单）</div>
          {#each includePaths as _, i (i)}
            <div class="path-row">
              <input class="field" bind:value={includePaths[i]} placeholder="例如 D:\\work" />
              <button class="btn btn-ghost btn-sm" onclick={() => pickIncludePath(i)}>
                选择…
              </button>
              <button class="btn btn-ghost btn-sm" onclick={() => removeIncludePath(i)}> ✕ </button>
            </div>
          {/each}
          <button class="btn btn-ghost btn-sm path-add" onclick={addIncludePath}>
            + 添加仅采集路径
          </button>
        </div>
      </section>
    {/if}

    <div class="page-foot">
      <button class="btn btn-primary save-btn" onclick={saveActive} disabled={saving}>
        {saving ? '保存中…' : `保存${activeTabLabel}`}
      </button>
    </div>
  </div>
</div>

<style>
  .page-scroll {
    height: 100%;
    overflow: auto;
  }
  .page-inner {
    max-width: 720px;
    margin: 0 auto;
    padding: 0 1.5rem 3rem;
  }
  .tabs {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    gap: 0.2rem;
    padding: 0.7rem 0;
    margin-bottom: 1.4rem;
    background: var(--paper);
    border-bottom: 1px solid var(--line);
  }
  .tab {
    appearance: none;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 0.5rem 0.9rem;
    margin-bottom: -1px;
    font-family: inherit;
    font-size: 0.9rem;
    color: var(--ink-soft);
    cursor: pointer;
    transition: color 0.15s;
  }
  .tab:hover {
    color: var(--ink);
  }
  .tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }
  .sec {
    position: relative;
    padding: 1.3rem 1.4rem;
    margin-bottom: 1rem;
    gap: 0;
    overflow: visible; /* 覆盖 .panel 的 overflow:hidden，让问号 tooltip 能越过卡片边界 */
  }
  .sec:hover {
    z-index: 20; /* 悬浮的卡片连同它的 tooltip 浮到相邻卡片之上 */
  }
  .sec-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.6rem;
  }
  .sec-actions-row {
    display: flex;
    gap: 0.4rem;
  }
  .sec-title {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.98rem;
    font-weight: 650;
    margin-bottom: 0.6rem;
  }
  .sec-title-row .sec-title {
    margin-bottom: 0; /* 行内标题的下方间距由 .sec-title-row 负责 */
  }
  .sub-title {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.9rem;
    font-weight: 650;
    margin: 1.1rem 0 0.4rem;
    color: var(--ink);
  }
  .sec-title-row .sub-title {
    margin: 0;
  }
  /* 圆形问号 + 悬浮提示 */
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
  .fld {
    display: block;
    margin-bottom: 0.9rem;
  }
  .fld-check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    color: var(--ink-soft);
  }
  .fld-check input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }
  .fld > span {
    display: block;
    font-size: 0.76rem;
    color: var(--ink-soft);
    margin-bottom: 0.35rem;
    letter-spacing: 0.02em;
  }
  .grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.9rem;
  }
  .grid-2 .fld {
    margin-bottom: 0.9rem;
  }
  .row-input {
    display: flex;
    gap: 0.5rem;
  }
  .row-input .field {
    flex: 1;
  }
  .sec-actions {
    margin-top: 0.4rem;
  }
  .tmpl {
    height: 260px;
    line-height: 1.65;
  }
  .var {
    font-family: var(--mono);
    font-size: 0.76rem;
    background: var(--paper);
    border: 1px solid var(--line);
    padding: 0.05rem 0.35rem;
    border-radius: 4px;
    color: var(--accent);
  }
  .page-foot {
    position: sticky;
    bottom: 0;
    z-index: 5;
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.6rem;
    margin-top: 0.8rem;
    padding: 0.7rem 0;
    background: var(--paper);
    border-top: 1px solid var(--line);
  }
  .save-btn {
    padding: 0.65rem 1.6rem;
  }
  .path-group {
    margin-top: 0.6rem;
  }
  .path-group + .path-group {
    margin-top: 0.9rem;
  }
  .path-group-label {
    font-size: 0.78rem;
    color: var(--ink-soft);
    margin-bottom: 0.4rem;
  }
  .path-row {
    display: flex;
    gap: 0.4rem;
    margin-bottom: 0.4rem;
  }
  .path-row .field {
    flex: 1;
  }
  .tool-path-row {
    margin: 0.35rem 0 0.7rem 1.6rem;
  }
  .path-add {
    margin-top: 0.15rem;
  }
</style>
