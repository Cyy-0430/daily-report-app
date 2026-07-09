import { invoke, Channel } from "@tauri-apps/api/core";

export interface ApiConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
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
  history: HistoryItem[];
}

export type StreamChunk =
  | { type: "delta"; text: string }
  | { type: "done" }
  | { type: "error"; message: string };

export function emptyConfig(): AppConfig {
  return { apiConfig: { baseUrl: "", apiKey: "", model: "" }, promptTemplate: "", exportDir: "", history: [] };
}

export const loadConfig = () => invoke<AppConfig>("load_config");
export const saveConfig = (config: AppConfig) => invoke<void>("save_config", { config });
export const testConnection = (api: ApiConfig) => invoke<string>("test_connection", { api });
export const exportReport = (content: string) => invoke<string | null>("export_report", { content });
export const writeTextFile = (path: string, content: string) =>
  invoke<void>("write_text_file", { path, content });

/** 流式生成日报。onMessage 在每个分片/完成/错误时回调。 */
export function generateReport(input: string, onMessage: (chunk: StreamChunk) => void): Promise<void> {
  const channel = new Channel<StreamChunk>();
  channel.onmessage = onMessage;
  return invoke<void>("generate_report", { input, onEvent: channel });
}
