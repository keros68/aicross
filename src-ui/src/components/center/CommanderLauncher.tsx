// 控制台启动器（TASK-22 第一期）。
//
// 空白 tab 的主视图：选 agent、模型和目录后进入 aicross 自己的消息流。
//
// 挑什么、显示成什么字都在 `commander/launcher.ts` 的纯函数里，这个文件只画。

import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { AgentBadge } from '../resources/AgentBadge';
import { commands, isTauri } from '../../tauri';
import type { DispatchableModel } from '../../graph/types';
import { type CommanderAgent } from '../../commander/args';
import {
  availableCommanderAgents,
  commanderAgentLabel,
  commanderBlockReason,
  commanderDefaultAvailable,
  commanderDefaultLine,
  commanderModelLabel,
  commanderModelSelectValue,
  commanderModelsFor,
  commanderModelValue,
  cwdName,
  COMMANDER_NO_CWD,
  COMMANDER_AGENT_CARD_HEIGHT,
  COMMANDER_AGENT_GRID_TEMPLATE,
  markCommanderSelection,
  openCommanderCli,
  parseCommanderModelValue,
  shouldShowNoCommanderModels,
  type CommanderCliEntry,
} from '../../commander/launcher';
import type { CommanderSettings } from '../../commander/types';
import { AGENT_DISPLAY_NAMES } from '../../resources/agent-display';
import { CONTROL_CONSOLE_LABEL } from '../../console-session';

export interface CommanderLaunchArgs {
  agent: CommanderAgent;
  provider: string;
  model: string;
  cwd: string;
}

interface Props {
  /** CenterPanel 的 `toolsInstalled`，按工具 id 存。 */
  installed: Record<string, boolean>;
  /** 首次工具探测是否已经返回。 */
  toolsScanned: boolean;
  /** 已安装且不属于四家内置 agent 的 CLI。 */
  otherClis: CommanderCliEntry[];
  /** 已存的默认；`agent` 为空 = 还没存过。 */
  defaults: CommanderSettings | null;
  /** 当前工作区目录，作为目录的初值。 */
  folderPath: string | null;
  /** 启动器首次进入布局时登记该 tab，防止晚到默认接管。 */
  onShown?: () => void;
  onLaunch: (args: CommanderLaunchArgs) => void;
  onOpenTerminal: () => void;
  onOpenGraph: () => void;
  onOpenTool: (tool: string) => void;
}

export function CommanderModelSelect({
  picked,
  models,
  onChange,
}: {
  picked: { provider: string; model: string } | null;
  models: DispatchableModel[];
  onChange: (picked: { provider: string; model: string } | null) => void;
}) {
  return (
    <select
      id="commander-model"
      className="commander-select"
      value={commanderModelSelectValue(picked)}
      onChange={(event) => onChange(parseCommanderModelValue(event.target.value))}
    >
      <option value="">请选择模型</option>
      {models.map((model) => (
        <option
          key={`${model.agent}/${model.provider}/${model.id}`}
          value={commanderModelValue(model)}
        >
          {commanderModelLabel(model)}
        </option>
      ))}
    </select>
  );
}

export function CommanderOtherCliMenu({
  otherClis,
  onOpenTool,
}: {
  otherClis: CommanderCliEntry[];
  onOpenTool: (tool: string) => void;
}) {
  if (otherClis.length === 0) return null;
  return (
    <details className="commander-other-clis">
      <summary className="commander-link">其它 CLI</summary>
      <div className="commander-other-menu">
        {otherClis.map((item) => (
          <button
            type="button"
            className="commander-link"
            key={item.key}
            onClick={() => openCommanderCli(item.key, onOpenTool)}
          >
            {item.label}
          </button>
        ))}
      </div>
    </details>
  );
}

export function CommanderLauncher({
  installed,
  toolsScanned,
  otherClis,
  defaults,
  folderPath,
  onShown,
  onLaunch,
  onOpenTerminal,
  onOpenGraph,
  onOpenTool,
}: Props) {
  useLayoutEffect(() => { onShown?.(); }, [onShown]);
  const agents = useMemo(() => availableCommanderAgents(installed), [installed]);
  const [models, setModels] = useState<DispatchableModel[]>([]);
  const [modelsLoaded, setModelsLoaded] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [agent, setAgent] = useState<CommanderAgent | ''>('');
  const [picked, setPicked] = useState<{ provider: string; model: string } | null>(null);
  const [cwd, setCwd] = useState<string>(folderPath ?? '');

  // 资源库读一次即可：这个视图的生命周期就是「用户还没起会话」的那一小段。
  useEffect(() => {
    if (!isTauri) return;
    let dead = false;
    commands
      .listDispatchableModels()
      .then((result) => {
        if (dead) return;
        setModels(result.models);
        setModelsError(result.error);
        setModelsLoaded(true);
      })
      .catch(() => {
        if (!dead) {
          setModelsError('读取模型清单失败');
          setModelsLoaded(true);
        }
      });
    return () => {
      dead = true;
    };
  }, []);

  // 默认值只填一次初值，填完就交给用户。用 ref 而不是把 defaults 放进
  // 依赖：defaults 由 CenterPanel 异步读进来，晚到的那一次不该覆盖用户
  // 已经改过的选择。
  const seeded = useRef(false);
  useEffect(() => {
    if (seeded.current || defaults === null || agents.length === 0) return;
    seeded.current = true;
    const preferred = defaults && agents.includes(defaults.agent as CommanderAgent)
      ? (defaults.agent as CommanderAgent)
      : agents[0];
    setAgent(preferred);
    if (defaults && defaults.agent === preferred && defaults.model !== '') {
      setPicked({ provider: defaults.provider, model: defaults.model });
    }
    if (defaults && defaults.cwd !== '' && (folderPath ?? '') === '') {
      setCwd(defaults.cwd);
    }
  }, [agents, defaults, folderPath]);

  // 工作区换目录时跟着换——用户在左栏选了文件夹，这里还显示旧的会误导。
  useEffect(() => {
    if ((folderPath ?? '') !== '') setCwd(folderPath as string);
  }, [folderPath]);

  const agentModels = useMemo(
    () => (agent === '' ? [] : commanderModelsFor(models, agent)),
    [models, agent],
  );

  // 换 agent 后旧模型多半不属于新 agent，清掉；能在新清单里找到同名的就留着。
  useEffect(() => {
    if (picked === null) return;
    const stillThere = agentModels.some(
      (m) => m.id === picked.model && m.provider === picked.provider,
    );
    if (modelsLoaded && !stillThere) setPicked(null);
  }, [agentModels, modelsLoaded, picked]);

  const pickFolder = () => {
    import('@tauri-apps/plugin-dialog')
      .then(({ open }) => open({ directory: true }))
      .then((selected) => {
        if (typeof selected === 'string') setCwd(selected);
      })
      .catch(() => {});
  };

  const blocked = commanderBlockReason(
    agent,
    agents,
    picked?.provider ?? '',
    picked?.model ?? '',
  ) ?? (picked === null ? '先选一个模型' : null);
  const launch = (args: CommanderLaunchArgs) => onLaunch(args);

  const defaultAvailable = commanderDefaultAvailable(defaults, installed, toolsScanned);
  const defaultLine = defaultAvailable ? commanderDefaultLine(defaults) : null;

  return (
    <div className="commander-launcher">
      <div className="commander-inner">
        <div className="commander-title">{CONTROL_CONSOLE_LABEL}</div>
        <div className="commander-desc">
          选定 agent、模型和目录后可单线聊天；输入 @ 可临时改派给另一家。
        </div>

        {defaultLine !== null && defaults !== null && (
          <button
            type="button"
            className="commander-primary-btn commander-oneshot"
            onClick={() =>
              launch({
                agent: defaults.agent as CommanderAgent,
                provider: defaults.provider,
                model: defaults.model,
                // 一键行也按「当前工作区优先」（R1 复审 3）
                cwd: (folderPath ?? '') !== '' ? (folderPath as string) : defaults.cwd,
              })
            }
          >
            {defaultLine}
          </button>
        )}

        {!toolsScanned ? (
          <div className="commander-empty">探测中…</div>
        ) : agents.length === 0 ? (
          <div className="commander-empty">
            本机没探测到 {Object.values(AGENT_DISPLAY_NAMES).join(' / ')}。可在接入页完成探测。
          </div>
        ) : (
          <>
            <div className="commander-row">
              <div className="commander-field commander-field--agents">
                <div className="commander-label">agent</div>
                <div
                  className="commander-agents"
                  style={{
                    '--commander-agent-grid': COMMANDER_AGENT_GRID_TEMPLATE,
                    '--commander-agent-card-height': `${COMMANDER_AGENT_CARD_HEIGHT}px`,
                  } as CSSProperties}
                >
                  {agents.map((item) => (
                    <button
                      type="button"
                      key={item}
                      className={`commander-agent-card${agent === item ? ' is-picked' : ''}`}
                      aria-pressed={agent === item}
                      onClick={() => markCommanderSelection(seeded, setAgent, item)}
                    >
                      <AgentBadge agentId={item} />
                      <span className="commander-agent-name">{commanderAgentLabel(item)}</span>
                    </button>
                  ))}
                </div>
              </div>

              <div className="commander-field">
                <label className="commander-label" htmlFor="commander-model">
                  模型
                </label>
                <CommanderModelSelect picked={picked} models={agentModels} onChange={setPicked} />
                {modelsError !== null && (
                  <div className="commander-note">{modelsError}</div>
                )}
                {shouldShowNoCommanderModels(modelsLoaded, modelsError, agent, agentModels.length) && (
                  <div className="commander-note">
                    资源库里还没有 {commanderAgentLabel(agent)} 的模型，先去接入页探测。
                  </div>
                )}
              </div>

              <div className="commander-field">
                <div className="commander-label">工作区目录</div>
                <button type="button" className="commander-cwd" onClick={pickFolder}>
                  <span className="commander-cwd-path">{cwd || COMMANDER_NO_CWD}</span>
                  <span className="commander-cwd-pick">选择</span>
                </button>
              </div>
            </div>

            <button
              type="button"
              className="commander-primary-btn"
              disabled={blocked !== null}
              title={blocked ?? undefined}
              onClick={() =>
                agent !== '' &&
                launch({
                  agent,
                  provider: picked?.provider ?? '',
                  model: picked?.model ?? '',
                  cwd,
                })
              }
            >
              进入控制台
            </button>
            {blocked !== null && <div className="commander-note">{blocked}</div>}
          </>
        )}

        <div className="commander-secondary">
          <button type="button" className="commander-link" onClick={onOpenTerminal}>
            系统终端
          </button>
          <button type="button" className="commander-link" onClick={onOpenGraph}>
            对话图
          </button>
          <CommanderOtherCliMenu otherClis={otherClis} onOpenTool={onOpenTool} />
        </div>
      </div>
    </div>
  );
}

/** 一键行的文案在测试里也要，转出来免得测试重算一遍。 */
export { commanderDefaultLine, cwdName };
