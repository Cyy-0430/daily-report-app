<script lang="ts">
  import { removeHistory, type HistoryItem } from "$lib/bindings";
  import { history, notify, pendingInput } from "$lib/store";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { goto } from "$app/navigation";
  import { renderMarkdown } from "$lib/markdown";

  let expanded = $state<Record<string, boolean>>({});

  async function remove(id: string) {
    try {
      await removeHistory(id);
      history.update((h) => h.filter((x) => x.id !== id));
      notify("ok", "已删除");
    } catch (e) {
      notify("err", String(e));
    }
  }

  function reuse(item: HistoryItem) {
    pendingInput.set(item.input);
    goto("/");
  }

  async function copy(item: HistoryItem) {
    try {
      await writeText(item.output);
      notify("ok", "已复制");
    } catch (e) {
      notify("err", String(e));
    }
  }
</script>

<div class="page-scroll">
  <div class="page-inner">
    <header class="page-head">
      <h1>历史记录</h1>
      <p>共 {$history.length} 条 · 生成后自动保存</p>
    </header>

    {#if !$history.length}
      <div class="empty-state">
        <span class="empty-mark">∅</span>
        <p>还没有记录<br />生成日报后会自动保存到这里</p>
      </div>
    {:else}
      <ul class="hist-list">
        {#each $history as item (item.id)}
          <li class="panel hist-item">
            <div class="hist-row">
              <div class="hist-meta">
                <div class="hist-title">{item.title}</div>
                <div class="hist-date">{item.date}</div>
              </div>
              <div class="hist-actions">
                <button class="btn btn-ghost btn-sm" onclick={() => reuse(item)}>复用</button>
                <button class="btn btn-ghost btn-sm" onclick={() => copy(item)}>复制</button>
                <button
                  class="btn btn-ghost btn-sm"
                  onclick={() => (expanded[item.id] = !expanded[item.id])}
                >
                  {expanded[item.id] ? "收起" : "查看"}
                </button>
                <button
                  class="btn btn-ghost btn-sm danger"
                  onclick={() => remove(item.id)}
                >
                  删除
                </button>
              </div>
            </div>

            {#if expanded[item.id]}
              <div class="hist-detail">
                <div class="detail-label">输入</div>
                <pre class="detail-pre">{item.input}</pre>
                <div class="detail-label">日报</div>
                <article class="md-body">{@html renderMarkdown(item.output)}</article>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .page-scroll {
    height: 100%;
    overflow: auto;
  }
  .page-inner {
    max-width: 760px;
    margin: 0 auto;
    padding: 2rem 1.5rem 3rem;
  }
  .page-head {
    margin-bottom: 1.5rem;
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
    font-family: var(--mono);
  }
  .empty-state {
    border: 1px dashed var(--line-strong);
    border-radius: 12px;
    padding: 3.5rem 1rem;
    text-align: center;
    color: var(--ink-faint);
  }
  .empty-mark {
    font-family: var(--mono);
    font-size: 1.8rem;
    color: var(--accent);
    display: block;
    margin-bottom: 0.8rem;
  }
  .empty-state p {
    margin: 0;
    line-height: 1.7;
    font-size: 0.88rem;
  }
  .hist-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  .hist-item {
    padding: 0.9rem 1.1rem;
  }
  .hist-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }
  .hist-title {
    font-weight: 600;
    font-size: 0.95rem;
    color: var(--ink);
  }
  .hist-date {
    font-family: var(--mono);
    font-size: 0.74rem;
    color: var(--ink-faint);
    margin-top: 0.15rem;
  }
  .hist-actions {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
  }
  .danger {
    color: var(--accent);
  }
  .danger:hover:not(:disabled) {
    background: rgba(156, 58, 38, 0.08);
    border-color: var(--accent);
    color: var(--accent);
  }
  .hist-detail {
    margin-top: 0.9rem;
    padding-top: 0.9rem;
    border-top: 1px solid var(--line);
  }
  .detail-label {
    font-family: var(--mono);
    font-size: 0.7rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-faint);
    margin-bottom: 0.4rem;
  }
  .detail-pre {
    margin: 0 0 1rem;
    white-space: pre-wrap;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 0.7rem 0.85rem;
    font-size: 0.8rem;
    color: var(--ink-soft);
    line-height: 1.6;
  }
</style>
