// Home.tsx — 「派发记录」页（TASK-40；fix-49、fix-51-r1 改为工具行分段筛选 + 数字条 + 白卡表格：
// 时间 / 模式 / 任务 / 状态）。点一行进入查看页，查看页「返回」回到这里。数据经 list_runs 只读 run 摘要，
// 不派发、不改文件。

import { useEffect, useState } from 'react';
import { commands, type RunListItem } from '../tauri';
import { runStatusLabel, runStatusTone } from './run-view';
import {
  RUN_FILTERS,
  filterRuns,
  formatStartedAt,
  homeEmptyText,
  runRowCells,
  runsOverview,
  type RunFilter,
} from './run-list';
import { MODE_ICONS, modeKind } from '../config/mode-kind';
import { Icon } from '../components/common/Icon';
import { PageHeader } from '../components/common/PageHeader';
import { StatCards } from '../components/common/StatCards';
import './Home.css';

interface Props {
  /** 导航切到「派发记录」时为 true；常驻不卸载，只在可见时读取与轮询（TASK-58）。 */
  active: boolean;
  /** 顶栏「刷新」的计数，变化时立即重读。 */
  refreshKey: number;
  /** 刷新触发的读取结束后回报（key 为本次 refreshKey），顶栏据此停止转圈。 */
  onRefreshDone: (key: number, ok: boolean) => void;
  onOpenRun: (runId: string) => void;
}

export function Home({ active, refreshKey, onRefreshDone, onOpenRun }: Props) {
  const [runs, setRuns] = useState<RunListItem[] | null>(null);
  const [error, setError] = useState('');
  const [filter, setFilter] = useState<RunFilter>('all');

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const load = () => commands.listRuns()
      .then((items) => {
        if (cancelled) return true;
        setRuns(items);
        setError('');
        return true;
      })
      .catch((err) => {
        if (cancelled) return false;
        // 读不出来保留旧数据继续重试；从未成功过才显示空状态文案。
        setRuns((current) => current ?? []);
        setError(String(err));
        return false;
      });
    void load().then((ok) => { if (refreshKey > 0) onRefreshDone(refreshKey, ok); });
    // 派发在宿主里发生，窗口可见时每 5 秒刷一次；不可见（最小化、被系统
    // 判定遮蔽）时暂停，恢复可见立即刷一次（fix-40-r1 R1-10）。终态 run
    // 不变，代价是一次目录读取。
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void load();
    }, 5000);
    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') void load();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, [active, refreshKey, onRefreshDone]);

  const items = runs ?? [];
  const shown = filterRuns(items, filter);

  return (
    <div className="page">
      <PageHeader
        left={items.length > 0 && (
          <div className="segmented" role="tablist" aria-label="按状态筛选">
            {RUN_FILTERS.map((option) => (
              <button
                key={option.id}
                type="button"
                role="tab"
                aria-selected={filter === option.id}
                className={`segmented-item${filter === option.id ? ' selected' : ''}`}
                onClick={() => setFilter(option.id)}
              >
                {option.label}
              </button>
            ))}
          </div>
        )}
      />
      <div className="page-body">
        {runs === null ? (
          <div className="home-empty">读取中…</div>
        ) : items.length === 0 ? (
          <div className="home-empty">
            <p>{homeEmptyText()}</p>
            {error !== '' && <p className="home-empty-error">{error}</p>}
          </div>
        ) : (
          <>
            <StatCards items={runsOverview(items)} />
            <section className="card home-card">
              <table className="home-table">
                <thead>
                  <tr>
                    <th className="home-col-time">时间</th>
                    <th className="home-col-mode">模式</th>
                    <th>任务</th>
                    <th className="home-col-status">状态</th>
                  </tr>
                </thead>
                <tbody>
                  {shown.map((item) => {
                    const tone = runStatusTone(item.status);
                    const cells = runRowCells(item);
                    const kind = modeKind('', item.presetName);
                    return (
                      <tr
                        key={item.runId}
                        tabIndex={0}
                        onClick={() => onOpenRun(item.runId)}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault();
                            onOpenRun(item.runId);
                          }
                        }}
                      >
                        <td className="home-col-time">{formatStartedAt(item.startedAt)}</td>
                        <td className="home-col-mode">
                          <span className={`home-mode tone-${kind}`} title={cells.mode}>
                            <Icon name={MODE_ICONS[kind]} size={14} />
                            <span className="home-mode-name">{cells.mode}</span>
                          </span>
                        </td>
                        <td className="home-col-task" title={cells.task}>{cells.task || '—'}</td>
                        <td className="home-col-status">
                          <span className={`pill${tone === 'pill-alert' ? ' alert' : ''}`}>{runStatusLabel(item.status)}</span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
              {shown.length === 0 && <p className="home-filter-empty">无记录</p>}
            </section>
          </>
        )}
      </div>
    </div>
  );
}
