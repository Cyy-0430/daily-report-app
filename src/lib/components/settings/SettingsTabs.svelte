<script lang="ts" generics="T extends string">
  // 设置页 tab 导航条(sticky)。active 只读:点击 tab 通过 onselect 回调通知父组件,
  // 由父组件在改 activeTab 之前先算好横滑方向(design §6)。
  let {
    tabs,
    active,
    onselect,
  }: {
    tabs: { id: T; label: string }[];
    active: T;
    onselect: (id: T) => void;
  } = $props();
</script>

<nav class="tabs">
  {#each tabs as t (t.id)}
    <button class="tab" class:active={active === t.id} onclick={() => onselect(t.id)}>
      {t.label}
    </button>
  {/each}
</nav>

<style>
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
</style>
