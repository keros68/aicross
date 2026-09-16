// 左栏「资源」tab。09-15 口径起列出全部已探测模型（不再只列冒烟通过的），
// 按 agent 折叠分组；点模型把 @agent/模型 插进当前对话图主面板输入框。
//
// TASK-09b（用户反馈「现在很乱」）：多 provider 的 agent（pi）再按 provider
// 分小组，组内未验证的默认折成一行；行内只剩「模型名 + 状态点」，真实 ID、
// provider、状态与时间都在悬停里——左栏宽度有限。

import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { commands, isTauri } from '../../tauri';
import { useAppState } from '../../store/app-state';
import { agentDisplayName } from '../../resources/agent-display';
import {
  errorText,
  groupKey,
  loginCheckHint,
  loggedOutProvidersToggleLabel,
  loginLabel,
  cliVersionNumber,
  modelHint,
  modelDisplayName,
  modelPickTarget,
  modelsByAgent,
  providerLoginOf,
  providerGroups,
  showsGroupHeads,
  splitUnverified,
  splitPiProviderGroups,
  unknownProvidersToggleLabel,
  unverifiedToggleLabel,
  verificationOf,
  type ProviderGroup,
  type ResourceAgent,
  type ResourceModel,
  type ResourceStore,
} from '../../resources/types';
import {
  insertMention,
  notifyResourcesChanged,
  openResourcesPage,
  RESOURCES_CHANGED_EVENT,
} from '../../resources/mention';
import { AgentBadge } from './AgentBadge';
import { VerificationMark } from './VerificationMark';
// The left-pane tab shares resources.css with ResourcesOverlay, which is lazy since
// TASK-05 C; import it here too so the tab is styled before the overlay ever opens.
import {
  initialExpanded,
  readRailState,
  toggleInList,
  writeRailState,
} from '../../resources/rail-state';
import './resources.css';

export function ResourcesTab() {
  const { state } = useAppState();
  const [store, setStore] = useState<ResourceStore | null>(null);
  // 折叠状态存在组件外（09-16 用户实测）：切到「工作区」时 Explorer 会把本组件
  // 整个卸载，留在 useState 里的展开状态和 initialized ref 一起清零，回来又被
  // 首次加载分支全部展开，用户刚折叠好的状态每次切标签都被推翻。
  const persisted = useRef(readRailState());
  const [expanded, setExpandedState] = useState<string[]>(persisted.current.expanded ?? []);
  // 展开了未验证清单的小组，键是 `agentId/provider`。默认全折叠。
  const [openUnverified, setOpenUnverifiedState] = useState<string[]>(persisted.current.openUnverified);
  const [openLoggedOut, setOpenLoggedOutState] = useState<string[]>(persisted.current.openLoggedOut);
  const [openUnknown, setOpenUnknownState] = useState<string[]>(persisted.current.openUnknown);

  const setExpanded = (next: string[]) => {
    setExpandedState(next);
    persisted.current = { ...persisted.current, expanded: next };
    writeRailState(persisted.current);
  };
  const setOpenUnverified = (next: string[]) => {
    setOpenUnverifiedState(next);
    persisted.current = { ...persisted.current, openUnverified: next };
    writeRailState(persisted.current);
  };
  const setOpenLoggedOut = (next: string[]) => {
    setOpenLoggedOutState(next);
    persisted.current = { ...persisted.current, openLoggedOut: next };
    writeRailState(persisted.current);
  };
  const setOpenUnknown = (next: string[]) => {
    setOpenUnknownState(next);
    persisted.current = { ...persisted.current, openUnknown: next };
    writeRailState(persisted.current);
  };
  const [checkingProvider, setCheckingProvider] = useState<string | null>(null);
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
        setExpanded(initialExpanded(persisted.current, modelsByAgent(value).map((agent) => agent.id)));
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

  const toggle = (agentId: string) => setExpanded(toggleInList(expanded, agentId));

  const toggleUnverified = (key: string) => setOpenUnverified(toggleInList(openUnverified, key));

  const toggleLoggedOut = (agentId: string) => setOpenLoggedOut(toggleInList(openLoggedOut, agentId));

  const toggleUnknown = (agentId: string) => setOpenUnknown(toggleInList(openUnknown, agentId));

  const recheckProvider = async (provider: string) => {
    setCheckingProvider(provider);
    try {
      const result = await commands.checkPiProviderLogin(provider);
      setStore(result.store);
      setError(result.errors.length > 0 ? result.errors.join('\n') : '');
      notifyResourcesChanged();
    } catch (value) {
      setError(`重新检查失败：${errorText(value)}`);
    } finally {
      setCheckingProvider(null);
    }
  };

  return (
    <div className="resources-pane">
      {error && <div className="resources-error">{error}</div>}
      <ResourceAgentList
        agents={modelsByAgent(store)}
        expanded={expanded}
        openUnverified={openUnverified}
        openLoggedOut={openLoggedOut}
        openUnknown={openUnknown}
        checkingProvider={checkingProvider}
        graphOpen={graphOpen}
        onToggle={toggle}
        onToggleUnverified={toggleUnverified}
        onToggleLoggedOut={toggleLoggedOut}
        onToggleUnknown={toggleUnknown}
        onRecheckProvider={recheckProvider}
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
  openUnverified,
  openLoggedOut = [],
  openUnknown = [],
  checkingProvider = null,
  graphOpen,
  onToggle,
  onToggleUnverified,
  onToggleLoggedOut = () => {},
  onToggleUnknown = () => {},
  onRecheckProvider = async () => {},
  onPick,
}: {
  agents: ResourceAgent[];
  expanded: string[];
  openUnverified: string[];
  openLoggedOut?: string[];
  openUnknown?: string[];
  checkingProvider?: string | null;
  graphOpen: boolean;
  onToggle: (agentId: string) => void;
  onToggleUnverified: (key: string) => void;
  onToggleLoggedOut?: (agentId: string) => void;
  onToggleUnknown?: (agentId: string) => void;
  onRecheckProvider?: (provider: string) => Promise<void>;
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
              <span className="resource-agent-name">{agentDisplayName(agent.id, agent.name || agent.id)}</span>
              <span className="resource-agent-version" title={agent.version || '版本未知'}>
                {agent.version ? cliVersionNumber(agent.version) : '版本未知'}
              </span>
            </button>
            {isExpanded && (
              <ResourceModelGroups
                agentId={agent.id}
                models={agent.models}
                openUnverified={openUnverified}
                loggedOutOpen={openLoggedOut.includes(agent.id)}
                unknownOpen={openUnknown.includes(agent.id)}
                checkingProvider={checkingProvider}
                graphOpen={graphOpen}
                onToggleUnverified={onToggleUnverified}
                onToggleLoggedOut={onToggleLoggedOut}
                onToggleUnknown={onToggleUnknown}
                onRecheckProvider={onRecheckProvider}
                onPick={onPick}
              />
            )}
          </section>
        );
      })}
    </div>
  );
}

/** 一个 agent 底下的全部小组。只有分出两组以上才渲染组头（修正轮 R1）。 */
function ResourceModelGroups({
  agentId,
  models,
  openUnverified,
  loggedOutOpen,
  unknownOpen,
  checkingProvider,
  graphOpen,
  onToggleUnverified,
  onToggleLoggedOut,
  onToggleUnknown,
  onRecheckProvider,
  onPick,
}: {
  agentId: string;
  models: ResourceModel[];
  openUnverified: string[];
  loggedOutOpen: boolean;
  unknownOpen: boolean;
  checkingProvider: string | null;
  graphOpen: boolean;
  onToggleUnverified: (key: string) => void;
  onToggleLoggedOut: (agentId: string) => void;
  onToggleUnknown: (agentId: string) => void;
  onRecheckProvider: (provider: string) => Promise<void>;
  onPick: (agent: string, model: string, provider: string) => void;
}) {
  const allGroups = providerGroups(models);
  const { shown: groups, loggedOut, unknown } = agentId === 'pi'
    ? splitPiProviderGroups(models)
    : { shown: allGroups, loggedOut: [], unknown: [] };
  const withHeads = showsGroupHeads([...groups, ...loggedOut, ...unknown]);
  return (
    <div className="resource-model-list">
      {groups.map((group) => (
        <ResourceModelGroup
          key={group.provider || 'models'}
          agentId={agentId}
          group={group}
          showHead={withHeads}
          showLogin={agentId === 'pi'}
          unverifiedOpen={openUnverified.includes(groupKey(agentId, group.provider))}
          graphOpen={graphOpen}
          onToggleUnverified={onToggleUnverified}
          onPick={onPick}
        />
      ))}
      {loggedOut.length > 0 && (
        <>
          <button
            className="resource-logged-out-toggle"
            type="button"
            aria-expanded={loggedOutOpen}
            onClick={() => onToggleLoggedOut(agentId)}
          >
            {loggedOutProvidersToggleLabel(loggedOut.length, loggedOutOpen)}
          </button>
          {loggedOutOpen && loggedOut.map((group) => (
            <div className="resource-logged-out-provider" key={group.provider}>
              <span>{group.provider}</span>
              <button
                type="button"
                className="row-action"
                disabled={checkingProvider !== null}
                onClick={() => void onRecheckProvider(group.provider)}
              >
                {checkingProvider === group.provider ? '检查中…' : '重新检查'}
              </button>
            </div>
          ))}
        </>
      )}
      {unknown.length > 0 && (
        <>
          <button
            className="resource-logged-out-toggle"
            type="button"
            aria-expanded={unknownOpen}
            onClick={() => onToggleUnknown(agentId)}
          >
            {unknownProvidersToggleLabel(unknown.length, unknownOpen)}
          </button>
          {unknownOpen && unknown.map((group) => (
            <div className="resource-logged-out-provider" key={group.provider}>
              <span>{group.provider}</span>
              {loginCheckHint(group.models) && <span>{loginCheckHint(group.models)}</span>}
              <button
                type="button"
                className="row-action"
                disabled={checkingProvider !== null}
                onClick={() => void onRecheckProvider(group.provider)}
              >
                {checkingProvider === group.provider ? '检查中…' : '重新检查'}
              </button>
            </div>
          ))}
        </>
      )}
    </div>
  );
}

/**
 * 一个 provider 小组：组头（provider 名 + 计数）+ 常显行 + 折叠的未验证行。
 * 只有一个小组时不出组头——provider 名改走行悬停（`modelHint` 里单列一行）。
 */
function ResourceModelGroup({
  agentId,
  group,
  showHead,
  showLogin,
  unverifiedOpen,
  graphOpen,
  onToggleUnverified,
  onPick,
}: {
  agentId: string;
  group: ProviderGroup;
  showHead: boolean;
  showLogin: boolean;
  unverifiedOpen: boolean;
  graphOpen: boolean;
  onToggleUnverified: (key: string) => void;
  onPick: (agent: string, model: string, provider: string) => void;
}) {
  const { shown, collapsed } = splitUnverified(group.models);
  const key = groupKey(agentId, group.provider);
  return (
    <div className="resource-group">
      {showHead && group.provider !== '' && (
        <div className="resource-group-head">
          <span className="resource-group-name">{group.provider}</span>
          {showLogin && <span className="resource-group-login">{loginLabel(providerLoginOf(group.models))}</span>}
          <span className="resource-group-count">{group.models.length}</span>
        </div>
      )}
      {shown.map((model) => (
        <ResourceModelRow
          key={`${model.provider}:${model.id}`}
          model={model}
          agentId={agentId}
          graphOpen={graphOpen}
          onPick={onPick}
        />
      ))}
      {collapsed.length > 0 && (
        <button
          className="resource-unverified-toggle"
          type="button"
          aria-expanded={unverifiedOpen}
          onClick={() => onToggleUnverified(key)}
        >
          <span className="verify-dot verify-unverified" aria-hidden="true" />
          {unverifiedToggleLabel(collapsed.length, unverifiedOpen)}
        </button>
      )}
      {unverifiedOpen &&
        collapsed.map((model) => (
          <ResourceModelRow
            key={`${model.provider}:${model.id}`}
            model={model}
            agentId={agentId}
            graphOpen={graphOpen}
            onPick={onPick}
          />
        ))}
    </div>
  );
}

/**
 * 模型行。悬停提示同时挂进 `aria-describedby`：`title` 在 WebView2 里只响应
 * 鼠标悬停，键盘 Tab 到这一行读不到来源与最近验证时间（修正轮 R1）。
 *
 * 没开对话图时用 `aria-disabled` 而不是 `disabled`：`disabled` 按钮不可聚焦，
 * 键盘整行 Tab 不到、读屏直接跳过，「读得到说明」这条就不成立；点击走 no-op
 * （修正轮 R1，DS A5）。
 */
function ResourceModelRow({
  model,
  agentId,
  graphOpen,
  onPick,
}: {
  model: ResourceModel;
  agentId: string;
  graphOpen: boolean;
  onPick: (agent: string, model: string, provider: string) => void;
}) {
  const hint = graphOpen ? modelHint(model) : `先打开对话图\n${modelHint(model)}`;
  // 按节点取唯一键：拼 id 再把非字母数字换成 `-` 会让 `glm-4.7` 与 `glm-4-7`
  // 撞成同一个 DOM id，aria-describedby 指到第一条（修正轮 R1，DS 10）。
  const hintId = useId();
  return (
    <>
      <button
        className="resource-model-row"
        type="button"
        aria-disabled={!graphOpen}
        title={hint}
        aria-describedby={hintId}
        onClick={() => {
          if (!graphOpen) return;
          const target = modelPickTarget(model, agentId);
          onPick(target.agent, target.model, target.provider);
        }}
      >
        <span className="resource-model-name">{modelDisplayName(model)}</span>
        <span className="resource-model-status">
          <VerificationMark verification={verificationOf(model)} />
        </span>
      </button>
      <span className="resources-sr-only" id={hintId}>
        {hint}
      </span>
    </>
  );
}
