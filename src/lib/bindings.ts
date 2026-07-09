import { invoke, Channel } from "@tauri-apps/api/core";

export interface ApiConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
}

export interface CollectConfig {
  /** 启用的采集工具 id,MVP 仅 "claude-code"。 */
  enabledTools: string[];
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

export function emptyConfig(): AppConfig {
  return {
    apiConfig: { baseUrl: "", apiKey: "", model: "" },
    promptTemplate: "",
    exportDir: "",
    collectConfig: { enabledTools: ["claude-code"] },
    history: [],
  };
}

export const loadConfig = () => invoke<AppConfig>("load_config");
export const saveConfig = (config: AppConfig) => invoke<void>("save_config", { config });
export const testConnection = (api: ApiConfig) => invoke<string>("test_connection", { api });
export const exportReport = (content: string) => invoke<string | null>("export_report", { content });
export const writeTextFile = (path: string, content: string) =>
  invoke<void>("write_text_file", { path, content });

/** 采集指定日期、指定工具的本地对话记录。date 为 "YYYY-MM-DD",空串表示今天。 */
export const collectConversations = (date: string, tools: string[]) =>
  invoke<CollectResult>("collect_conversations", { date, tools });

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
