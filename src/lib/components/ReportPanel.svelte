<script lang="ts">
  import { renderMarkdown } from '$lib/markdown';
  import { exportReport, writeTextFile } from '$lib/bindings';
  import { notify } from '$lib/store';
  import { save } from '@tauri-apps/plugin-dialog';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';

  // output/mode 双向绑定(父组件流式写入 output;预览/编辑模式随页面状态保留);
  // busy/label/exportName 只读。
  let {
    output = $bindable(''),
    mode = $bindable('preview'),
    busy = false,
    label = '日报',
    exportName = 'report',
  }: {
    output?: string;
    mode?: 'edit' | 'preview';
    busy?: boolean;
    label?: string;
    exportName?: string;
  } = $props();

  let html = $derived(renderMarkdown(output));

  async function onCopy() {
    if (!output) return;
    try {
      await writeText(output);
      notify('ok', '已复制到剪贴板');
    } catch (e) {
      notify('err', String(e));
    }
  }

  async function onExport() {
    if (!output) return;
    try {
      const saved = await exportReport(output);
      if (saved) {
        notify('ok', `已导出：${saved}`);
        return;
      }
      const path = await save({
        defaultPath: `${exportName}.md`,
        filters: [{ name: 'Markdown', extensions: ['md'] }],
      });
      if (!path) return;
      await writeTextFile(path, output);
      notify('ok', `已导出：${path}`);
    } catch (e) {
      notify('err', String(e));
    }
  }
</script>

<section class="panel">
  <div class="panel-head">
    <span class="panel-label">02 — {mode === 'edit' ? '编辑' : label}</span>
    <div class="head-actions">
      <button
        class="btn btn-ghost btn-sm"
        onclick={() => (mode = mode === 'edit' ? 'preview' : 'edit')}
        disabled={!output || busy}
      >
        {mode === 'edit' ? '预览' : '编辑'}
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
    {#if mode === 'edit'}
      <textarea bind:value={output} class="editor-textarea is-code"></textarea>
    {:else if output}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
      <article
        class="md-body"
        onclick={onCopy}
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onCopy()}
        title="点击复制全部内容"
      >
        {@html html}
      </article>
    {:else}
      <div class="editor-empty">
        <span class="empty-mark">▍</span>
        <p>填写左侧要点或「采集对话」，点「生成{label}」<br />结果会逐字呈现，之后可手动编辑。</p>
      </div>
    {/if}
  </div>

  <div class="panel-foot">
    <span class="meta">{busy ? 'streaming…' : output ? `约 ${output.length} 字` : ''}</span>
  </div>
</section>

<style>
  .panel {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* .panel-head / .panel-label 不覆盖,走 app.css 全局(与原日报右侧面板一致)。 */
  .head-actions {
    display: flex;
    gap: 0.4rem;
  }
  .editor-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    padding: 1.05rem 1.15rem;
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
  .editor-textarea.is-code {
    font-family: var(--mono);
    font-size: 0.84rem;
  }
  .meta {
    font-family: var(--mono);
    font-size: 0.74rem;
    color: var(--ink-faint);
  }
  .md-body {
    cursor: pointer;
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
