<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { getVersion } from '@tauri-apps/api/app';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { notify, config } from '$lib/store';
  import { APP_NAME, APP_NAME_EN, APP_AUTHOR, GITHUB_URL } from '$lib/app-meta';
  import { checkForUpdate, openUpdateDialog } from '$lib/updater';
  import HelpTip from './HelpTip.svelte';

  // autoCheckUpdate 双向绑定到页面层 $state(保存由页面按页保存负责);
  // appVersion/checking 为本 tab 局部状态。
  let { autoCheckUpdate = $bindable(true) }: { autoCheckUpdate?: boolean } = $props();

  let appVersion = $state('');
  let checking = $state(false);

  onMount(() => {
    getVersion().then((v) => (appVersion = v));
  });

  async function checkUpdateManual() {
    checking = true;
    try {
      const info = await checkForUpdate(get(config).apiConfig.proxy || undefined);
      if (info.available && info.version) {
        openUpdateDialog({ version: info.version, body: info.body });
      } else {
        notify('ok', '已是最新版本');
      }
    } catch (e) {
      notify('err', `检查更新失败:${String(e)}`);
    } finally {
      checking = false;
    }
  }
</script>

<!-- 应用信息 -->
<section class="panel sec about-card">
  <div class="about-head">
    <span class="about-mark" aria-hidden="true"></span>
    <div class="about-title">
      <span class="about-name">{APP_NAME}</span>
      <span class="about-name-en">{APP_NAME_EN}</span>
    </div>
  </div>
  <dl class="about-meta">
    <div class="meta-row">
      <dt>版本</dt>
      <dd class="mono">v{appVersion || '—'}</dd>
    </div>
    <div class="meta-row">
      <dt>作者</dt>
      <dd>{APP_AUTHOR}</dd>
    </div>
    <div class="meta-row">
      <dt>仓库</dt>
      <dd>
        <button class="link-btn" onclick={() => openUrl(GITHUB_URL)}>
          {GITHUB_URL} ↗
        </button>
      </dd>
    </div>
  </dl>
  <div class="star-line">
    如果这个工具对你有帮助，欢迎在 GitHub
    <button class="star-inline" onclick={() => openUrl(GITHUB_URL)}>点个 ⭐ Star</button>
  </div>
</section>

<!-- 更新 -->
<section class="panel sec">
  <div class="sec-title">
    更新
    <HelpTip>
      开启后每次打开应用会静默检查一次新版本，有更新时弹出对话框。也可随时点「立即检查更新」手动检查。更新包经签名校验，下载安装后自动重启。
    </HelpTip>
  </div>
  <label class="toggle-row">
    <span class="toggle-text">
      <span class="toggle-label">启动时自动检查更新</span>
      <span class="toggle-sub">每次打开应用时静默检查新版本（默认开启）</span>
    </span>
    <button
      class="switch"
      class:on={autoCheckUpdate}
      role="switch"
      aria-checked={autoCheckUpdate}
      aria-label="启动时自动检查更新"
      onclick={() => (autoCheckUpdate = !autoCheckUpdate)}
    >
      <span class="knob"></span>
    </button>
  </label>
  <div class="sec-actions">
    <button class="btn btn-ghost" onclick={checkUpdateManual} disabled={checking}>
      {checking ? '检查中…' : '立即检查更新'}
    </button>
  </div>
</section>

<style>
  .about-card {
    align-items: stretch;
  }
  .about-head {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    margin-bottom: 1.1rem;
  }
  .about-mark {
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    background: var(--accent);
    border-radius: 8px;
    transform: rotate(45deg);
    box-shadow: 0 0 0 4px rgba(156, 58, 38, 0.12);
  }
  .about-title {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
  }
  .about-name {
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--ink);
  }
  .about-name-en {
    font-family: var(--mono);
    font-size: 0.78rem;
    letter-spacing: 0.12em;
    color: var(--ink-faint);
  }
  .about-meta {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }
  .meta-row {
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
    font-size: 0.84rem;
  }
  .meta-row dt {
    width: 3rem;
    flex-shrink: 0;
    font-size: 0.76rem;
    color: var(--ink-faint);
    letter-spacing: 0.04em;
  }
  .meta-row dd {
    margin: 0;
    color: var(--ink-soft);
    word-break: break-all;
  }
  .meta-row .mono {
    font-family: var(--mono);
    color: var(--ink);
  }
  .link-btn {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 0.82rem;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .link-btn:hover {
    color: var(--accent-2);
  }
  .star-line {
    margin-top: 1rem;
    padding-top: 0.85rem;
    border-top: 1px dashed var(--line);
    font-size: 0.78rem;
    color: var(--ink-faint);
  }
  .star-inline {
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
  .star-inline:hover {
    color: var(--accent-2);
  }
  /* 开关 */
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.4rem 0;
    cursor: pointer;
  }
  .toggle-text {
    display: flex;
    flex-direction: column;
    gap: 0.18rem;
  }
  .toggle-label {
    font-size: 0.86rem;
    font-weight: 600;
    color: var(--ink);
  }
  .toggle-sub {
    font-size: 0.74rem;
    color: var(--ink-faint);
  }
  .switch {
    flex-shrink: 0;
    width: 42px;
    height: 24px;
    border-radius: 999px;
    border: 1px solid var(--line-strong);
    background: var(--paper);
    position: relative;
    cursor: pointer;
    transition:
      background 0.18s,
      border-color 0.18s;
  }
  .switch.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--paper-card);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
    transition: transform 0.18s ease;
  }
  .switch.on .knob {
    transform: translateX(18px);
  }
</style>
