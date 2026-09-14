// RightPanel.tsx — 右栏 tab 壳：「任务」（现有 TaskBoard，内部任务/修改记录
// 两个 tab 不动）与「方案与编排」（第 2 期 OrchestrationPanel）。
// .compiler-top wrapper styling lives in global.css (same selector,
// kept around because changing the wrapper class would touch every
// theme rule that scopes to it).

import { useEffect, useState } from 'react';
import { TaskBoard } from './TaskBoard';
import { OrchestrationPanel } from './orchestration/OrchestrationPanel';
import {
  consumeOrchestrationRequest,
  initialRightTab,
  OPEN_ORCHESTRATION_EVENT,
  type RightTab,
} from '../../lib/open-orchestration';
import './Compiler.css';

export function RightPanel() {
  // 使用 aicross 自己的 key，避免旧版保存的任务板默认值覆盖本期新默认。
  // 待办请求优先：右栏是被「选预设」那一步刚展开的话，事件在 mount 之前
  // 就发完了，这里补读一次（见 lib/open-orchestration.ts）。
  const [tab, setTab] = useState<RightTab>(() => {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem('aicross-right-panel-tab');
    } catch {
      /* localStorage 不可用则用默认值 */
    }
    return initialRightTab(saved, consumeOrchestrationRequest());
  });
  useEffect(() => {
    try { localStorage.setItem('aicross-right-panel-tab', tab); } catch {
      /* 存储失败不阻断 */
    }
  }, [tab]);

  // 中栏主面板「当前节点：…」一行点击 → 切到「方案与编排」。
  // 顺手把待办标记吃掉：已经挂着的时候事件就够了，标记留着会让下次
  // 重新 mount 时又跳过来。
  useEffect(() => {
    const onOpen = () => {
      consumeOrchestrationRequest();
      setTab('orchestration');
    };
    window.addEventListener(OPEN_ORCHESTRATION_EVENT, onOpen);
    return () => window.removeEventListener(OPEN_ORCHESTRATION_EVENT, onOpen);
  }, []);

  return (
    <div className="compiler-top">
      <div className="right-tabs right-shell-tabs">
        <button
          className={`right-tab ${tab === 'orchestration' ? 'active' : ''}`}
          onClick={() => setTab('orchestration')}
        >
          方案与编排
        </button>
        <button
          className={`right-tab ${tab === 'tasks' ? 'active' : ''}`}
          onClick={() => setTab('tasks')}
        >
          任务
        </button>
      </div>
      {tab === 'tasks' ? <TaskBoard /> : <OrchestrationPanel />}
    </div>
  );
}
