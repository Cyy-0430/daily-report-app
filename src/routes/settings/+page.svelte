<script lang="ts">
  import { onMount } from "svelte";
  import {
    loadConfig,
    saveConfig,
    testConnection,
    defaultCollectPaths,
    COLLECT_TOOLS,
    type ApiConfig,
  } from "$lib/bindings";
  import { config, notify } from "$lib/store";
  import { DEFAULT_PROMPT_TEMPLATE } from "$lib/template";
  import { open } from "@tauri-apps/plugin-dialog";

  let api = $state<ApiConfig>({ baseUrl: "", apiKey: "", model: "" });
  let template = $state(DEFAULT_PROMPT_TEMPLATE);
  let customDefault = $state("");
  let exportDir = $state("");
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
    customDefault = c.customDefaultTemplate || "";
    exportDir = c.exportDir;
    const tools = c.collectConfig?.enabledTools ?? [];
    toolEnabled = Object.fromEntries(COLLECT_TOOLS.map((t) => [t.id, tools.includes(t.id)]));
    includePaths = [...(c.collectConfig?.includePaths ?? [])];
    excludePaths = [...(c.collectConfig?.excludePaths ?? [])];
    // 各工具数据源路径:后端给出权威默认(展开 ~),前端用「覆盖 ?? 默认」作为输入框初值。
    defaultPaths = await defaultCollectPaths();
    const saved = c.collectConfig?.toolPaths ?? {};
    toolPaths = Object.fromEntries(
      COLLECT_TOOLS.map((t) => [t.id, (saved[t.id] ?? "").trim() || defaultPaths[t.id] || ""]),
    );
  });

  async function save() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = {
        ...cur,
        apiConfig: { ...api },
        promptTemplate: template,
        exportDir,
        collectConfig: {
          enabledTools: COLLECT_TOOLS.filter((t) => toolEnabled[t.id]).map((t) => t.id),
          includePaths: dedupePaths(includePaths),
          excludePaths: dedupePaths(excludePaths),
          toolPaths: Object.fromEntries(COLLECT_TOOLS.map((t) => [t.id, storedPath(t.id)])),
        },
      };
      await saveConfig(merged);
      config.set(merged);
      notify("ok", "已保存");
    } catch (e) {
      notify("err", String(e));
    } finally {
      saving = false;
    }
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
    const v = (toolPaths[id] ?? "").trim();
    const d = (defaultPaths[id] ?? "").trim();
    return v && v !== d ? v : "";
  }

  async function setAsDefault() {
    try {
      const cur = await loadConfig();
      cur.customDefaultTemplate = template;
      await saveConfig(cur);
      customDefault = template;
      notify("ok", "已设为默认");
    } catch (e) {
      notify("err", String(e));
    }
  }

  function resetTemplate() {
    template = customDefault || DEFAULT_PROMPT_TEMPLATE;
  }

  async function test() {
    testing = true;
    try {
      const msg = await testConnection({ ...api });
      notify("ok", msg);
    } catch (e) {
      notify("err", String(e));
    } finally {
      testing = false;
    }
  }

  async function pickDir() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") exportDir = dir;
  }

  // ---- 路径过滤(排除 / 仅采集)----
  function addExcludePath() {
    excludePaths = [...excludePaths, ""];
  }
  function addIncludePath() {
    includePaths = [...includePaths, ""];
  }
  function removeExcludePath(i: number) {
    excludePaths = excludePaths.filter((_, idx) => idx !== i);
  }
  function removeIncludePath(i: number) {
    includePaths = includePaths.filter((_, idx) => idx !== i);
  }
  async function pickExcludePath(i: number) {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string" && dir) {
      excludePaths[i] = dir;
      excludePaths = [...excludePaths];
    }
  }
  async function pickIncludePath(i: number) {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string" && dir) {
      includePaths[i] = dir;
      includePaths = [...includePaths];
    }
  }
</script>

<div class="page-scroll">
  <div class="page-inner">
    <header class="page-head">
      <h1>设置</h1>
      <p>配置 API、提示词模板与导出目录</p>
    </header>

    <!-- A · API -->
    <section class="panel sec">
      <div class="sec-title"><span class="num">A</span>API 配置</div>
      <p class="sec-hint">OpenAI 兼容格式，支持 OpenAI / DeepSeek / 通义 / Moonshot / 本地 Ollama 等。</p>
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
            type={showKey ? "text" : "password"}
            bind:value={api.apiKey}
            placeholder="sk-..."
          />
          <button class="btn btn-ghost" onclick={() => (showKey = !showKey)}>
            {showKey ? "隐藏" : "显示"}
          </button>
        </div>
      </label>
      <div class="sec-actions">
        <button class="btn btn-ghost" onclick={test} disabled={testing}>
          {testing ? "测试中…" : "测试连接"}
        </button>
      </div>
    </section>

    <!-- B · 模板 -->
    <section class="panel sec">
      <div class="sec-title-row">
        <div class="sec-title"><span class="num">B</span>生成提示词模板</div>
        <div class="sec-actions-row">
          <button class="btn btn-ghost btn-sm" onclick={setAsDefault}>设为默认</button>
          <button class="btn btn-ghost btn-sm" onclick={resetTemplate}>恢复默认</button>
        </div>
      </div>
      <p class="sec-hint">
        变量：<code class="var">{"{{date}}"}</code>（今天，如 7.9）、<code class="var"
          >{"{{input}}"}</code
        >（左侧输入内容）
      </p>
      <textarea bind:value={template} class="field code tmpl"></textarea>
    </section>

    <!-- C · 导出 -->
    <section class="panel sec">
      <div class="sec-title"><span class="num">C</span>导出目录</div>
      <p class="sec-hint">留空则每次导出时弹窗选择；文件名默认 yyyy-mm-dd.md。</p>
      <div class="row-input">
        <input
          class="field"
          bind:value={exportDir}
          placeholder="例如 D:\\Reports"
        />
        <button class="btn btn-ghost" onclick={pickDir}>选择…</button>
        <button class="btn btn-ghost" onclick={() => (exportDir = "")}>清除</button>
      </div>
    </section>

    <!-- D · 采集 -->
    <section class="panel sec">
      <div class="sec-title"><span class="num">D</span>采集工具</div>
      <p class="sec-hint">
        勾选日报生成时可自动读取的本地工具对话记录。模板变量 <code class="var"
          >{"{{conversations}}"}</code
        > 为采集到的当日对话（字段级过滤后，token 已大幅压缩）。
      </p>
      {#each COLLECT_TOOLS as t (t.id)}
        <label class="fld fld-check">
          <input type="checkbox" bind:checked={toolEnabled[t.id]} />
          <span>{t.label} · {t.hint}</span>
        </label>
        <div class="path-row tool-path-row">
          <input
            class="field"
            bind:value={toolPaths[t.id]}
            placeholder={t.kind === "file"
              ? "数据库文件路径,如 ~/.zcode/cli/db/db.sqlite"
              : "数据目录路径,如 ~/.claude/projects"}
          />
          <button
            class="btn btn-ghost btn-sm"
            onclick={() => (toolPaths[t.id] = defaultPaths[t.id] ?? "")}
            disabled={(toolPaths[t.id] ?? "") === (defaultPaths[t.id] ?? "")}
          >
            恢复默认
          </button>
        </div>
      {/each}

      <div class="sub-title">路径过滤</div>
      <p class="sec-hint">
        按会话的「真实工作目录」(cwd) 过滤:子目录会被一并包含/排除;<strong
          >排除优先于仅采集</strong
        >(敏感目录绝不会进日报)。两者均可留空(=不过滤);路径分隔符与大小写不影响匹配。
      </p>

      <div class="path-group">
        <div class="path-group-label">排除路径（黑名单)</div>
        {#each excludePaths as _, i (i)}
          <div class="path-row">
            <input
              class="field"
              bind:value={excludePaths[i]}
              placeholder="例如 D:\\aaaa"
            />
            <button class="btn btn-ghost btn-sm" onclick={() => pickExcludePath(i)}>
              选择…
            </button>
            <button class="btn btn-ghost btn-sm" onclick={() => removeExcludePath(i)}>
              ✕
            </button>
          </div>
        {/each}
        <button class="btn btn-ghost btn-sm path-add" onclick={addExcludePath}>
          + 添加排除路径
        </button>
      </div>

      <div class="path-group">
        <div class="path-group-label">仅采集路径（白名单)</div>
        {#each includePaths as _, i (i)}
          <div class="path-row">
            <input
              class="field"
              bind:value={includePaths[i]}
              placeholder="例如 D:\\work"
            />
            <button class="btn btn-ghost btn-sm" onclick={() => pickIncludePath(i)}>
              选择…
            </button>
            <button class="btn btn-ghost btn-sm" onclick={() => removeIncludePath(i)}>
              ✕
            </button>
          </div>
        {/each}
        <button class="btn btn-ghost btn-sm path-add" onclick={addIncludePath}>
          + 添加仅采集路径
        </button>
      </div>
    </section>

    <div class="page-foot">
      <button class="btn btn-primary save-btn" onclick={save} disabled={saving}>
        {saving ? "保存中…" : "保存设置"}
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
    padding: 2rem 1.5rem 3rem;
  }
  .page-head h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .page-head p {
    margin: 0.3rem 0 0;
    color: var(--ink-soft);
    font-size: 0.88rem;
  }
  .page-head {
    margin-bottom: 1.5rem;
  }
  .sec {
    padding: 1.3rem 1.4rem;
    margin-bottom: 1rem;
    gap: 0;
  }
  .sec-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .sec-actions-row {
    display: flex;
    gap: 0.4rem;
  }
  .sec-title {
    font-size: 0.98rem;
    font-weight: 650;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.3rem;
  }
  .num {
    font-family: var(--mono);
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 5px;
    width: 20px;
    height: 20px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .sec-hint {
    color: var(--ink-faint);
    font-size: 0.78rem;
    margin: 0 0 1rem;
    line-height: 1.6;
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
    display: flex;
    justify-content: flex-end;
    margin-top: 0.5rem;
  }
  .save-btn {
    padding: 0.65rem 1.6rem;
  }
  .sub-title {
    font-size: 0.9rem;
    font-weight: 650;
    margin: 1.1rem 0 0.2rem;
    color: var(--ink);
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
