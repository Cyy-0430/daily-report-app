<script lang="ts" module>
  /** 挂载即聚焦全选,便于直接键入新名。 */
  export function focusInput(el: HTMLInputElement) {
    el.focus();
    el.select();
  }
</script>

<script lang="ts">
  import type { CustomTheme } from '$lib/bindings';
  import { PRESET_NAME } from '$lib/theme';

  /**
   * 主题下拉(设置页主题 tab 专用):预设行(不可改删)+ 自定义行(悬停 ✎/⤓/🗑、行内重命名)。
   * 纯展示 + 回调:选中/重命名/导出/删除的编排与持久化在 ThemeTab(回调 props 约定)。
   */
  let {
    activeId,
    custom,
    onselect,
    onrename,
    onexport,
    ondel,
  }: {
    /** 当前激活主题 id;'' = 预设「纸墨」。 */
    activeId: string;
    custom: CustomTheme[];
    /** 选中即激活;id = '' 表示预设。 */
    onselect: (id: string) => void;
    /** 重命名提交(trim 后空名不回调,保持原名由本组件处理)。 */
    onrename: (id: string, name: string) => void;
    /** 导出:序列化为分享 JSON 并复制剪贴板(剪贴板调用在 ThemeTab)。 */
    onexport: (id: string) => void;
    ondel: (id: string) => void;
  } = $props();

  let open = $state(false);
  /** 处于行内重命名的主题 id;null = 无。 */
  let renamingId = $state<string | null>(null);
  let renameText = $state('');
  let rootEl: HTMLDivElement | undefined = $state();

  const activeName = $derived(
    (activeId ? custom.find((t) => t.id === activeId)?.name : undefined) ?? PRESET_NAME,
  );

  function toggle() {
    open = !open;
    renamingId = null;
  }
  function pick(id: string) {
    open = false;
    renamingId = null;
    onselect(id);
  }
  function startRename(t: CustomTheme) {
    renamingId = t.id;
    renameText = t.name;
  }
  function commitRename(id: string) {
    if (renamingId !== id) return; // 已被 Esc 取消
    const name = renameText.trim();
    if (name) onrename(id, name); // 空名 → 保持原名,不提交
    renamingId = null;
  }
  function cancelRename() {
    renamingId = null;
  }
  function del(t: CustomTheme, e: MouseEvent) {
    e.stopPropagation();
    ondel(t.id);
  }

  function onWindowClick(e: MouseEvent) {
    if (open && rootEl && e.target instanceof Node && !rootEl.contains(e.target)) {
      open = false;
      renamingId = null;
    }
  }
  function onWindowKey(e: KeyboardEvent) {
    if (renamingId) {
      // 重命名中的 Esc = 取消重命名(输入框随后被移除而失焦,guard 拦住提交);其余按键放行
      if (e.key === 'Escape') cancelRename();
      return;
    }
    if (open && e.key === 'Escape') open = false;
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={onWindowKey} />

<div class="dd" bind:this={rootEl}>
  <button
    type="button"
    class="field trigger"
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={toggle}
  >
    <span class="trigger-name">{activeName}</span>
    {#if !activeId}<span class="preset-tag">预设</span>{/if}
    <svg class="chev" class:open viewBox="0 0 10 6" aria-hidden="true" width="10" height="6"
      ><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" /></svg
    >
  </button>

  {#if open}
    <div class="menu" role="listbox" aria-label="主题列表">
      <!-- 预设行:不可改删 -->
      <div
        class="row"
        role="option"
        aria-selected={activeId === ''}
        tabindex="0"
        onclick={() => pick('')}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            pick('');
          }
        }}
      >
        <span class="row-name" class:active={activeId === ''}>{PRESET_NAME}</span>
        <span class="preset-tag">预设</span>
      </div>

      {#each custom as t (t.id)}
        {#if renamingId === t.id}
          <!-- 行内重命名:Enter/失焦提交,Esc 取消(空名保持原名) -->
          <div class="row rename-row">
            <input
              class="field rename-input"
              type="text"
              bind:value={renameText}
              aria-label="重命名主题"
              use:focusInput
              onblur={() => commitRename(t.id)}
              onkeydown={(e) => {
                if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
              }}
            />
          </div>
        {:else}
          <div
            class="row"
            role="option"
            aria-selected={activeId === t.id}
            tabindex="0"
            onclick={() => pick(t.id)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                pick(t.id);
              }
            }}
          >
            <span class="row-name" class:active={activeId === t.id}>{t.name}</span>
            <span class="row-acts">
              <button
                type="button"
                class="act"
                title="重命名"
                aria-label="重命名 {t.name}"
                onclick={(e) => {
                  e.stopPropagation();
                  startRename(t);
                }}
              >
                <svg viewBox="0 0 14 14" aria-hidden="true" width="13" height="13">
                  <path
                    d="M9.7 2.2l2.1 2.1M2 12l1-3.5 6.7-6.7 2.5 2.5-6.7 6.7L2 12z"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
              <button
                type="button"
                class="act"
                title="导出"
                aria-label="导出 {t.name}"
                onclick={(e) => {
                  e.stopPropagation();
                  onexport(t.id);
                }}
              >
                <svg viewBox="0 0 14 14" aria-hidden="true" width="13" height="13">
                  <path
                    d="M5.2 5.2V3h6v6H9M3 5.2h6v6H3z"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
              <button
                type="button"
                class="act del"
                title="删除"
                aria-label="删除 {t.name}"
                onclick={(e) => del(t, e)}
              >
                <svg viewBox="0 0 14 14" aria-hidden="true" width="13" height="13">
                  <path
                    d="M3 4.5h8M5.5 4.5V3h3v1.5M4.2 4.5l.5 7h4.6l.5-7"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
            </span>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .dd {
    position: relative;
  }
  .trigger {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    text-align: left;
    cursor: pointer;
    font-size: 0.86rem;
  }
  .trigger-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chev {
    flex-shrink: 0;
    color: var(--ink-faint);
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 40;
    padding: 0.35rem;
    background: var(--paper-card);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.14);
    max-height: 260px;
    overflow: auto;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.48rem 0.6rem;
    border-radius: 7px;
    cursor: pointer;
    font-size: 0.84rem;
    color: var(--ink-soft);
  }
  .row:hover,
  .row:focus-visible {
    background: var(--paper);
    color: var(--ink);
    outline: none;
  }
  .row-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-name.active {
    color: var(--accent);
    font-weight: 600;
  }
  .preset-tag {
    flex-shrink: 0;
    font-family: var(--mono);
    font-size: 0.62rem;
    letter-spacing: 0.08em;
    color: var(--ink-faint);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0.08rem 0.32rem;
  }
  .row-acts {
    display: flex;
    gap: 0.15rem;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .row:hover .row-acts,
  .row:focus-visible .row-acts,
  .row:focus-within .row-acts {
    opacity: 1;
  }
  .act {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--ink-faint);
    cursor: pointer;
    transition:
      background 0.12s,
      color 0.12s;
  }
  .act:hover {
    background: var(--paper-card);
    color: var(--ink);
  }
  .act.del:hover {
    color: var(--accent);
  }
  .rename-row {
    cursor: default;
    padding: 0.28rem 0.35rem;
  }
  .rename-input {
    padding: 0.38rem 0.55rem;
    font-size: 0.84rem;
  }
</style>
