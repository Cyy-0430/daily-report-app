<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { get } from 'svelte/store';
  import { GITHUB_URL } from '$lib/app-meta';
  import { notify, config } from '$lib/store';
  import { renderMarkdown } from '$lib/markdown';
  import {
    updateDialog,
    closeUpdateDialog,
    downloadAndInstallWithProgress,
    type DownloadProgress,
  } from '$lib/updater';

  let progress = $state<DownloadProgress | null>(null);
  const busy = $derived(progress !== null);

  const stageLabel = $derived.by(() => {
    if (!progress) return '';
    if (progress.stage === 'downloading') {
      return progress.percent != null ? `下载中… ${Math.round(progress.percent)}%` : '下载中…';
    }
    if (progress.stage === 'installing') return '安装中… 即将重启';
    return '即将重启…';
  });

  async function install() {
    try {
      // 网络代理与检查更新共用 config.apiConfig.proxy:下载(重新 check 取句柄)同代理。
      await downloadAndInstallWithProgress(
        (p) => (progress = p),
        get(config).apiConfig.proxy || undefined,
      );
      // 成功后 relaunch() 已在封装内调用,此行通常不会执行。
    } catch (e) {
      notify('err', `更新失败:${String(e)}`);
      progress = null;
      closeUpdateDialog();
    }
  }
</script>

{#if $updateDialog}
  <div class="overlay" role="dialog" aria-modal="true" aria-label="发现新版本">
    <div class="dialog">
      <div class="dialog-head">
        <span class="new-tag">新版本</span>
        <span class="version">v{$updateDialog.version}</span>
      </div>

      {#if $updateDialog.body}
        <!-- md-body:全局 markdown 样式(app.css);.notes 叠加弹窗内的留白/高度约束。
             body 来自 latest.json notes,经 renderMarkdown 的 DOMPurify 清理后注入。 -->
        <div class="notes md-body">{@html renderMarkdown($updateDialog.body)}</div>
      {/if}

      {#if busy}
        <div class="progress-wrap">
          <div class="progress-track">
            <div
              class="progress-bar"
              style="width: {progress?.percent != null
                ? Math.max(4, Math.min(100, progress.percent))
                : 100}%"
              class:indeterminate={progress?.percent == null}
            ></div>
          </div>
          <span class="progress-label">{stageLabel}</span>
        </div>
      {/if}

      <div class="star-hint">
        如果这个工具对你有帮助，欢迎去 GitHub
        <button class="star-link" onclick={() => openUrl(GITHUB_URL)}>点个 ⭐ Star</button>
      </div>

      <div class="dialog-actions">
        <button class="btn btn-ghost" onclick={closeUpdateDialog} disabled={busy}>稍后</button>
        <button class="btn btn-accent" onclick={install} disabled={busy}>
          {busy ? stageLabel : '立即下载并安装'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    background: rgba(31, 28, 24, 0.42);
    backdrop-filter: blur(2px);
    animation: fade 0.16s ease;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .dialog {
    width: 100%;
    max-width: 440px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--paper-card);
    border: 1px solid var(--line);
    border-radius: 14px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.28);
    overflow: hidden;
    animation: pop 0.18s ease;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  .dialog-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 1.1rem 1.3rem 0.7rem;
  }
  .new-tag {
    font-family: var(--mono);
    font-size: 0.66rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #fffdf7;
    background: var(--accent);
    padding: 0.22rem 0.5rem;
    border-radius: 5px;
  }
  .version {
    font-family: var(--mono);
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--ink);
  }
  .notes {
    padding: 0 1.3rem;
    margin-bottom: 0.9rem;
    font-size: 0.82rem;
    line-height: 1.7;
    color: var(--ink-soft);
    word-break: break-word;
    max-height: 220px;
    overflow: auto;
  }
  /* markdown 首元素(如「## 新增」)不带上边距,贴齐弹窗头部。
     内容经 {@html} 注入,子元素须 :global 才不被 scoped 剪枝。 */
  .notes > :global(:first-child) {
    margin-top: 0;
  }
  .progress-wrap {
    padding: 0 1.3rem;
    margin-bottom: 0.9rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .progress-track {
    height: 7px;
    border-radius: 999px;
    background: var(--line);
    overflow: hidden;
  }
  .progress-bar {
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
    transition: width 0.2s ease;
  }
  .progress-bar.indeterminate {
    width: 35% !important;
    animation: slide 1.1s ease-in-out infinite;
  }
  @keyframes slide {
    0% {
      transform: translateX(-120%);
    }
    100% {
      transform: translateX(320%);
    }
  }
  .progress-label {
    font-size: 0.74rem;
    color: var(--ink-soft);
    font-family: var(--mono);
  }
  .star-hint {
    padding: 0 1.3rem;
    margin-bottom: 0.9rem;
    font-size: 0.78rem;
    color: var(--ink-faint);
  }
  .star-link {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .star-link:hover {
    color: var(--accent-2);
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.9rem 1.3rem;
    border-top: 1px solid var(--line);
    background: var(--paper);
  }
</style>
