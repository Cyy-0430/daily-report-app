<script lang="ts">
  import { parseThemeJson, type ThemeColors } from '$lib/theme';
  import { notify } from '$lib/store';

  /**
   * 导入主题弹窗(overlay 模式对齐 UpdateDialog):粘贴分享 JSON,
   * parseThemeJson 校验失败 → toast 报错且保留内容供修改;成功 → onimport 并自清空关闭。
   * 落盘/启用编排不在本组件(ThemeTab.importTheme);Esc / 点遮罩 / 取消 = onclose。
   */

  let {
    onimport,
    onclose,
  }: {
    /** 校验通过的主题载荷(名称已 trim,色板已过白名单 + hex 校验)。 */
    onimport: (payload: { name: string; colors: ThemeColors }) => void;
    onclose: () => void;
  } = $props();

  let text = $state('');
  let overlayEl: HTMLDivElement | undefined = $state();

  /** 占位示例放在脚本常量:模板属性值里的字面 { 会被 Svelte 当表达式起始。 */
  const PLACEHOLDER =
    '粘贴分享的主题 JSON,如 {"name":"夜读","colors":{"paper":"#101010","ink":"#f0eae0"}}';

  /** 挂载即聚焦,便于直接粘贴。 */
  function focusTextarea(el: HTMLTextAreaElement) {
    el.focus();
  }

  function confirmImport() {
    const payload = parseThemeJson(text);
    if (!payload) {
      notify('err', 'JSON 格式不正确,请检查后重试');
      return;
    }
    onimport(payload);
    text = ''; // 自清空后关闭(下次打开是干净状态)
    onclose();
  }

  // 遮罩点击(target 是遮罩自身 = 未点进卡片)与 Esc 都走 onclose;
  // 监听挂 window 而非遮罩 div,规避非交互元素 click 的 a11y 警告(同 ThemeDropdown 外点关闭)。
  function onWindowClick(e: MouseEvent) {
    if (e.target === overlayEl) onclose();
  }
  function onWindowKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onWindowKey} />

<div class="overlay" bind:this={overlayEl} role="dialog" aria-modal="true" aria-label="导入主题">
  <div class="dialog">
    <div class="dialog-title">导入主题</div>
    <div class="dialog-body">
      <label class="fld" for="import-theme-json">
        <span>主题 JSON</span>
        <textarea
          id="import-theme-json"
          class="field code"
          bind:value={text}
          rows="7"
          spellcheck="false"
          placeholder={PLACEHOLDER}
          use:focusTextarea></textarea>
      </label>
      <p class="hint">仅需 name 与 colors 两个字段;缺失的颜色导入后回退预设值,多余字段忽略。</p>
    </div>
    <div class="dialog-actions">
      <button class="btn btn-ghost" onclick={onclose}>取消</button>
      <button class="btn btn-primary" onclick={confirmImport} disabled={!text.trim()}>导入</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    /* 令牌化遮罩:预设下等效 rgba(31,28,24,0.42),跟随自定义 ink 换色 */
    background: color-mix(in srgb, var(--ink) 42%, transparent);
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
    max-width: 460px;
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
  .dialog-title {
    padding: 1.1rem 1.3rem 0.2rem;
    font-size: 0.98rem;
    font-weight: 650;
    color: var(--ink);
  }
  .dialog-body {
    padding: 0.8rem 1.3rem 0;
  }
  .dialog-body .fld {
    margin-bottom: 0.55rem;
  }
  .hint {
    margin: 0 0 1rem;
    font-size: 0.74rem;
    line-height: 1.6;
    color: var(--ink-faint);
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
