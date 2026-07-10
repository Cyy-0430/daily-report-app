import { writable } from "svelte/store";
import { emptyConfig, loadConfig, listHistory, type AppConfig, type HistoryItem } from "./bindings";

/** 全局配置（API / 模板 / 导出目录 / 采集）。历史记录见 `history` store。 */
export const config = writable<AppConfig>(emptyConfig());
export const configLoaded = writable(false);

/** 历史记录(独立于配置,存于 SQLite;按 createdAt 降序)。 */
export const history = writable<HistoryItem[]>([]);

/** 轻量 toast 提示。 */
export const toast = writable<{ kind: "ok" | "err"; msg: string } | null>(null);

/** 历史记录「复用」时，回填到主页输入框的待处理内容。 */
export const pendingInput = writable<string | null>(null);

export function notify(kind: "ok" | "err", msg: string) {
  toast.set({ kind, msg });
  setTimeout(() => toast.set(null), 3000);
}

export async function initConfig() {
  try {
    const [c, h] = await Promise.all([loadConfig(), listHistory()]);
    config.set(c);
    history.set(h);
  } catch (e) {
    notify("err", String(e));
  } finally {
    configLoaded.set(true);
  }
}
