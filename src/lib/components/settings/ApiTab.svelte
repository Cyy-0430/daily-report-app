<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { testConnection, type ApiConfig } from '$lib/bindings';
  import { notify } from '$lib/store';
  import HelpTip from './HelpTip.svelte';

  // api/exportDir 双向绑定到页面层 $state(切 tab 不丢,保存由页面按页保存负责);
  // showKey/testing 为本 tab 局部 UI 状态。
  let {
    api = $bindable({ baseUrl: '', apiKey: '', model: '', proxy: '' }),
    exportDir = $bindable(''),
  }: {
    api?: ApiConfig;
    exportDir?: string;
  } = $props();

  let showKey = $state(false);
  let testing = $state(false);

  async function test() {
    testing = true;
    try {
      const msg = await testConnection({ ...api });
      notify('ok', msg);
    } catch (e) {
      notify('err', String(e));
    } finally {
      testing = false;
    }
  }

  async function pickDir() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === 'string') exportDir = dir;
  }
</script>

<!-- API 配置 -->
<section class="panel sec">
  <div class="sec-title">
    API 配置
    <HelpTip>
      填写接口地址、模型与密钥即可连接。兼容 OpenAI 接口格式，可接
      DeepSeek、通义千问、Moonshot、本地 Ollama 等。
    </HelpTip>
  </div>
  <div class="grid-2">
    <label class="fld">
      <span>BaseURL</span>
      <input class="field" bind:value={api.baseUrl} placeholder="https://api.openai.com/v1" />
    </label>
    <label class="fld">
      <span>模型</span>
      <input class="field" bind:value={api.model} placeholder="gpt-4o-mini" />
    </label>
  </div>
  <label class="fld">
    <span>API Key</span>
    <div class="row-input">
      <input
        class="field"
        type={showKey ? 'text' : 'password'}
        bind:value={api.apiKey}
        placeholder="sk-..."
      />
      <button class="btn btn-ghost" onclick={() => (showKey = !showKey)}>
        {showKey ? '隐藏' : '显示'}
      </button>
    </div>
  </label>
  <label class="fld">
    <span
      >网络代理<HelpTip>
        代理同时作用于 API 请求与检查更新/下载。支持 host:port（如 127.0.0.1:7890，按 HTTP
        代理处理）或完整 http://… 地址；仅支持 HTTP(S) 代理，留空不代理。
      </HelpTip></span
    >
    <input class="field" bind:value={api.proxy} placeholder="例如 127.0.0.1:7890（留空不代理）" />
  </label>
  <div class="sec-actions">
    <button class="btn btn-ghost" onclick={test} disabled={testing}>
      {testing ? '测试中…' : '测试连接'}
    </button>
  </div>
</section>

<!-- 导出目录 -->
<section class="panel sec">
  <div class="sec-title">
    导出目录
    <HelpTip>
      日报导出时默认存到这里。留空则每次导出时手动选择保存位置；文件名默认为当天日期，如
      2025-08-14.md。
    </HelpTip>
  </div>
  <div class="row-input">
    <input class="field" bind:value={exportDir} placeholder="例如 D:\\Reports" />
    <button class="btn btn-ghost" onclick={pickDir}>选择…</button>
    <button class="btn btn-ghost" onclick={() => (exportDir = '')}>清除</button>
  </div>
</section>

<style>
  .grid-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.9rem;
  }
  .grid-2 .fld {
    margin-bottom: 0.9rem;
  }
  /* 字段标签行内的问号提示与标签文字留一点间距(.help 属 HelpTip 子组件,需 :global) */
  .fld > span :global(.help) {
    margin-left: 0.3rem;
  }
  .row-input {
    display: flex;
    gap: 0.5rem;
  }
  .row-input .field {
    flex: 1;
  }
</style>
