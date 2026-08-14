/**
 * 更新检测与安装封装(基于 @tauri-apps/plugin-updater + plugin-process)。
 *
 * 设计要点:
 * - 检测/下载/安装全在前端(updater 插件把能力直接暴露给 JS),无需 Rust 命令。
 * - `checkForUpdate()` 把 `check()` 的 `Update | null` 投影成纯数据 `UpdateInfo`,任何
 *   底层抛错都向上抛(由调用方决定静默或 toast)。
 * - `updateDialog` 是全局单例 store:启动自动检查与「关于」tab 的手动检查都复用
 *   同一个 UpdateDialog(挂在 +layout.svelte)。
 * - 安装时重新 `check()` 取一个新鲜的 Update 句柄(展示用的那个可能已被关闭),
 *   再 `downloadAndInstall()` → `relaunch()`(遵循 Tauri 官方示例)。
 */
import { writable } from 'svelte/store';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

/** 检测结果投影(纯数据,便于放入 store)。 */
export interface UpdateInfo {
  available: boolean;
  version?: string;
  body?: string;
}

/** 下载/安装进度。percent 在下载阶段无总大小时为 undefined。 */
export interface DownloadProgress {
  stage: 'downloading' | 'installing' | 'done';
  percent?: number;
}

/** 全局更新弹窗状态:null = 关闭。 */
export const updateDialog = writable<{ version: string; body?: string } | null>(null);

export function openUpdateDialog(info: { version: string; body?: string }) {
  updateDialog.set(info);
}

export function closeUpdateDialog() {
  updateDialog.set(null);
}

/**
 * 检查更新。无更新返回 `{ available: false }`;有更新返回版本号与更新说明。
 * 底层错误向上抛(调用方决定静默或提示)。
 */
export async function checkForUpdate(): Promise<UpdateInfo> {
  const update = await check();
  if (!update) return { available: false };
  return { available: true, version: update.version, body: update.body };
}

/**
 * 下载并安装更新,完成后重启应用。`onProgress` 回调驱动 UI 进度条。
 * 安装时重新 check() 以取得新鲜的 Update 句柄。
 */
export async function downloadAndInstallWithProgress(
  onProgress?: (p: DownloadProgress) => void,
): Promise<void> {
  const update = await check();
  if (!update) throw new Error('没有可用的更新');
  let downloaded = 0;
  let total: number | undefined;
  await update.downloadAndInstall((e) => {
    if (e.event === 'Started') {
      total = e.data.contentLength;
      downloaded = 0;
      onProgress?.({ stage: 'downloading', percent: total ? 0 : undefined });
    } else if (e.event === 'Progress') {
      downloaded += e.data.chunkLength;
      onProgress?.({
        stage: 'downloading',
        percent: total ? Math.min(100, (downloaded / total) * 100) : undefined,
      });
    } else if (e.event === 'Finished') {
      onProgress?.({ stage: 'installing' });
    }
  });
  // 平台已安装(Windows NSIS 安装器会接管);其它平台需手动重启。
  onProgress?.({ stage: 'done' });
  await relaunch();
}
