<script lang="ts">
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { loadConfig, saveConfig } from '$lib/bindings';
  import { notify } from '$lib/store';

  // 提示词模板编辑区(日报模板 / 每日摘要模板 / 整周汇总模板三段同构):
  // 标题行(variant 区分主/子标题)+ 设为默认/恢复默认 + textarea。
  // 正文双向绑定到 PromptTab(页面 $state);「设为默认」是即时持久化(loadConfig→改→saveConfig),
  // 不等页面「保存」。本组件挂载晚于页面 onMount,无竞态;tab 反复挂载时重读也无害(磁盘值始终同步)。
  // help snippet 内容按父组件作用域编译,里面的 <code class="var"> 等类留在全局 settings-shared.css。
  let {
    title,
    variant = 'sec',
    value = $bindable(''),
    configKey,
    builtinDefault,
    help,
  }: {
    title: string;
    /** 'sec'=.sec-title 主标题 | 'sub'=.sub-title 子标题。 */
    variant?: 'sec' | 'sub';
    value?: string;
    /** 「设为默认」写入的 AppConfig 字段。 */
    configKey: 'customDefaultTemplate' | 'weeklyDefaultMapTemplate' | 'weeklyDefaultReduceTemplate';
    /** 内置默认模板(恢复默认回退链的兜底)。 */
    builtinDefault: string;
    /** 标题旁的 HelpTip 提示,由父组件注入。 */
    help?: Snippet;
  } = $props();

  // 恢复默认的回退链:自定义默认 → 内置默认模板。
  let customDefault = $state('');

  onMount(async () => {
    const c = await loadConfig();
    customDefault = c[configKey] || '';
  });

  async function setAsDefault() {
    try {
      const cur = await loadConfig();
      cur[configKey] = value;
      await saveConfig(cur);
      customDefault = value;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function reset() {
    value = customDefault || builtinDefault;
  }
</script>

<div class="sec-title-row">
  <div class={variant === 'sec' ? 'sec-title' : 'sub-title'}>
    {title}
    {#if help}{@render help()}{/if}
  </div>
  <div class="sec-actions-row">
    <button class="btn btn-ghost btn-sm" onclick={setAsDefault}>设为默认</button>
    <button class="btn btn-ghost btn-sm" onclick={reset}>恢复默认</button>
  </div>
</div>
<textarea bind:value class="field code tmpl"></textarea>

<style>
  .tmpl {
    height: 260px;
    line-height: 1.65;
  }
  /* textarea 与下一个标题行(整周汇总)之间的 1.1rem 呼吸间距在 settings-shared.css:
     该相邻关系跨两个 TemplateEditor 实例(每日摘要 textarea + 整周汇总标题行),
     组件内静态分析判为 unused 并剪除,scoped 写法不生效。 */
</style>
