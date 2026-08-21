<script lang="ts">
  import { get } from 'svelte/store';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { saveConfig, type AppConfig, type CustomTheme } from '$lib/bindings';
  import { config, configLoaded, notify } from '$lib/store';
  import { theme } from '$lib/theme-state.svelte';
  import {
    THEME_VAR_GROUPS,
    applyTheme,
    dedupeName,
    exportThemeJson,
    nextThemeName,
    resolveColors,
    type ThemeColors,
  } from '$lib/theme';
  import HelpTip from './HelpTip.svelte';
  import ThemeDropdown from './ThemeDropdown.svelte';
  import ImportThemeDialog from './ImportThemeDialog.svelte';
  import ColorPicker from '$lib/components/ColorPicker.svelte';

  /**
   * 主题 tab:三个 section(当前主题 / 颜色定制 / 操作区)。
   * 编辑现场(draft)与预览(preview)在模块级 $state(跨 tab/路由保留);
   * 选中/预览/保存/重命名/删除的编排与即时持久化全部在本组件
   * (get config → 改 → saveConfig → config.set,不回读;域内即时持久化随域组件走)。
   */

  let saving = $state(false);

  // draft 首次进入时以激活主题初始化;同步初始化(组件脚本先于首帧,避免色块闪空),
  // $effect 兜底竞态(config 未加载完就进设置页的极端情况)。
  function initDraft() {
    const cfg = get(config);
    theme.draft = { baseId: cfg.themeConfig.activeId, colors: resolveColors(cfg.themeConfig) };
  }
  if (!theme.draft && get(configLoaded)) initDraft();
  $effect(() => {
    if ($configLoaded && !theme.draft) initDraft();
  });

  // ---- 编排(交互表见 design §5) ----

  /** 下拉选中(含预设, id=''):立即切换激活并持久化;同时载入编辑器;预览结束。 */
  async function selectTheme(id: string) {
    const base = get(config);
    const merged: AppConfig = { ...base, themeConfig: { ...base.themeConfig, activeId: id } };
    try {
      await saveConfig(merged);
      config.set(merged);
    } catch (e) {
      notify('err', String(e));
      return;
    }
    const colors = resolveColors(merged.themeConfig, id);
    applyTheme(colors);
    theme.preview = null; // 选中其他主题 = 用户明确结束预览
    theme.draft = { baseId: id, colors };
  }

  /** 调色:只改 draft,不触全局。 */
  function setVar(key: string, hex: string) {
    if (theme.draft) theme.draft.colors[key] = hex;
  }

  /** 单项重置:回所选主题基线。 */
  function resetVar(key: string) {
    if (!theme.draft) return;
    theme.draft.colors[key] = resolveColors(get(config).themeConfig, theme.draft.baseId)[key];
  }

  /** 预览:当前编辑值临时应用全局(仅内存,不落盘,跨路由保持)。 */
  function startPreview() {
    if (!theme.draft) return;
    theme.preview = { ...theme.draft.colors };
    applyTheme(theme.preview);
  }

  /** 结束预览:回到已保存激活主题。 */
  function endPreview() {
    theme.preview = null;
    applyTheme(resolveColors(get(config).themeConfig));
  }

  /** 保存:总是新建主题(自动命名),立即启用;预设不可变。 */
  async function saveTheme() {
    if (!theme.draft || saving) return;
    saving = true;
    try {
      const base = get(config);
      const id = crypto.randomUUID();
      const item: CustomTheme = {
        id,
        name: nextThemeName(base.themeConfig.custom),
        colors: { ...theme.draft.colors },
      };
      const merged: AppConfig = {
        ...base,
        themeConfig: { activeId: id, custom: [...base.themeConfig.custom, item] },
      };
      await saveConfig(merged);
      config.set(merged);
      applyTheme(item.colors); // 新主题转正,视觉无跳变
      theme.preview = null;
      theme.draft = { baseId: id, colors: { ...item.colors } };
      notify('ok', `已保存并启用「${item.name}」`);
    } catch (e) {
      notify('err', String(e));
    } finally {
      saving = false;
    }
  }

  /** 重命名(下拉 ✎ 行内提交):trim 后空 → 保持原名;重名 → 自动后缀。 */
  async function renameTheme(id: string, name: string) {
    const trimmed = name.trim();
    if (!trimmed) return; // 空名保持原名(ThemeDropdown 已挡,双保险)
    const base = get(config);
    const others = base.themeConfig.custom.filter((t) => t.id !== id);
    const final = dedupeName(trimmed, others);
    const custom = base.themeConfig.custom.map((t) => (t.id === id ? { ...t, name: final } : t));
    const merged: AppConfig = { ...base, themeConfig: { ...base.themeConfig, custom } };
    try {
      await saveConfig(merged);
      config.set(merged);
      notify('ok', '已重命名');
    } catch (e) {
      notify('err', String(e));
    }
  }

  /** 删除(下拉 🗑):删的是激活主题 → 回落预设并持久化。 */
  async function deleteTheme(id: string) {
    const base = get(config);
    const wasActive = base.themeConfig.activeId === id;
    const custom = base.themeConfig.custom.filter((t) => t.id !== id);
    const activeId = wasActive ? '' : base.themeConfig.activeId;
    const merged: AppConfig = { ...base, themeConfig: { activeId, custom } };
    try {
      await saveConfig(merged);
      config.set(merged);
    } catch (e) {
      notify('err', String(e));
      return;
    }
    if (wasActive) {
      // 回落预设:等同一次「选中预设」,应用并结束预览;draft 基线若是被删主题也回落。
      applyTheme(resolveColors(merged.themeConfig));
      theme.preview = null;
    }
    if (theme.draft?.baseId === id) {
      theme.draft = {
        baseId: merged.themeConfig.activeId,
        colors: resolveColors(merged.themeConfig),
      };
    }
    notify('ok', '已删除');
  }

  /** 导出(下拉 ⤓):序列化为分享 JSON 并复制剪贴板;预设不可导出(下拉无该按钮)。 */
  async function exportTheme(id: string) {
    const t = get(config).themeConfig.custom.find((x) => x.id === id);
    if (!t) return;
    try {
      await writeText(exportThemeJson(t));
      notify('ok', `已复制「${t.name}」主题 JSON`);
    } catch (e) {
      notify('err', String(e));
    }
  }

  // ---- 导入(弹窗) ----

  let importOpen = $state(false);

  /**
   * 导入(弹窗回调):新 UUID + 名称取 JSON(重名 dedupeName 去重),入库并立即启用。
   * 编排对齐 saveTheme,仅名称来源不同 —— 语义不同,并存不复用。
   */
  async function importTheme(p: { name: string; colors: ThemeColors }) {
    const base = get(config);
    const id = crypto.randomUUID();
    const item: CustomTheme = {
      id,
      name: dedupeName(p.name, base.themeConfig.custom),
      colors: p.colors, // 存原始色板(不补全):缺 key 由 resolveColors 兜底预设
    };
    const merged: AppConfig = {
      ...base,
      themeConfig: { activeId: id, custom: [...base.themeConfig.custom, item] },
    };
    try {
      await saveConfig(merged);
      config.set(merged);
    } catch (e) {
      notify('err', String(e));
      return;
    }
    const colors = resolveColors(merged.themeConfig);
    applyTheme(colors);
    theme.preview = null;
    theme.draft = { baseId: id, colors };
    notify('ok', `已导入并启用「${item.name}」`);
  }
</script>

<!-- 当前主题 -->
<section class="panel sec">
  <div class="sec-title-row">
    <div class="sec-title">
      当前主题
      <HelpTip>
        下拉选中即应用并保存(重启仍是它)。自定义主题悬停可重命名 ✎、导出 ⤓(复制 JSON 分享)、删除
        🗑;「导入主题」粘贴他人分享的 JSON 即新建并启用;删除正在使用的主题会自动回到预设。
      </HelpTip>
    </div>
    <button class="btn btn-ghost btn-sm" onclick={() => (importOpen = true)}>导入主题</button>
  </div>
  <ThemeDropdown
    activeId={$config.themeConfig.activeId}
    custom={$config.themeConfig.custom}
    onselect={selectTheme}
    onrename={renameTheme}
    onexport={exportTheme}
    ondel={deleteTheme}
  />
</section>

<!-- 颜色定制 -->
<section class="panel sec">
  <div class="sec-title">
    颜色定制
    <HelpTip>
      点击色块打开调色盘,可拖动取色或键入 HEX /
      RGB;「重置」恢复为当前所选主题的基线值。修改后先「预览」看效果,满意再「保存为主题」。
    </HelpTip>
  </div>
  {#each THEME_VAR_GROUPS as g (g.group)}
    <div class="sub-title">{g.group}</div>
    {#each g.vars as v (v.key)}
      <div class="var-row">
        <ColorPicker
          label={`调整「${v.label}」颜色`}
          value={theme.draft?.colors[v.key] ?? ''}
          onchange={(hex) => setVar(v.key, hex)}
        />
        <div class="var-info">
          <span class="var-label">
            {v.label}<code class="var-key">--{v.key}</code>
          </span>
          <span class="var-desc">{v.desc}</span>
        </div>
        <span class="var-hex">{theme.draft?.colors[v.key] ?? ''}</span>
        <button class="btn btn-ghost btn-sm reset-btn" onclick={() => resetVar(v.key)}>
          重置
        </button>
      </div>
    {/each}
  {/each}
</section>

<!-- 操作区 -->
<section class="panel sec">
  {#if theme.preview}
    <div class="preview-banner" role="status">
      <span class="banner-text">预览中 — 临时配色,关闭应用后自动恢复为已保存主题。</span>
      <button class="btn btn-ghost btn-sm" onclick={endPreview}>结束预览</button>
    </div>
  {/if}
  <div class="actions">
    <button class="btn btn-ghost" onclick={startPreview} disabled={saving}>预览</button>
    <button class="btn btn-primary" onclick={saveTheme} disabled={saving}>
      {saving ? '保存中…' : '保存为主题'}
    </button>
  </div>
  <p class="hint">
    「保存」会把当前颜色存为新的自定义主题(自动命名)并立即启用,可在下拉中重命名;「预览」仅临时生效,不写入配置。
  </p>
</section>

{#if importOpen}
  <ImportThemeDialog onimport={importTheme} onclose={() => (importOpen = false)} />
{/if}

<style>
  .var-row {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.42rem 0;
  }
  .var-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .var-label {
    font-size: 0.84rem;
    font-weight: 600;
    color: var(--ink);
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
  }
  .var-key {
    font-family: var(--mono);
    font-size: 0.68rem;
    color: var(--ink-faint);
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0.05rem 0.3rem;
    font-weight: 400;
  }
  .var-desc {
    font-size: 0.72rem;
    color: var(--ink-faint);
  }
  .var-hex {
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--ink-soft);
    width: 4.4rem;
    text-align: right;
    flex-shrink: 0;
  }
  .reset-btn {
    flex-shrink: 0;
  }
  .preview-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.8rem;
    margin-bottom: 0.85rem;
    padding: 0.6rem 0.85rem;
    border: 1px dashed var(--accent);
    border-radius: 8px;
    background: var(--paper);
    font-size: 0.78rem;
    color: var(--ink-soft);
  }
  .banner-text {
    min-width: 0;
  }
  .actions {
    display: flex;
    gap: 0.6rem;
  }
  .hint {
    margin: 0.7rem 0 0;
    font-size: 0.74rem;
    line-height: 1.6;
    color: var(--ink-faint);
  }
</style>
