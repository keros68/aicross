// 左栏「资源」tab。09-15 口径起列出全部已探测模型（不再只列冒烟通过的），
// 按 agent 折叠分组；点模型把 @agent/模型 插进当前对话图主面板输入框。
//
// 行内只放「模型名 + 状态标记」，来源路径与最近验证时间挪到 title 悬停——
// 左栏宽度有限，每行顶着一条 `读:C:\Users\…` 会把模型名挤没。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri } from '../../tauri';
import { useAppState } from '../../store/app-state';
import {
  ambiguousModelIds,
  errorText,
  modelHint,
  modelRowLabel,
  modelsByAgent,
  verificationOf,
  type ResourceAgent,
  type ResourceModel,
  type ResourceStore,
} from '../../resources/types';
import {
  insertMention,
  openResourcesPage,
  RESOURCES_CHANGED_EVENT,
} from '../../resources/mention';
import { AgentBadge } from './AgentBadge';
import { VerificationMark } from './VerificationMark';
// The left-pane tab shares resources.css with ResourcesOverlay, which is lazy since
// TASK-05 C; import it here too so the tab is styled before the overlay ever opens.
import './resources.css';

export function ResourcesTab() {
  const { state } = useAppState();
  const [store, setStore] = useState<ResourceStore | null>(null);
  const [expanded, setExpanded] = useState<string[]>([]);
  const [error, setError] = useState('');
  const initialized = useRef(false);
  // 点模型靠 GraphTab 挂载时的事件监听接收；没有打开的对话图 tab 时
  // 事件无人接收，模型行置灰提示（修正轮 R1，审查 1）。
  const graphOpen = state.terminals.some((session) => session.tool === 'graph');

  const load = useCallback(async () => {
    if (!isTauri) {
      setError('未在应用内运行，无法读取资源');
      return;
    }
    try {
      const value = await commands.getResources();
      setStore(value);
      if (!initialized.current) {
        setExpanded(modelsByAgent(value).map((agent) => agent.id));
        initialized.current = true;
      }
      setError('');
    } catch (value) {
      setError(`读取资源失败：${errorText(value)}`);
    }
  }, []);

  useEffect(() => {
    void load();
    window.addEventListener(RESOURCES_CHANGED_EVENT, load);
    return () => window.removeEventListener(RESOURCES_CHANGED_EVENT, load);
  }, [load]);

  if (!store) {
    return <div className="resources-pane">{error || '正在读取资源…'}</div>;
  }

  const toggle = (agentId: string) =>
    setExpanded((value) =>
      value.includes(agentId) ? value.filter((id) => id !== agentId) : [...value, agentId],
    );

  return (
    <div className="resources-pane">
      {error && <div className="resources-error">{error}</div>}
      <ResourceAgentList
        agents={modelsByAgent(store)}
        expanded={expanded}
        graphOpen={graphOpen}
        onToggle={toggle}
        onPick={insertMention}
      />
      <button className="resource-manage-link" type="button" onClick={openResourcesPage}>
        管理接入 →
      </button>
    </div>
  );
}

/** 纯展示部分：无副作用、无 tauri 依赖，vitest 直接 renderToStaticMarkup。 */
export function ResourceAgentList({
  agents,
  expanded,
  graphOpen,
  onToggle,
  onPick,
}: {
  agents: ResourceAgent[];
  expanded: string[];
  graphOpen: boolean;
  onToggle: (agentId: string) => void;
  onPick: (agent: string, model: string, provider: string) => void;
}) {
  return (
    <div className="resource-list">
      {agents.length === 0 && (
        <div className="resources-empty">
          暂无已探测模型
          <button className="resources-empty-link" type="button" onClick={openResourcesPage}>
            去接入
          </button>
        </div>
      )}
      {agents.map((agent) => {
        const isExpanded = expanded.includes(agent.id);
        return (
          <section className="resource-agent" key={agent.id}>
            <button
              className="resource-agent-summary"
              type="button"
              aria-expanded={isExpanded}
              onClick={() => onToggle(agent.id)}
            >
              <span className="resource-caret" aria-hidden="true">
                {isExpanded ? '▾' : '▸'}
              </span>
              <AgentBadge agentId={agent.id} />
              <span className="resource-agent-name">{agent.name || agent.id}</span>
              <span className="resource-agent-version" title={agent.version || '版本未知'}>
                {agent.version || '版本未知'}
              </span>
            </button>
            {isExpanded && (
              <div className="resource-model-list">
                {agent.models.map((model) => (
                  <ResourceModelRow
                    key={`${model.provider}:${model.id}`}
                    model={model}
                    agentId={agent.id}
                    ambiguous={ambiguousModelIds(agent.models)}
                    graphOpen={graphOpen}
                    onPick={onPick}
                  />
                ))}
              </div>
            )}
          </section>
        );
      })}
    </div>
  );
}

/**
 * 模型行。悬停提示同时挂进 `aria-describedby`：`title` 在 WebView2 里只响应
 * 鼠标悬停，键盘 Tab 到这一行读不到来源与最近验证时间（修正轮 R1）。
 */
function ResourceModelRow({
  model,
  agentId,
  ambiguous,
  graphOpen,
  onPick,
}: {
  model: ResourceModel;
  agentId: string;
  ambiguous: Set<string>;
  graphOpen: boolean;
  onPick: (agent: string, model: string, provider: string) => void;
}) {
  const hint = graphOpen ? modelHint(model) : '先打开对话图';
  const hintId = `resource-hint-${agentId}-${model.provider}-${model.id}`.replace(
    /[^A-Za-z0-9_-]/g,
    '-',
  );
  return (
    <>
      <button
        className="resource-model-row"
        type="button"
        disabled={!graphOpen}
        title={hint}
        aria-describedby={hintId}
        onClick={() => onPick(model.agent || agentId, model.id, model.provider)}
      >
        <span className="resource-model-id">{modelRowLabel(model, ambiguous)}</span>
        <VerificationMark verification={verificationOf(model)} />
      </button>
      <span className="resources-sr-only" id={hintId}>
        {hint}
      </span>
    </>
  );
}
