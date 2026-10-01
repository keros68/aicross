// App.tsx — TASK-40 下线其余界面后的窗口外壳：自绘标题栏 + 缩放边条 +
// 首页「最近的派发」与查看页两页切换。`aicross open <run_id>` 的冷启动
// 请求随 argv 走（takePendingOpen 取一次），热启动走单实例插件转发的
// 'open-run-request' 事件（带递增序号，同一 run 重开也要刷新）。

import { useEffect, useState } from 'react';
import { commands, isTauri } from './tauri';
import { TitleBar } from './components/common/TitleBar';
import { ResizeEdges } from './components/common/ResizeEdges';
import { Home } from './viewer/Home';
import { RunViewer } from './viewer/RunViewer';

export function App() {
  // 查看页请求：runId + 递增序号（fix-37-r1 R1-9，同一个 run 再次 open 也
  // 要刷新）。null = 显示首页。
  const [viewerRequest, setViewerRequest] = useState<{ runId: string; seq: number } | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    commands.takePendingOpen()
      .then((runId) => { if (runId) setViewerRequest({ runId, seq: 0 }); })
      .catch(() => {});
    // R1-10：卸载可能早于异步注册完成——disposed 标志在注销完成时补拉销。
    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const fn = await listen<{ runId: string; seq: number }>('open-run-request', (event) => {
          setViewerRequest({ runId: event.payload.runId, seq: event.payload.seq });
        });
        if (disposed) fn();
        else unlisten = fn;
      } catch { /* event bridge unavailable (e.g. browser dev) — ignore */ }
    })();
    return () => { disposed = true; unlisten?.(); };
  }, []);

  return (
    <>
      <TitleBar />
      {viewerRequest === null ? (
        <Home onOpenRun={(runId) => setViewerRequest({ runId, seq: 0 })} />
      ) : (
        <RunViewer
          key={viewerRequest.runId}
          runId={viewerRequest.runId}
          refreshSeq={viewerRequest.seq}
          onClose={() => setViewerRequest(null)}
        />
      )}
      <ResizeEdges />
    </>
  );
}
