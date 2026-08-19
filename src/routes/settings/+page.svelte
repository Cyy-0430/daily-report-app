<script lang="ts">
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import {
    loadConfig,
    saveConfig,
    defaultCollectPaths,
    COLLECT_TOOLS,
    type ApiConfig,
  } from '$lib/bindings';
  import { config, notify } from '$lib/store';
  import {
    DEFAULT_PROMPT_TEMPLATE,
    DEFAULT_WEEKLY_MAP_TEMPLATE,
    DEFAULT_WEEKLY_REDUCE_TEMPLATE,
  } from '$lib/template';
  import SettingsTabs from '$lib/components/settings/SettingsTabs.svelte';
  import ApiTab from '$lib/components/settings/ApiTab.svelte';
  import PromptTab from '$lib/components/settings/PromptTab.svelte';
  import CollectTab from '$lib/components/settings/CollectTab.svelte';
  import AboutTab from '$lib/components/settings/AboutTab.svelte';
  // 跨 tab 共享的设置页样式(.sec/.fld/.var 等);导入一次全局生效,仅设置页使用。
  import '$lib/components/settings/settings-shared.css';

  type SettingsTab = 'api' | 'prompt' | 'collect' | 'about';
  const SETTINGS_TABS: { id: SettingsTab; label: string }[] = [
    { id: 'api', label: 'API' },
    { id: 'prompt', label: '提示词' },
    { id: 'collect', label: '采集' },
    { id: 'about', label: '关于' },
  ];

  let activeTab = $state<SettingsTab>('api');
  const activeTabLabel = $derived(SETTINGS_TABS.find((t) => t.id === activeTab)?.label ?? '');
  // 横滑方向:按 SETTINGS_TABS 顺序,前进(下标增大)为 1,后退为 -1。
  // 必须在改 activeTab 之前算好 —— 新面板挂载时 in/out 过渡参数即定型,不能用 $effect 事后追。
  let direction = $state(1);

  function selectTab(id: SettingsTab) {
    const from = SETTINGS_TABS.findIndex((t) => t.id === activeTab);
    const to = SETTINGS_TABS.findIndex((t) => t.id === id);
    direction = to >= from ? 1 : -1;
    activeTab = id;
  }

  // ---- 持久化字段($state 全部留在页面层:tab 组件随 {#if} 卸载,切 tab 后未保存的编辑不丢)----
  let api = $state<ApiConfig>({ baseUrl: '', apiKey: '', model: '' });
  let template = $state(DEFAULT_PROMPT_TEMPLATE);
  // 周报双模板:每日摘要(map) + 整周汇总(reduce)。「自定义默认」随 PromptTab 维护。
  let weeklyMap = $state(DEFAULT_WEEKLY_MAP_TEMPLATE);
  let weeklyReduce = $state(DEFAULT_WEEKLY_REDUCE_TEMPLATE);
  let exportDir = $state('');
  // 各采集工具的勾选状态(按 COLLECT_TOOLS 渲染,id 与 Rust all_collectors() 对齐)。
  let toolEnabled = $state<Record<string, boolean>>({});
  // 路径过滤:排除(黑名单)/ 仅采集(白名单),基于真实工作目录(cwd)。
  let excludePaths = $state<string[]>([]);
  let includePaths = $state<string[]>([]);
  // 各采集工具的数据源路径(可编辑)。defaultPaths = 后端权威默认;
  // toolPaths = 输入框当前值(初值 = 覆盖 ?? 默认,故始终有真实路径)。
  let defaultPaths = $state<Record<string, string>>({});
  let toolPaths = $state<Record<string, string>>({});
  // 关于 tab:自动检查更新开关(运行时版本号/检查中状态随 AboutTab 局部持有)。
  let autoCheckUpdate = $state(true);
  let saving = $state(false);

  onMount(async () => {
    const c = await loadConfig();
    autoCheckUpdate = c.autoCheckUpdate ?? true;
    api = { ...c.apiConfig };
    template = c.promptTemplate || DEFAULT_PROMPT_TEMPLATE;
    weeklyMap = c.weeklyMapTemplate || DEFAULT_WEEKLY_MAP_TEMPLATE;
    weeklyReduce = c.weeklyReduceTemplate || DEFAULT_WEEKLY_REDUCE_TEMPLATE;
    exportDir = c.exportDir;
    const tools = c.collectConfig?.enabledTools ?? [];
    toolEnabled = Object.fromEntries(COLLECT_TOOLS.map((t) => [t.id, tools.includes(t.id)]));
    includePaths = [...(c.collectConfig?.includePaths ?? [])];
    excludePaths = [...(c.collectConfig?.excludePaths ?? [])];
    // 各工具数据源路径:后端给出权威默认(展开 ~),前端用「覆盖 ?? 默认」作为输入框初值。
    defaultPaths = await defaultCollectPaths();
    const saved = c.collectConfig?.toolPaths ?? {};
    toolPaths = Object.fromEntries(
      COLLECT_TOOLS.map((t) => [t.id, (saved[t.id] ?? '').trim() || defaultPaths[t.id] || '']),
    );
  });

  // 按页保存:每次只 load 当前全量配置,overlay 当前 tab 的字段后整份回写。
  // 这样其它 tab 未保存的本地编辑既不会丢失(仍在内存里),也不会被意外写入。
  async function saveApi() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = { ...cur, apiConfig: { ...api }, exportDir };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存 API 与导出设置');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function savePrompt() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = {
        ...cur,
        promptTemplate: template,
        weeklyMapTemplate: weeklyMap,
        weeklyReduceTemplate: weeklyReduce,
      };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存提示词模板');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function saveCollect() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = {
        ...cur,
        collectConfig: {
          enabledTools: COLLECT_TOOLS.filter((t) => toolEnabled[t.id]).map((t) => t.id),
          includePaths: dedupePaths(includePaths),
          excludePaths: dedupePaths(excludePaths),
          toolPaths: Object.fromEntries(COLLECT_TOOLS.map((t) => [t.id, storedPath(t.id)])),
        },
      };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存采集设置');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function saveAbout() {
    saving = true;
    try {
      const cur = await loadConfig();
      const merged = { ...cur, autoCheckUpdate };
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已保存关于设置');
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  async function saveActive() {
    if (activeTab === 'api') return saveApi();
    if (activeTab === 'prompt') return savePrompt();
    if (activeTab === 'about') return saveAbout();
    return saveCollect();
  }

  /** 规整路径列表:去空白、丢空串、去重(保留顺序)。 */
  function dedupePaths(paths: string[]): string[] {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const raw of paths) {
      const s = raw.trim();
      if (!s) continue;
      const key = s.toLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(s);
    }
    return out;
  }

  /** 规整某工具数据源路径为待存值:等于默认或为空 → ""(=用默认,保持配置干净);否则存 trim 后的值。 */
  function storedPath(id: string): string {
    const v = (toolPaths[id] ?? '').trim();
    const d = (defaultPaths[id] ?? '').trim();
    return v && v !== d ? v : '';
  }
</script>

<div class="page-scroll">
  <div class="page-inner">
    <SettingsTabs tabs={SETTINGS_TABS} active={activeTab} onselect={selectTab} />

    <!-- 新旧面板同格叠加(grid-area: 1/1):横滑过渡期间重叠而非并排,不顶动 page-foot。
         fly 默认插值透明度,重叠期两面板交叉淡化,避免不透明卡片叠穿帮。 -->
    <div class="tab-panes">
      {#key activeTab}
        <div
          class="tab-pane"
          in:fly={{ x: direction * 64, duration: 220, easing: cubicOut }}
          out:fly={{ x: direction * -64, duration: 220, easing: cubicOut }}
        >
          {#if activeTab === 'api'}
            <ApiTab bind:api bind:exportDir />
          {:else if activeTab === 'prompt'}
            <PromptTab bind:template bind:weeklyMap bind:weeklyReduce />
          {:else if activeTab === 'collect'}
            <CollectTab
              bind:toolEnabled
              bind:includePaths
              bind:excludePaths
              bind:toolPaths
              {defaultPaths}
            />
          {:else if activeTab === 'about'}
            <AboutTab bind:autoCheckUpdate />
          {/if}
        </div>
      {/key}
    </div>

    <div class="page-foot">
      <button class="btn btn-primary save-btn" onclick={saveActive} disabled={saving}>
        {saving ? '保存中…' : `保存${activeTabLabel}`}
      </button>
    </div>
  </div>
</div>

<style>
  .page-scroll {
    height: 100%;
    overflow: auto;
    /* 滚动条槽常驻:切 tab 时内容高度变化不再引起滚动条出现/消失导致的居中列左右跳动 */
    scrollbar-gutter: stable;
  }
  .page-inner {
    max-width: 720px;
    margin: 0 auto;
    padding: 0 1.5rem 3rem;
  }
  /* 过渡期新旧面板落在同一格叠加,不占两份文档流(容器高度 = max(旧, 新))。
     grid 容器阻断 margin 折叠:末卡片 .sec 的 1rem 下边距留在格内、不再与
     .page-foot 的 0.8rem 上边距折叠,负 margin 令两者相消,净距复原为原 1rem。 */
  .tab-panes {
    display: grid;
    margin-bottom: -0.8rem;
  }
  .tab-pane {
    grid-area: 1 / 1;
  }
  .page-foot {
    position: sticky;
    bottom: 0;
    z-index: 5;
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.6rem;
    margin-top: 0.8rem;
    padding: 0.7rem 0;
    background: var(--paper);
    border-top: 1px solid var(--line);
  }
  .save-btn {
    padding: 0.65rem 1.6rem;
  }
</style>
