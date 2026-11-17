// 软件版本与更新的视图判定（TASK-88）：把顶栏按钮与浮层的状态文案拆成纯函数，
// 启动自动检查失败时顶栏保持安静，点开浮层才展示失败原因与手动重试入口。

import type { AppUpdateAvailable } from '../../tauri';

export interface AppUpdateModel {
  current: string;
  available: AppUpdateAvailable | null;
  checking: boolean;
  checked: boolean;
  checkError: string;
  progress: string | null;
  installError: string;
}

export interface TriggerView {
  visible: boolean;
  primary: boolean;
  label: string;
  title: string;
}

export function triggerView(model: AppUpdateModel): TriggerView {
  const currentTitle = model.current !== '' ? `当前版本 ${model.current}` : '';
  if (model.progress !== null) {
    return {
      visible: true,
      primary: true,
      label: model.progress,
      title: currentTitle || model.progress,
    };
  }
  if (model.installError !== '') {
    return {
      visible: true,
      primary: true,
      label: '更新失败，重试',
      title: model.installError,
    };
  }
  if (model.available !== null) {
    const label = `更新到 ${model.available.version}`;
    return {
      visible: true,
      primary: true,
      label,
      title: currentTitle || label,
    };
  }
  if (model.current === '') {
    return { visible: false, primary: false, label: '', title: '' };
  }
  return {
    visible: true,
    primary: false,
    label: `v${model.current}`,
    title: '软件版本与更新',
  };
}

export function popoverStatus(model: AppUpdateModel): { text: string; alert: boolean } {
  if (model.available !== null) {
    if (model.installError !== '') return { text: model.installError, alert: true };
    if (model.progress !== null) return { text: model.progress, alert: false };
    if (model.checking) return { text: '正在检查更新…', alert: false };
    if (model.checkError !== '') return { text: model.checkError, alert: true };
    return { text: '', alert: false };
  }
  if (model.checking) return { text: '正在检查更新…', alert: false };
  if (model.checkError !== '') return { text: model.checkError, alert: true };
  if (model.checked) return { text: '已是最新版本', alert: false };
  return { text: '未检查更新', alert: false };
}

export function downloadProgressText(downloaded: number, total: number | null): string {
  if (total !== null && total > 0) {
    const percent = Math.min(100, Math.floor((downloaded / total) * 100));
    return `下载中 ${percent}%`;
  }
  return '下载中…';
}
