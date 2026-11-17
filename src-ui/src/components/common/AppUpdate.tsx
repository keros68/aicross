// 软件版本与更新（TASK-65、TASK-88）：顶栏常驻当前版本号（如 v0.3.11），启动时自动查一次
// GitHub Release 的 latest.json；有新版时顶栏切换为「更新到 X」。点击可展开浮层查看当前版本、
// 检查结果或失败原因，并可随时手动「检查更新」或下载校验安装新版。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri, type AppUpdateAvailable } from '../../tauri';
import { downloadProgressText, popoverStatus, triggerView } from './app-update-view';
import { useDismiss } from './use-dismiss';

export function AppUpdateButton() {
  const [current, setCurrent] = useState('');
  const [available, setAvailable] = useState<AppUpdateAvailable | null>(null);
  const [checking, setChecking] = useState(false);
  const [checked, setChecked] = useState(false);
  const [checkError, setCheckError] = useState('');
  const [progress, setProgress] = useState<string | null>(null);
  const [installError, setInstallError] = useState('');
  const [open, setOpen] = useState(false);

  const rootRef = useRef<HTMLDivElement | null>(null);
  const mountedRef = useRef(true);
  const busyRef = useRef<'check' | 'install' | null>(null);

  const close = useCallback(() => setOpen(false), []);
  useDismiss(open, rootRef, close);

  const runCheck = useCallback(async () => {
    if (!isTauri || busyRef.current !== null) return;
    busyRef.current = 'check';
    setChecking(true);
    setCheckError('');
    setInstallError('');
    try {
      const status = await commands.appUpdateCheck();
      if (!mountedRef.current) return;
      if (status.current !== '') setCurrent(status.current);
      setAvailable(status.available);
      setChecked(true);
    } catch (err) {
      if (!mountedRef.current) return;
      setCheckError(String(err));
      setChecked(true);
    } finally {
      if (busyRef.current === 'check') busyRef.current = null;
      if (mountedRef.current) setChecking(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    if (!isTauri) return () => { mountedRef.current = false; };
    commands.appVersion()
      .then((version) => {
        if (mountedRef.current && version !== '') setCurrent(version);
      })
      .catch(() => {});
    void runCheck();
    return () => { mountedRef.current = false; };
  }, [runCheck]);

  const model = { current, available, checking, checked, checkError, progress, installError };
  const trigger = triggerView(model);
  const status = popoverStatus(model);

  if (!trigger.visible) return null;

  const install = async () => {
    if (busyRef.current !== null) return;
    busyRef.current = 'install';
    setInstallError('');
    setProgress('准备下载…');
    let unlisten: (() => void) | undefined;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<{ downloaded: number; total: number | null }>('app-update-progress', ({ payload }) => {
        if (mountedRef.current) setProgress(downloadProgressText(payload.downloaded, payload.total));
      });
    } catch { /* 没有进度也照常更新 */ }
    try {
      await commands.appUpdateInstall();
    } catch (err) {
      if (mountedRef.current) {
        setInstallError(String(err));
        setProgress(null);
      }
    } finally {
      unlisten?.();
      if (busyRef.current === 'install') busyRef.current = null;
    }
  };

  return (
    <div className="app-update" ref={rootRef}>
      <button
        type="button"
        className={trigger.primary ? 'btn primary topbar-update' : `topbar-version${open ? ' open' : ''}`}
        title={trigger.title}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        {trigger.label}
      </button>
      {open && (
        <div className="app-update-pop" role="dialog" aria-label="软件版本与更新">
          <div className="app-update-title">
            当前版本 <span className="app-update-mono">{current || '未知'}</span>
          </div>
          {available !== null && (
            <div className="app-update-body">
              可更新到 <span className="app-update-mono">{available.version}</span>。下载并校验后安装，安装时窗口会关闭，装完自动重新打开；数据目录不受影响。
              {available.notes.trim() !== '' && <div className="update-notes">{available.notes}</div>}
            </div>
          )}
          {status.text !== '' && (
            <div className={`app-update-status${status.alert ? ' alert' : ''}`} role="status">
              {status.text}
            </div>
          )}
          <div className="app-update-actions">
            <button
              type="button"
              className="btn"
              disabled={checking || progress !== null}
              onClick={() => void runCheck()}
            >
              {checking ? '检查中…' : '检查更新'}
            </button>
            {available !== null && (
              <button
                type="button"
                className="btn primary"
                disabled={progress !== null}
                onClick={() => void install()}
              >
                {progress !== null ? '更新中…' : '更新'}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
