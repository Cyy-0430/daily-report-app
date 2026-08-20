import {
  COLLECT_TOOLS,
  DEFAULT_TOOL_IDS,
  type AppConfig,
  type CollectResult,
  type PathFilter,
  type RangeCollectResult,
  type StreamChunk,
} from './bindings';

/**
 * 日报/周报页的工作状态,模块级 $state(应用生命周期单例)。
 * SvelteKit SPA 客户端导航不重新执行模块 → 切到设置/历史页再切回,状态原样保留;
 * 生成中切走,异步闭包继续写这里的字段,回到页面即见累计输出。
 * 注意:.svelte.ts 模块不能导出可重赋值的 $state let,只能导出 const 对象改其属性。
 * 页面只做编排(采集/生成函数),状态的持有权在这里。
 */

/** 日报页(`/`)工作状态。日期在模块加载时取一次「今天」,跨路由不再被重置。 */
export const daily = $state({
  collectDate: todayStr(),
  input: '',
  output: '',
  busy: false,
  collecting: false,
  collectResult: null as CollectResult | null,
  showConversations: false,
  /** ReportPanel 的预览/编辑模式也随页保留。 */
  mode: 'preview' as 'edit' | 'preview',
});

/** 周报 map/reduce 进度分片。 */
export type WeeklyProgress = Extract<StreamChunk, { type: 'progress' }>;

/** 周报页(`/weekly`)工作状态。起止日期在模块加载时取「本周一~今天」。 */
export const weekly = $state({
  startDate: mondayStr(),
  endDate: todayStr(),
  weeklyInput: '',
  output: '',
  busy: false,
  collecting: false,
  /** 当前进度(map/reduce);null = 未在生成。 */
  progress: null as WeeklyProgress | null,
  rangeResult: null as RangeCollectResult | null,
  mode: 'preview' as 'edit' | 'preview',
});

// ---- 两页共享的纯函数(页面用一行 $derived 包装保持响应性) ----

/** 启用的采集工具 id;配置为空时回退默认全部(与采集逻辑一致)。 */
export function enabledToolIdsOf(cfg: AppConfig): string[] {
  const t = cfg.collectConfig?.enabledTools ?? [];
  return t.length ? t : DEFAULT_TOOL_IDS;
}

/** 采集来源标签:依勾选的工具动态展示(id→label),多个用英文逗号隔开。 */
export function sourceLabelOf(ids: string[]): string {
  return ids.map((id) => COLLECT_TOOLS.find((t) => t.id === id)?.label ?? id).join(', ');
}

/** 路径过滤:从配置读取,缺省等价于空规则(不过滤,向后兼容)。 */
export function buildFilter(cfg: AppConfig): PathFilter {
  return {
    includePaths: cfg.collectConfig?.includePaths ?? [],
    excludePaths: cfg.collectConfig?.excludePaths ?? [],
  };
}

/** "YYYY-MM-DD"(本地时区)。 */
export function todayStr(): string {
  return fmt(new Date());
}

/** 本周一(Monday)。 */
export function mondayStr(): string {
  const d = new Date();
  const day = d.getDay(); // 0=Sun..6=Sat
  const diff = day === 0 ? 6 : day - 1;
  const mon = new Date(d);
  mon.setDate(d.getDate() - diff);
  return fmt(mon);
}

function fmt(d: Date): string {
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}
