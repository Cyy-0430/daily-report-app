import { invoke, Channel } from "@tauri-apps/api/core";

export interface ApiConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
}

export interface CollectConfig {
  /** 启用的采集工具 id,MVP 仅 "claude-code"。 */
  enabledTools: string[];
  /** 仅采集(白名单)的工作目录,空 = 不限。子目录一并包含。 */
  includePaths: string[];
  /** 排除(黑名单)的工作目录,其下会话一律不采集。排除优先于仅采集。 */
  excludePaths: string[];
}

export interface HistoryItem {
  id: string;
  date: string;
  title: string;
  input: string;
  output: string;
  createdAt: number;
}

export interface AppConfig {
  apiConfig: ApiConfig;
  promptTemplate: string;
  customDefaultTemplate: string;
  exportDir: string;
  collectConfig: CollectConfig;
  history: HistoryItem[];
}

export type StreamChunk =
  | { type: "delta"; text: string }
  | { type: "done" }
  | { type: "error"; message: string };

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

/** 路径过滤参数(传给采集命令,基于真实 cwd)。两者均空 = 不过滤。 */
export interface PathFilter {
  /** 仅采集(白名单)路径。 */
  includePaths: string[];
  /** 排除(黑名单)路径。 */
  excludePaths: string[];
}

export function emptyConfig(): AppConfig {
  return {
    apiConfig: { baseUrl: "", apiKey: "", model: "" },
    promptTemplate: "",
    customDefaultTemplate: "",
    exportDir: "",
    collectConfig: {
      enabledTools: ["claude-code"],
      includePaths: [],
      excludePaths: [],
    },
    history: [],
  };
}

export const loadConfig = () => invoke<AppConfig>("load_config");
export const saveConfig = (config: AppConfig) => invoke<void>("save_config", { config });
export const testConnection = (api: ApiConfig) => invoke<string>("test_connection", { api });
export const exportReport = (content: string) => invoke<string | null>("export_report", { content });
export const writeTextFile = (path: string, content: string) =>
  invoke<void>("write_text_file", { path, content });

/**
 * 采集指定日期、指定工具的本地对话记录,并按 filter 做路径过滤。
 * date 为 "YYYY-MM-DD",空串表示今天;filter 传空数组等价于不过滤。
 */
export const collectConversations = (
  date: string,
  tools: string[],
  filter: PathFilter,
) => invoke<CollectResult>("collect_conversations", { date, tools, filter });

/** 流式生成日报。onMessage 在每个分片/完成/错误时回调。 */
export function generateReport(
  input: string,
  conversations: string,
  onMessage: (chunk: StreamChunk) => void,
): Promise<void> {
  const channel = new Channel<StreamChunk>();
  channel.onmessage = onMessage;
  return invoke<void>("generate_report", { input, conversations, onEvent: channel });
}
