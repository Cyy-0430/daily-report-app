<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { initConfig, toast } from "$lib/store";

  let { children } = $props();
  const appWindow = getCurrentWindow();

  const nav = [
    { href: "/", label: "生成" },
    { href: "/settings", label: "设置" },
    { href: "/history", label: "历史" },
  ];

  onMount(() => {
    initConfig();
  });
</script>

<div class="app-shell">
  <header class="topbar" data-tauri-drag-region>
    <div class="topbar-left" data-tauri-drag-region>
      <a href="/" class="brand" aria-label="首页">
        <span class="brand-mark" aria-hidden="true"></span>
        <span class="brand-name">DAILY<span class="brand-sub">· 日报生成</span></span>
      </a>
      <nav class="nav">
        {#each nav as item}
          <a
            href={item.href}
            class="nav-link"
            class:active={$page.url.pathname === item.href}
          >
            {item.label}
          </a>
        {/each}
      </nav>
    </div>

    <div class="win-ctrls">
      <button
        class="win-btn"
        title="最小化"
        aria-label="最小化"
        onclick={() => appWindow.minimize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true"><rect x="1.5" y="4.6" width="7" height="1" fill="currentColor" /></svg>
      </button>
      <button
        class="win-btn"
        title="最大化"
        aria-label="最大化"
        onclick={() => appWindow.toggleMaximize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true"><rect x="1.6" y="1.6" width="6.8" height="6.8" rx="1" fill="none" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
      <button
        class="win-btn close"
        title="关闭"
        aria-label="关闭"
        onclick={() => appWindow.close()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" stroke-width="1.2" fill="none" /></svg>
      </button>
    </div>
  </header>

  <main class="app-main">
    {@render children()}
  </main>

  {#if $toast}
    <div class="toast {$toast.kind}">
      <span class="toast-dot"></span>
      {$toast.msg}
    </div>
  {/if}
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 0.6rem 0 1.5rem;
    height: 56px;
    border-bottom: 1px solid var(--line);
    background: rgba(255, 253, 247, 0.7);
    backdrop-filter: blur(8px);
    flex-shrink: 0;
  }
  .topbar-left {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    height: 100%;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    text-decoration: none;
    color: var(--ink);
  }
  .brand-mark {
    width: 13px;
    height: 13px;
    background: var(--accent);
    border-radius: 3px;
    transform: rotate(45deg);
    box-shadow: 0 0 0 3px rgba(156, 58, 38, 0.12);
  }
  .brand-name {
    font-family: var(--mono);
    font-weight: 700;
    letter-spacing: 0.2em;
    font-size: 0.85rem;
    display: flex;
    align-items: baseline;
    gap: 0.55rem;
  }
  .brand-sub {
    font-family: var(--sans);
    font-weight: 500;
    letter-spacing: 0.04em;
    font-size: 0.78rem;
    color: var(--ink-soft);
  }
  .nav {
    display: flex;
    gap: 0.2rem;
  }
  .nav-link {
    font-size: 0.85rem;
    color: var(--ink-soft);
    text-decoration: none;
    padding: 0.4rem 0.85rem;
    border-radius: 7px;
    transition: all 0.15s;
  }
  .nav-link:hover {
    color: var(--ink);
    background: rgba(31, 28, 24, 0.05);
  }
  .nav-link.active {
    color: var(--ink);
    font-weight: 600;
    background: rgba(31, 28, 24, 0.07);
  }
  .win-ctrls {
    display: flex;
    gap: 0.1rem;
  }
  .win-btn {
    width: 38px;
    height: 34px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    cursor: pointer;
    color: var(--ink-soft);
    border-radius: 7px;
    transition: background 0.15s, color 0.15s;
  }
  .win-btn svg {
    width: 11px;
    height: 11px;
  }
  .win-btn:hover {
    background: rgba(31, 28, 24, 0.08);
    color: var(--ink);
  }
  .win-btn.close:hover {
    background: var(--accent);
    color: #fffdf7;
  }
  .app-main {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .toast {
    position: fixed;
    top: 4.5rem;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--ink);
    color: var(--paper-card);
    padding: 0.6rem 1.1rem;
    border-radius: 999px;
    font-size: 0.82rem;
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.2);
    z-index: 50;
    animation: toast-in 0.22s ease;
  }
  .toast.err {
    background: var(--accent);
  }
  .toast-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.85;
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translate(-50%, -10px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }
</style>
