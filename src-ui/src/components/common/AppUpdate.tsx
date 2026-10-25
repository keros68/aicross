// 软件更新：启动时查一次 GitHub 上的最新版本，有新版时顶栏出现「更新到 X」，确认后下载、
// 校验签名并安装。查不到（离线、没发布更新清单）时什么也不显示。

import { useEffect, useState } from 'react';
import { commands, isTauri, type AppUpdateInfo } from '../../tauri';
import { ConfirmDialog } from './ConfirmDialog';

export function AppUpdateButton() {
  const [update, setUpdate] = useState<AppUpdateInfo | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!isTauri) return;
    commands.appUpdateCheck().then(setUpdate, () => {});
  }, []);

  if (update === null) return null;

  const install = async () => {
    setConfirming(false);
    setError('');
    setProgress('准备下载…');
    let unlisten: (() => void) | undefined;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<{ downloaded: number; total: number | null }>('app-update-progress', ({ payload }) => {
        setProgress(payload.total ? `下载中 ${Math.floor((payload.downloaded / payload.total) * 100)}%` : '下载中…');
      });
    } catch { /* 没有进度也照常更新 */ }
    try {
      await commands.appUpdateInstall();
    } catch (err) {
      setError(String(err));
      setProgress(null);
    }
    unlisten?.();
  };

  return (
    <>
      <button
        type="button"
        className="btn primary topbar-update"
        disabled={progress !== null}
        title={error !== '' ? error : `当前 ${update.current}`}
        onClick={() => setConfirming(true)}
      >
        {progress ?? (error !== '' ? '更新失败，重试' : `更新到 ${update.version}`)}
      </button>
      {confirming && (
        <ConfirmDialog
          title={`更新到 aicross ${update.version}`}
          confirmLabel="更新"
          onConfirm={() => void install()}
          onCancel={() => setConfirming(false)}
        >
          当前版本 {update.current}。下载并校验后安装，安装时窗口会关闭，装完自动重新打开；数据目录不受影响。
          {update.notes.trim() !== '' && <div className="update-notes">{update.notes}</div>}
        </ConfirmDialog>
      )}
    </>
  );
}
