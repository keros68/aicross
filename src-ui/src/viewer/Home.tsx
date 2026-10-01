// Home.tsx — 首页「最近的派发」（TASK-40）。直接启动窗口（不带 `open`）时
// 显示；点一条进入查看页，查看页「关闭」回到这里。数据经 list_runs 只读
// run 摘要，不派发、不改文件。

import { useEffect, useState } from 'react';
import { commands, type RunListItem } from '../tauri';
import { runStatusLabel, runStatusTone } from './run-view';
import { formatStartedAt, homeEmptyText, runListItemText } from './run-list';
import './Home.css';

interface Props {
  onOpenRun: (runId: string) => void;
}

export function Home({ onOpenRun }: Props) {
  const [runs, setRuns] = useState<RunListItem[] | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    let cancelled = false;
    const load = () => {
      commands.listRuns()
        .then((items) => {
          if (cancelled) return;
          setRuns(items);
          setError('');
        })
        .catch((err) => {
          if (cancelled) return;
          // 读不出来保留旧数据继续重试；从未成功过才显示空状态文案。
          setRuns((current) => current ?? []);
          setError(String(err));
        });
    };
    load();
    // 派发在宿主里发生，窗口可见时每 5 秒刷一次；不可见（最小化、被系统
    // 判定遮蔽）时暂停，恢复可见立即刷一次（fix-40-r1 R1-10）。终态 run
    // 不变，代价是一次目录读取。
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') load();
    }, 5000);
    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') load();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, []);

  const items = runs ?? [];

  return (
    <div className="home">
      <header className="home-head">
        <h1 className="home-title">最近的派发</h1>
        <p className="home-subtitle">派发在宿主对话里发起；这里看结果、进查看页采纳。</p>
      </header>
      {items.length === 0 ? (
        <div className="home-empty">
          <p>{homeEmptyText()}</p>
          {error !== '' && <p className="home-empty-error">{error}</p>}
        </div>
      ) : (
        <ul className="home-list">
          {items.map((item) => {
            const tone = runStatusTone(item.status);
            return (
              <li key={item.runId}>
                <button
                  type="button"
                  className="home-item"
                  onClick={() => onOpenRun(item.runId)}
                >
                  <span className="home-item-text">{runListItemText(item)}</span>
                  <span className={`home-item-status${tone === 'pill-alert' ? ' alert' : tone === 'pill' ? ' pill' : ''}`}>
                    {runStatusLabel(item.status)}
                  </span>
                  <span className="home-item-time">{formatStartedAt(item.startedAt)}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
