import { writable } from "svelte/store";
import { emptyConfig, loadConfig, type AppConfig } from "./bindings";

/** 全局配置（API / 模板 / 导出目录 / 历史）。 */
export const config = writable<AppConfig>(emptyConfig());
export const configLoaded = writable(false);

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
    const c = await loadConfig();
    config.set(c);
  } catch (e) {
    notify("err", String(e));
  } finally {
    configLoaded.set(true);
  }
}
