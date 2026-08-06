import { invoke, Channel } from "@tauri-apps/api/core";

export interface ApiConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
}

export interface CollectConfig {
  /** 启用的采集工具 id,默认 "claude-code"、"zcode"、"codex" 与 "opencode"。 */
  enabledTools: string[];
  /** 仅采集(白名单)的工作目录,空 = 不限。子目录一并包含。 */
  includePaths: string[];
  /** 排除(黑名单)的工作目录,其下会话一律不采集。排除优先于仅采集。 */
  excludePaths: string[];
  /** 各采集工具的自定义数据源路径(覆盖默认)。键=工具 id,值=路径;空串/缺失=用默认。 */
  toolPaths: Record<string, string>;
}

export interface HistoryItem {
  id: string;
  date: string;
  title: string;
  input: string;
  output: string;
  createdAt: number;
}

/** 应用配置(历史记录已独立存于 SQLite,见 listHistory/addHistory/removeHistory)。 */
export interface AppConfig {
  apiConfig: ApiConfig;
  promptTemplate: string;
  customDefaultTemplate: string;
  /** 周报 map(每日摘要)提示词;空串 = 用内置默认。 */
  weeklyMapTemplate: string;
  /** 周报 reduce(整周汇总)提示词;空串 = 用内置默认。 */
  weeklyReduceTemplate: string;
  /** 周报 map 模板的「自定义默认」(设置页「设为默认」写入;空 = 恢复时回退内置默认)。 */
  weeklyDefaultMapTemplate: string;
  /** 周报 reduce 模板的「自定义默认」。 */
  weeklyDefaultReduceTemplate: string;
  exportDir: string;
  collectConfig: CollectConfig;
}

export type StreamChunk =
  | { type: "delta"; text: string }
  | { type: "done" }
  | { type: "error"; message: string }
  | {
      type: "progress";
      stage: "map" | "reduce";
      current: number;
      total: number;
      message: string;
    };

export interface ConversationLine {
  ts: string;
  role: "user" | "assistant";
  text: string;
  tools: string[];
}

export interface SessionDigest {
  tool: string;
  project: string;
  cwd: string | null;
  sessionId: string;
  startedAt: string;
  endedAt: string;
  lineCount: number;
  estTokens: number;
  lines: ConversationLine[];
}

export interface CollectResult {
  sessions: SessionDigest[];
  renderedText: string;
  estTokens: number;
  skippedLines: number;
}

/** 区间采集的单日结果(周报 map 的一个批次)。 */
export interface DayCollect {
  /** "YYYY-MM-DD"。 */
  date: string;
  sessions: SessionDigest[];
  /** 当日渲染后的对话文本(喂给 map 摘要)。 */
  renderedText: string;
  estTokens: number;
}

/** 区间采集结果:逐日明细 + 总 token(供 /weekly 预览/预算,不耗 LLM)。 */
export interface RangeCollectResult {
  /** 按日期升序;无对话的日期也保留(estTokens=0)。 */
  days: DayCollect[];
  totalTokens: number;
  skippedLines: number;
}

/** 路径过滤参数(传给采集命令,基于真实 cwd)。两者均空 = 不过滤。 */
export interface PathFilter {
  /** 仅采集(白名单)路径。 */
  includePaths: string[];
  /** 排除(黑名单)路径。 */
  excludePaths: string[];
}

/** 可用的采集工具(id 与 Rust `all_collectors()` 对齐,单一来源)。 */
export const COLLECT_TOOLS: {
  id: string;
  label: string;
  hint: string;
  /** 数据源类型:dir=扫描目录下的会话文件;file=打开单个 SQLite db 文件。仅展示用。 */
  kind: "dir" | "file";
}[] = [
  { id: "claude-code", label: "Claude Code", hint: "~/.claude/projects", kind: "dir" },
  { id: "zcode", label: "ZCode", hint: "~/.zcode/cli/db", kind: "file" },
  { id: "codex", label: "Codex", hint: "~/.codex/sessions", kind: "dir" },
  { id: "opencode", label: "Opencode", hint: "~/.local/share/opencode", kind: "file" },
];

export function emptyConfig(): AppConfig {
  return {
    apiConfig: { baseUrl: "", apiKey: "", model: "" },
    promptTemplate: "",
    customDefaultTemplate: "",
    weeklyMapTemplate: "",
    weeklyReduceTemplate: "",
    weeklyDefaultMapTemplate: "",
    weeklyDefaultReduceTemplate: "",
    exportDir: "",
    collectConfig: {
      enabledTools: ["claude-code", "zcode", "codex", "opencode"],
      includePaths: [],
      excludePaths: [],
      toolPaths: {},
    },
  };
}

export const loadConfig = () => invoke<AppConfig>("load_config");
export const saveConfig = (config: AppConfig) => invoke<void>("save_config", { config });
export const testConnection = (api: ApiConfig) => invoke<string>("test_connection", { api });
export const exportReport = (content: string) => invoke<string | null>("export_report", { content });
export const writeTextFile = (path: string, content: string) =>
  invoke<void>("write_text_file", { path, content });

/** 各采集工具数据源的默认路径(已展开 ~),供设置页展示与「恢复默认」。键=工具 id。 */
export const defaultCollectPaths = () =>
  invoke<Record<string, string>>("default_collect_paths");

/**
 * 采集指定日期、指定工具的本地对话记录,并按 filter 做路径过滤。
 * date 为 "YYYY-MM-DD",空串表示今天;filter 传空数组等价于不过滤;
 * toolPaths 为各工具的自定义数据源路径(覆盖默认),键缺失/空串=用默认。
 */
export const collectConversations = (
  date: string,
  tools: string[],
  filter: PathFilter,
  toolPaths: Record<string, string>,
) => invoke<CollectResult>("collect_conversations", { date, tools, filter, toolPaths });

/**
 * 采集区间(含首尾)内逐日的对话记录。逐日单日切片采集,每日一个 DayCollect
 * (周报 map 的一个批次)。仅本地 IO,无 LLM。
 * start/end 为 "YYYY-MM-DD",空串表示今天;end<start 时后端自动交换。
 */
export const collectConversationsRange = (
  start: string,
  end: string,
  tools: string[],
  filter: PathFilter,
  toolPaths: Record<string, string>,
) =>
  invoke<RangeCollectResult>("collect_conversations_range", {
    start,
    end,
    tools,
    filter,
    toolPaths,
  });

/** 历史记录(独立于配置,存于 SQLite)。 */
export const listHistory = () => invoke<HistoryItem[]>("list_history");
export const addHistory = (item: HistoryItem) => invoke<void>("add_history", { item });
export const removeHistory = (id: string) => invoke<void>("remove_history", { id });

/** 流式生成日报;成功时返回已保存的 HistoryItem。onMessage 在每个分片/完成/错误时回调。 */
export function generateReport(
  input: string,
  conversations: string,
  onMessage: (chunk: StreamChunk) => void,
): Promise<HistoryItem> {
  const channel = new Channel<StreamChunk>();
  channel.onmessage = onMessage;
  return invoke<HistoryItem>("generate_report", { input, conversations, onEvent: channel });
}

/**
 * 流式生成周报(map-reduce):区间采集→逐日摘要(map)→整周汇总(reduce)。
 * onMessage 回调 delta(最终周报逐字)、progress(正在摘要第 X/N 天 / 正在汇总)、
 * done、error。成功时返回已保存的 HistoryItem。
 */
export function generateWeeklyReport(
  start: string,
  end: string,
  tools: string[],
  filter: PathFilter,
  toolPaths: Record<string, string>,
  weeklyInput: string,
  onMessage: (chunk: StreamChunk) => void,
): Promise<HistoryItem> {
  const channel = new Channel<StreamChunk>();
  channel.onmessage = onMessage;
  return invoke<HistoryItem>("generate_weekly_report", {
    start,
    end,
    tools,
    filter,
    toolPaths,
    weeklyInput,
    onEvent: channel,
  });
}
