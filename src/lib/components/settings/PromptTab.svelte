<script lang="ts">
  import { onMount } from 'svelte';
  import { loadConfig, saveConfig } from '$lib/bindings';
  import { notify } from '$lib/store';
  import HelpTip from './HelpTip.svelte';
  import {
    DEFAULT_PROMPT_TEMPLATE,
    DEFAULT_WEEKLY_MAP_TEMPLATE,
    DEFAULT_WEEKLY_REDUCE_TEMPLATE,
    TPL_DATE,
    TPL_INPUT,
    TPL_CONV,
    TPL_DATE_RANGE,
    TPL_DAY_SUMMARIES,
  } from '$lib/template';

  // 模板正文双向绑定到页面层 $state(切 tab 不丢,保存由页面按页保存负责)。
  let {
    template = $bindable(''),
    weeklyMap = $bindable(''),
    weeklyReduce = $bindable(''),
  }: {
    template?: string;
    weeklyMap?: string;
    weeklyReduce?: string;
  } = $props();

  // 「自定义默认」是提示词域概念,随本组件维护;「设为默认」即时持久化(loadConfig→改→saveConfig),
  // 不等页面「保存」。本组件挂载晚于页面 onMount,无竞态;tab 反复挂载时重读也无害(磁盘值始终同步)。
  // 恢复默认的回退链:自定义默认 → 内置默认模板。
  let customDefault = $state('');
  let weeklyDefMap = $state('');
  let weeklyDefReduce = $state('');

  onMount(async () => {
    const c = await loadConfig();
    customDefault = c.customDefaultTemplate || '';
    weeklyDefMap = c.weeklyDefaultMapTemplate || '';
    weeklyDefReduce = c.weeklyDefaultReduceTemplate || '';
  });

  async function setAsDefault() {
    try {
      const cur = await loadConfig();
      cur.customDefaultTemplate = template;
      await saveConfig(cur);
      customDefault = template;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function resetTemplate() {
    template = customDefault || DEFAULT_PROMPT_TEMPLATE;
  }

  async function setWeeklyMapDefault() {
    try {
      const cur = await loadConfig();
      cur.weeklyDefaultMapTemplate = weeklyMap;
      await saveConfig(cur);
      weeklyDefMap = weeklyMap;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function resetWeeklyMap() {
    weeklyMap = weeklyDefMap || DEFAULT_WEEKLY_MAP_TEMPLATE;
  }

  async function setWeeklyReduceDefault() {
    try {
      const cur = await loadConfig();
      cur.weeklyDefaultReduceTemplate = weeklyReduce;
      await saveConfig(cur);
      weeklyDefReduce = weeklyReduce;
      notify('ok', '已设为默认');
    } catch (e) {
      notify('err', String(e));
    }
  }

  function resetWeeklyReduce() {
    weeklyReduce = weeklyDefReduce || DEFAULT_WEEKLY_REDUCE_TEMPLATE;
  }
</script>

<!-- 日报模板 -->
<section class="panel sec">
  <div class="sec-title-row">
    <div class="sec-title">
      日报模板
      <HelpTip>
        这份提示词决定日报的写作风格与结构。占位符：<code class="var">{TPL_DATE}</code>
        自动填入今天日期，<code class="var">{TPL_INPUT}</code> 填入你在左侧写的今日要点。
      </HelpTip>
    </div>
    <div class="sec-actions-row">
      <button class="btn btn-ghost btn-sm" onclick={setAsDefault}>设为默认</button>
      <button class="btn btn-ghost btn-sm" onclick={resetTemplate}>恢复默认</button>
    </div>
  </div>
  <textarea bind:value={template} class="field code tmpl"></textarea>
</section>

<!-- 周报模板 -->
<section class="panel sec">
  <div class="sec-title">
    周报模板
    <HelpTip>
      周报分两步生成：第一步用「每日摘要模板」逐日提炼每天的对话，第二步用「整周汇总模板」把每天的摘要归纳成一份完整周报。
    </HelpTip>
  </div>

  <div class="sec-title-row">
    <div class="sub-title">
      每日摘要模板
      <HelpTip>
        用于提炼单日对话的摘要。占位符：<code class="var">{TPL_DATE}</code> 当天日期，<code
          class="var">{TPL_CONV}</code
        >
        当日对话内容。
      </HelpTip>
    </div>
    <div class="sec-actions-row">
      <button class="btn btn-ghost btn-sm" onclick={setWeeklyMapDefault}>设为默认</button>
      <button class="btn btn-ghost btn-sm" onclick={resetWeeklyMap}>恢复默认</button>
    </div>
  </div>
  <textarea bind:value={weeklyMap} class="field code tmpl"></textarea>

  <div class="sec-title-row">
    <div class="sub-title">
      整周汇总模板
      <HelpTip>
        用于把各日摘要汇总成周报。占位符：<code class="var">{TPL_DATE_RANGE}</code>
        本周日期范围，<code class="var">{TPL_INPUT}</code> 你补充的本周要点，<code class="var"
          >{TPL_DAY_SUMMARIES}</code
        >
        各日摘要。
      </HelpTip>
    </div>
    <div class="sec-actions-row">
      <button class="btn btn-ghost btn-sm" onclick={setWeeklyReduceDefault}>设为默认</button>
      <button class="btn btn-ghost btn-sm" onclick={resetWeeklyReduce}>恢复默认</button>
    </div>
  </div>
  <textarea bind:value={weeklyReduce} class="field code tmpl"></textarea>
</section>

<style>
  .tmpl {
    height: 260px;
    line-height: 1.65;
  }
  /* textarea 与下一个标题行(整周汇总)之间补呼吸间距,对齐独立 .sub-title 的 1.1rem 上间距节奏 */
  .tmpl + .sec-title-row {
    margin-top: 1.1rem;
  }
</style>
