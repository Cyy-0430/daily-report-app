<script lang="ts">
  import { onMount } from "svelte";
  import { loadConfig, saveConfig, testConnection, type ApiConfig } from "$lib/bindings";
  import { config, notify } from "$lib/store";
  import { DEFAULT_PROMPT_TEMPLATE } from "$lib/template";
  import { open } from "@tauri-apps/plugin-dialog";

  let api = $state<ApiConfig>({ baseUrl: "", apiKey: "", model: "" });
  let template = $state(DEFAULT_PROMPT_TEMPLATE);
  let exportDir = $state("");
  let showKey = $state(false);
  let testing = $state(false);
  let saving = $state(false);

  onMount(async () => {
    const c = await loadConfig();
    api = { ...c.apiConfig };
    template = c.promptTemplate || DEFAULT_PROMPT_TEMPLATE;
    exportDir = c.exportDir;
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

  function resetTemplate() {
    template = DEFAULT_PROMPT_TEMPLATE;
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
        <button class="btn btn-ghost btn-sm" onclick={resetTemplate}>恢复默认</button>
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
</style>
