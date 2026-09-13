// RightPanel.tsx — 右栏 tab 壳：「任务」（现有 TaskBoard，内部任务/修改记录
// 两个 tab 不动）与「方案与编排」（第 2 期 OrchestrationPanel）。
// .compiler-top wrapper styling lives in global.css (same selector,
// kept around because changing the wrapper class would touch every
// theme rule that scopes to it).

import { useEffect, useState } from 'react';
import { TaskBoard } from './TaskBoard';
import { OrchestrationPanel } from './orchestration/OrchestrationPanel';
import './Compiler.css';

type RightTab = 'tasks' | 'orchestration';

export function RightPanel() {
  // 外层 tab 存 localStorage（照 TaskBoard 的 'cc-right-tab' 模式，新 key）。
  // 默认仍「任务」。
  const [tab, setTab] = useState<RightTab>(() => {
    try {
      const saved = localStorage.getItem('cc-right-panel-tab');
      if (saved === 'tasks' || saved === 'orchestration') return saved;
    } catch {
      /* localStorage 不可用则用默认值 */
    }
    return 'tasks';
  });
  useEffect(() => {
    try { localStorage.setItem('cc-right-panel-tab', tab); } catch {
      /* 存储失败不阻断 */
    }
  }, [tab]);

  // 中栏主面板「当前节点：…」一行点击 → 切到「方案与编排」
  useEffect(() => {
    const onOpen = () => setTab('orchestration');
    window.addEventListener('aicross:open-orchestration', onOpen);
    return () => window.removeEventListener('aicross:open-orchestration', onOpen);
  }, []);

  return (
    <div className="compiler-top">
      <div className="right-tabs right-shell-tabs">
        <button
          className={`right-tab ${tab === 'tasks' ? 'active' : ''}`}
          onClick={() => setTab('tasks')}
        >
          任务
        </button>
        <button
          className={`right-tab ${tab === 'orchestration' ? 'active' : ''}`}
          onClick={() => setTab('orchestration')}
        >
          方案与编排
        </button>
      </div>
      {tab === 'tasks' ? <TaskBoard /> : <OrchestrationPanel />}
    </div>
  );
}
