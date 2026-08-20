<script lang="ts">
  import HelpTip from './HelpTip.svelte';
  import TemplateEditor from './TemplateEditor.svelte';
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
  // 三段编辑区的标题/设为默认/恢复默认/textarea 已收敛为 TemplateEditor。
  let {
    template = $bindable(''),
    weeklyMap = $bindable(''),
    weeklyReduce = $bindable(''),
  }: {
    template?: string;
    weeklyMap?: string;
    weeklyReduce?: string;
  } = $props();
</script>

<!-- 日报模板 -->
<section class="panel sec">
  <TemplateEditor
    title="日报模板"
    bind:value={template}
    configKey="customDefaultTemplate"
    builtinDefault={DEFAULT_PROMPT_TEMPLATE}
  >
    {#snippet help()}
      <HelpTip>
        这份提示词决定日报的写作风格与结构。占位符：<code class="var">{TPL_DATE}</code>
        自动填入今天日期，<code class="var">{TPL_INPUT}</code> 填入你在左侧写的今日要点。
      </HelpTip>
    {/snippet}
  </TemplateEditor>
</section>

<!-- 周报模板 -->
<section class="panel sec">
  <div class="sec-title">
    周报模板
    <HelpTip>
      周报分两步生成：第一步用「每日摘要模板」逐日提炼每天的对话，第二步用「整周汇总模板」把每天的摘要归纳成一份完整周报。
    </HelpTip>
  </div>

  <TemplateEditor
    title="每日摘要模板"
    variant="sub"
    bind:value={weeklyMap}
    configKey="weeklyDefaultMapTemplate"
    builtinDefault={DEFAULT_WEEKLY_MAP_TEMPLATE}
  >
    {#snippet help()}
      <HelpTip>
        用于提炼单日对话的摘要。占位符：<code class="var">{TPL_DATE}</code> 当天日期，<code
          class="var">{TPL_CONV}</code
        >
        当日对话内容。
      </HelpTip>
    {/snippet}
  </TemplateEditor>

  <TemplateEditor
    title="整周汇总模板"
    variant="sub"
    bind:value={weeklyReduce}
    configKey="weeklyDefaultReduceTemplate"
    builtinDefault={DEFAULT_WEEKLY_REDUCE_TEMPLATE}
  >
    {#snippet help()}
      <HelpTip>
        用于把各日摘要汇总成周报。占位符：<code class="var">{TPL_DATE_RANGE}</code>
        本周日期范围，<code class="var">{TPL_INPUT}</code> 你补充的本周要点，<code class="var"
          >{TPL_DAY_SUMMARIES}</code
        >
        各日摘要。
      </HelpTip>
    {/snippet}
  </TemplateEditor>
</section>
