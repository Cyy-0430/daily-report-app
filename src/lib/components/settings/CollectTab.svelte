<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { COLLECT_TOOLS } from '$lib/bindings';
  import HelpTip from './HelpTip.svelte';
  import { TPL_CONV } from '$lib/template';

  // 采集设置双向绑定到页面层 $state(切 tab 不丢,保存由页面按页保存负责);
  // defaultPaths 为后端权威默认路径,只读(页面 saveCollect 的 storedPath 也要用)。
  let {
    toolEnabled = $bindable({}),
    includePaths = $bindable([]),
    excludePaths = $bindable([]),
    toolPaths = $bindable({}),
    defaultPaths = {},
  }: {
    toolEnabled?: Record<string, boolean>;
    includePaths?: string[];
    excludePaths?: string[];
    toolPaths?: Record<string, string>;
    defaultPaths?: Record<string, string>;
  } = $props();

  // ---- 路径过滤(排除 / 仅采集)----
  function addExcludePath() {
    excludePaths = [...excludePaths, ''];
  }
  function addIncludePath() {
    includePaths = [...includePaths, ''];
  }
  function removeExcludePath(i: number) {
    excludePaths = excludePaths.filter((_, idx) => idx !== i);
  }
  function removeIncludePath(i: number) {
    includePaths = includePaths.filter((_, idx) => idx !== i);
  }
  async function pickExcludePath(i: number) {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string' && dir) {
      excludePaths[i] = dir;
      excludePaths = [...excludePaths];
    }
  }
  async function pickIncludePath(i: number) {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string' && dir) {
      includePaths[i] = dir;
      includePaths = [...includePaths];
    }
  }
</script>

<!-- 采集工具 -->
<section class="panel sec">
  <div class="sec-title">
    采集工具
    <HelpTip>
      勾选你在用的本地编程工具，生成日报时会自动读取这些工具当天的对话。采集到的对话会作为占位符
      <code class="var">{TPL_CONV}</code> 填入提示词。
    </HelpTip>
  </div>
  {#each COLLECT_TOOLS as t (t.id)}
    <label class="fld fld-check">
      <input type="checkbox" bind:checked={toolEnabled[t.id]} />
      <span>{t.label} · {t.hint}</span>
    </label>
    <div class="path-row tool-path-row">
      <input
        class="field"
        bind:value={toolPaths[t.id]}
        placeholder={t.kind === 'file'
          ? '数据库文件路径,如 ~/.zcode/cli/db/db.sqlite'
          : '数据目录路径,如 ~/.claude/projects'}
      />
      <button
        class="btn btn-ghost btn-sm"
        onclick={() => (toolPaths[t.id] = defaultPaths[t.id] ?? '')}
        disabled={(toolPaths[t.id] ?? '') === (defaultPaths[t.id] ?? '')}
      >
        恢复默认
      </button>
    </div>
  {/each}

  <div class="sub-title">
    路径过滤
    <HelpTip>
      只想采集（或想跳过）某些项目时，在这里按项目目录过滤。子目录会一并纳入；「排除」优先于「仅采集」，被排除的目录绝不会进入日报。两项都可以留空，表示不过滤。
    </HelpTip>
  </div>

  <div class="path-group">
    <div class="path-group-label">排除路径（黑名单）</div>
    {#each excludePaths as _, i (i)}
      <div class="path-row">
        <input class="field" bind:value={excludePaths[i]} placeholder="例如 D:\\aaaa" />
        <button class="btn btn-ghost btn-sm" onclick={() => pickExcludePath(i)}> 选择… </button>
        <button class="btn btn-ghost btn-sm" onclick={() => removeExcludePath(i)}> ✕ </button>
      </div>
    {/each}
    <button class="btn btn-ghost btn-sm path-add" onclick={addExcludePath}> + 添加排除路径 </button>
  </div>

  <div class="path-group">
    <div class="path-group-label">仅采集路径（白名单）</div>
    {#each includePaths as _, i (i)}
      <div class="path-row">
        <input class="field" bind:value={includePaths[i]} placeholder="例如 D:\\work" />
        <button class="btn btn-ghost btn-sm" onclick={() => pickIncludePath(i)}> 选择… </button>
        <button class="btn btn-ghost btn-sm" onclick={() => removeIncludePath(i)}> ✕ </button>
      </div>
    {/each}
    <button class="btn btn-ghost btn-sm path-add" onclick={addIncludePath}>
      + 添加仅采集路径
    </button>
  </div>
</section>

<style>
  .path-group {
    margin-top: 0.6rem;
  }
  .path-group + .path-group {
    margin-top: 0.9rem;
  }
  .path-group-label {
    font-size: 0.78rem;
    color: var(--ink-soft);
    margin-bottom: 0.4rem;
  }
  .path-row {
    display: flex;
    gap: 0.4rem;
    margin-bottom: 0.4rem;
  }
  .path-row .field {
    flex: 1;
  }
  .tool-path-row {
    margin: 0.35rem 0 0.7rem 1.6rem;
  }
  .path-add {
    margin-top: 0.15rem;
  }
</style>
