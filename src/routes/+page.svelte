<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { generateReport, exportReport, writeTextFile } from "$lib/bindings";
  import { config, notify, pendingInput } from "$lib/store";
  import { renderMarkdown } from "$lib/markdown";
  import { save } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";

  let input = $state("");
  let output = $state("");
  let busy = $state(false);
  let mode = $state<"edit" | "preview">("preview");

  let html = $derived(renderMarkdown(output));

  onMount(() => {
    const p = get(pendingInput);
    if (p) {
      input = p;
      pendingInput.set(null);
    }
  });

  async function onGenerate() {
    if (!$config.apiConfig.baseUrl || !$config.apiConfig.apiKey || !$config.apiConfig.model) {
      notify("err", "请先在「设置」中配置 API");
      return;
    }
    if (!input.trim()) {
      notify("err", "请先填写今日工作要点");
      return;
    }
    busy = true;
    output = "";
    mode = "preview";
    try {
      await generateReport(input, (chunk) => {
        if (chunk.type === "delta") output += chunk.text;
        else if (chunk.type === "error") notify("err", chunk.message);
      });
      notify("ok", "生成完成");
    } catch (e) {
      notify("err", String(e));
    } finally {
      busy = false;
    }
  }

  async function onCopy() {
    if (!output) return;
    try {
      await writeText(output);
      notify("ok", "已复制到剪贴板");
    } catch (e) {
      notify("err", String(e));
    }
  }

  async function onExport() {
    if (!output) return;
    try {
      const saved = await exportReport(output);
      if (saved) {
        notify("ok", `已导出：${saved}`);
        return;
      }
      const date = new Date().toISOString().slice(0, 10);
      const path = await save({
        defaultPath: `${date}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await writeTextFile(path, output);
      notify("ok", `已导出：${path}`);
    } catch (e) {
      notify("err", String(e));
    }
  }
</script>

<div class="editor-grid">
  <!-- 01 · 输入 -->
  <section class="panel">
    <div class="panel-head">
      <span class="panel-label">01 — 今日要点</span>
      <span class="meta">{input.length} 字</span>
    </div>
    <textarea
      bind:value={input}
      placeholder="用要点写下今天做的事，越具体越好…"
      class="editor-textarea"
    ></textarea>
    <div class="panel-foot">
      <button
        class="btn btn-ghost"
        onclick={() => {
          input = "";
          output = "";
        }}
        disabled={busy}
      >
        清空
      </button>
      <button class="btn btn-primary" onclick={onGenerate} disabled={busy}>
        {busy ? "生成中…" : "生成日报"}<span class="arrow">→</span>
      </button>
    </div>
  </section>

  <!-- 02 · 日报 -->
  <section class="panel">
    <div class="panel-head">
      <span class="panel-label">02 — {mode === "edit" ? "编辑" : "日报"}</span>
      <div class="head-actions">
        <button
          class="btn btn-ghost btn-sm"
          onclick={() => (mode = mode === "edit" ? "preview" : "edit")}
          disabled={!output || busy}
        >
          {mode === "edit" ? "预览" : "编辑"}
        </button>
        <button class="btn btn-ghost btn-sm" onclick={onCopy} disabled={!output || busy}>
          复制
        </button>
        <button class="btn btn-accent btn-sm" onclick={onExport} disabled={!output || busy}>
          导出 .md
        </button>
      </div>
    </div>

    <div class="editor-body">
      {#if mode === "edit"}
        <textarea bind:value={output} class="editor-textarea is-code"></textarea>
      {:else if output}
        <article class="md-body">{@html html}</article>
      {:else}
        <div class="editor-empty">
          <span class="empty-mark">▍</span>
          <p>填写左侧要点，点「生成日报」<br />结果会逐字呈现，之后可手动编辑。</p>
        </div>
      {/if}
    </div>

    <div class="panel-foot">
      <span class="meta">{busy ? "streaming…" : output ? `约 ${output.length} 字` : ""}</span>
    </div>
  </section>
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
  .editor-textarea.is-code {
    font-family: var(--mono);
    font-size: 0.84rem;
  }
  .editor-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 1.05rem 1.15rem;
  }
  .head-actions {
    display: flex;
    gap: 0.4rem;
  }
  .meta {
    font-family: var(--mono);
    font-size: 0.74rem;
    color: var(--ink-faint);
  }
  .arrow {
    margin-left: 0.35rem;
  }
  .editor-empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.85rem;
    color: var(--ink-faint);
    text-align: center;
    font-size: 0.85rem;
    line-height: 1.7;
  }
  .empty-mark {
    font-family: var(--mono);
    font-size: 1.5rem;
    color: var(--accent);
    animation: blink 1.1s steps(2, start) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
</style>
