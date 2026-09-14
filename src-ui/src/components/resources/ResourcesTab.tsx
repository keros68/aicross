// 左栏「资源」tab（TASK-07）：只列冒烟通过（已接入）的模型，按 agent 折叠
// 分组；点模型把 @agent/模型 插进当前对话图主面板输入框；栏底汇总进接入页。

import { useCallback, useEffect, useRef, useState } from 'react';
import { commands, isTauri } from '../../tauri';
import { useAppState } from '../../store/app-state';
import {
  connectedByAgent,
  errorText,
  resourceSummaryText,
  type ResourceStore,
} from '../../resources/types';
import {
  insertMention,
  openResourcesPage,
  RESOURCES_CHANGED_EVENT,
} from '../../resources/mention';
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
        setExpanded(
          connectedByAgent(value).map((agent) => agent.id),
        );
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

  const agents = connectedByAgent(store);

  return (
    <div className="resources-pane">
      {error && <div className="resources-error">{error}</div>}
      <div className="resource-list">
        {agents.length === 0 && (
          <div className="resources-empty">
            暂无已接入模型
            <button
              className="resources-empty-link"
              type="button"
              onClick={openResourcesPage}
            >
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
                onClick={() =>
                  setExpanded((value) =>
                    isExpanded
                      ? value.filter((id) => id !== agent.id)
                      : [...value, agent.id],
                  )
                }
              >
                <span className="resource-caret" aria-hidden="true">
                  {isExpanded ? '▾' : '▸'}
                </span>
                <span className="resource-agent-name">{agent.name || agent.id}</span>
                <span className="resource-agent-version">{agent.version || '版本未知'}</span>
              </button>
              {isExpanded && (
                <div className="resource-model-list">
                  {agent.models.map((model) => (
                    <button
                      className="resource-model-row"
                      key={`${model.provider}:${model.id}`}
                      type="button"
                      disabled={!graphOpen}
                      title={
                        graphOpen
                          ? `${model.id}\n${model.source || model.provider || '来源未记录'}`
                          : '先打开对话图'
                      }
                      onClick={() => insertMention(model.agent, model.id)}
                    >
                      <span className="resource-model-id">{model.id}</span>
                      <span className="resource-model-source">
                        {model.source || model.provider || '来源未记录'}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </section>
          );
        })}
      </div>
      <button className="resource-manage-link" type="button" onClick={openResourcesPage}>
        {resourceSummaryText(store)}
      </button>
    </div>
  );
}
