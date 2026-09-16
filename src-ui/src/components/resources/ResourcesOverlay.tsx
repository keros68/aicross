// 接入页（TASK-07；09-15 换冒烟口径）：全屏覆盖层，与「讲解」ExplainOverlay
// 同形态。agent 卡片网格：字母徽标、名称、版本、检测状态、全部模型与验证档位；
// 卡底「探测 / 验证 / 手动输入」。
//
// 09-15 口径：原生 CLI（claude / codex / kimi）的模型读清单即可用，不给「验证」
// 按钮；第三方端点的模型显示未验证 / 已验证 / 验证失败三态，首次派发时引擎自动
// 验一次，用户也可以在这里手动重验。
// cc-switch 降级为发现提示：只显示「cc-switch 里有 X，建议接入 pi」。

import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { commands, isTauri } from '../../tauri';
import { loadDispatchSettings } from '../../graph/dispatch-settings';
import {
  errorText,
  cliVersionNumber,
  loginCheckHint,
  modelDetailLines,
  modelDisplayName,
  modelHint,
  parseCommandArgs,
  providerLoginOf,
  providerGroups,
  showsGroupHeads,
  quotaExhaustedLabel,
  resourceModelKey,
  loggedOutProvidersToggleLabel,
  loginLabel,
  loginOf,
  splitPiProviderGroups,
  splitUnverified,
  unknownProvidersToggleLabel,
  unverifiedSummaryText,
  unverifiedToggleLabel,
  verificationOf,
  type AddManualAgentRequest,
  type CcSwitchHint,
  type ResourceAgent,
  type ResourceModel,
  type ResourceModelRequest,
  type ResourceOperation,
  type ResourceStore,
} from '../../resources/types';
import { notifyResourcesChanged } from '../../resources/mention';
import { agentDisplayName } from '../../resources/agent-display';
import { AgentBadge } from './AgentBadge';
import { VerificationMark } from './VerificationMark';
import './resources.css';

const vendors = ['Anthropic', 'OpenAI', 'Moonshot', 'Zhipu', 'DeepSeek', 'MiniMax', 'Alibaba', 'StepFun', '其他'];
const tiers = ['', '低', '中', '高'];

const emptyModel = (agentId: string): ResourceModelRequest => ({
  agent_id: agentId,
  id: '',
  provider: '',
  vendor: '其他',
  tier: '',
  note: '',
  fallback_order: [],
});

function operationErrors(operation: ResourceOperation): string {
  return operation.errors.length > 0 ? operation.errors.join('\n') : '';
}

export function ResourcesOverlay({ onClose }: { onClose: () => void }) {
  const [store, setStore] = useState<ResourceStore | null>(null);
  const [hints, setHints] = useState<CcSwitchHint[]>([]);
  const [modelAgent, setModelAgent] = useState<string | null>(null);
  const [manualAgentOpen, setManualAgentOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [error, setError] = useState('');

  useEffect(() => {
    if (!isTauri) {
      setError('未在应用内运行，无法读取资源');
      return;
    }
    commands
      .getResources()
      .then(setStore)
      .catch((value) => setError(`读取 agent 状态失败：${errorText(value)}`));
  }, []);

  const ccSwitchPath = () => loadDispatchSettings().ccSwitchPath;

  const finishOperation = (value: ResourceOperation) => {
    setStore(value.store);
    // 冒烟等命令不带 hints：空数组不清空已有提示条（否则点一次「冒烟」
    // cc-switch 提示条就消失，下次探测才回来）。
    if (value.cc_switch_hints.length > 0) {
      setHints(value.cc_switch_hints);
    }
    setError(operationErrors(value));
    notifyResourcesChanged();
  };

  const detectAll = async () => {
    setBusy('detect:all');
    try {
      finishOperation(await commands.detectAllResources(ccSwitchPath()));
    } catch (value) {
      setError(`探测失败：${errorText(value)}`);
    } finally {
      setBusy(null);
    }
  };

  const detect = async (agent: ResourceAgent) => {
    setBusy(`detect:${agent.id}`);
    setNotes((value) => ({ ...value, [agent.id]: '探测中…' }));
    try {
      const result = await commands.detectResourceAgent(agent.id, ccSwitchPath());
      finishOperation(result);
      const nextAgent = result.store.agents.find((item) => item.id === agent.id);
      setNotes((value) => ({
        ...value,
        [agent.id]: result.errors.length
          ? '探测完成，有提示'
          : `已读取 ${nextAgent?.models.length ?? 0} 个模型`,
      }));
    } catch (value) {
      const message = `探测失败：${errorText(value)}`;
      setError(message);
      setNotes((notesValue) => ({ ...notesValue, [agent.id]: message }));
    } finally {
      setBusy(null);
    }
  };

  // 整卡重验：按钮只挂在第三方 agent 的卡片上（原生卡片不渲染它），
  // 后端仍走同一个 smoke_agent_models 命令。
  const verifyAgent = async (agent: ResourceAgent) => {
    setBusy(`smoke-agent:${agent.id}`);
    setNotes((value) => ({ ...value, [agent.id]: '验证中…' }));
    try {
      const result = await commands.smokeAgentModels(agent.id);
      finishOperation(result);
      setNotes((value) => ({
        ...value,
        [agent.id]: result.errors.length ? '验证完成，有失败项' : '验证完成',
      }));
    } catch (value) {
      const message = `验证失败：${errorText(value)}`;
      setError(message);
      setNotes((notesValue) => ({ ...notesValue, [agent.id]: message }));
    } finally {
      setBusy(null);
    }
  };

  const verifyModel = async (model: ResourceModel) => {
    setBusy(`smoke:${model.agent}:${model.provider}:${model.id}`);
    setNotes((value) => ({ ...value, [model.agent]: '验证中…' }));
    try {
      const value = await commands.smokeResourceModel({
        agent_id: model.agent,
        id: model.id,
        provider: model.provider,
      });
      setStore(value);
      setError('');
      setNotes((notesValue) => ({ ...notesValue, [model.agent]: `${model.id} 验证完成` }));
      notifyResourcesChanged();
    } catch (value) {
      const message = `验证失败：${errorText(value)}`;
      setError(message);
      setNotes((notesValue) => ({ ...notesValue, [model.agent]: message }));
    } finally {
      setBusy(null);
    }
  };

  const recheckProvider = async (provider: string) => {
    setBusy(`login:${provider}`);
    try {
      finishOperation(await commands.checkPiProviderLogin(provider));
    } catch (value) {
      setError(`重新检查失败：${errorText(value)}`);
    } finally {
      setBusy(null);
    }
  };

  const updateModel = async (
    model: ResourceModel,
    patch: Partial<Pick<ResourceModel, 'vendor' | 'tier' | 'note' | 'fallback_order'>> & { clear_quota_exhausted?: boolean },
  ) => {
    setBusy(`update:${model.agent}:${model.provider}:${model.id}`);
    try {
      const value = await commands.updateResourceModel({
        agent_id: model.agent,
        id: model.id,
        provider: model.provider,
        vendor: patch.vendor ?? model.vendor,
        tier: patch.tier ?? model.tier,
        note: patch.note ?? model.note,
        fallback_order: patch.fallback_order ?? model.fallback_order ?? [],
        clear_quota_exhausted: patch.clear_quota_exhausted ?? false,
      });
      setStore(value);
      setError('');
      notifyResourcesChanged();
    } catch (value) {
      setError(`更新资源失败：${errorText(value)}`);
    } finally {
      setBusy(null);
    }
  };

  const removeModel = async (model: ResourceModel) => {
    setBusy(`remove:${model.agent}:${model.provider}:${model.id}`);
    try {
      const value = await commands.removeResourceModel({
        agent_id: model.agent,
        id: model.id,
        provider: model.provider,
      });
      setStore(value);
      setError('');
      setNotes((notesValue) => ({ ...notesValue, [model.agent]: `已移除 ${model.id}` }));
      notifyResourcesChanged();
    } catch (value) {
      setError(`移除资源失败：${errorText(value)}`);
    } finally {
      setBusy(null);
    }
  };

  return createPortal(
    <div className="resources-backdrop">
      <div className="resources-page">
        <button className="resources-close" onClick={onClose}>
          关闭
        </button>
        <h1>接入资源</h1>
        <p className="resources-lead">
          检测本机已安装的 agent。原生 CLI 的模型读清单即可用；第三方端点的模型
          首次派发时自动验证真身。
          <button
            className="res-btn resources-detect-all"
            type="button"
            disabled={busy !== null}
            onClick={() => void detectAll()}
          >
            {busy === 'detect:all' ? '探测中…' : '全部探测'}
          </button>
        </p>
        {store && unverifiedSummaryText(store) !== '' && (
          <p className="resources-count">{unverifiedSummaryText(store)}</p>
        )}
        {error && <div className="resources-error">{error}</div>}
        {hints.length > 0 && (
          <div className="resources-hints">
            {hints.map((hint) => (
              <div className="resources-hint" key={hint.name}>
                cc-switch 里有 {hint.name}（{hint.models.length} 个模型
                {hint.has_token ? '' : '，未配置密钥'}），建议接入 pi
              </div>
            ))}
          </div>
        )}
        <div className="agent-card-grid">
          {store ? (
            store.agents.map((agent) => (
              <AgentCard
                key={agent.id}
                agent={agent}
                models={store.agents.flatMap((item) => item.models)}
                busy={busy}
                note={notes[agent.id] || ''}
                onDetect={detect}
                onVerifyAgent={verifyAgent}
                onVerifyModel={verifyModel}
                onRecheckProvider={recheckProvider}
                onUpdate={updateModel}
                onRemove={removeModel}
                onManual={() => setModelAgent(agent.id)}
              />
            ))
          ) : (
            <div className="resources-empty">正在读取…</div>
          )}
        </div>
        <div className="resources-footer">
          <button className="res-btn" type="button" onClick={() => setManualAgentOpen(true)}>
            手动接入 agent
          </button>
        </div>
        {store && modelAgent && (
          <ResourceModelDialog
            agentId={modelAgent}
            agentName={
              agentDisplayName(
                modelAgent,
                store.agents.find((agent) => agent.id === modelAgent)?.name || modelAgent,
              )
            }
            onClose={() => setModelAgent(null)}
            onSaved={(value) => {
              setStore(value);
              setModelAgent(null);
              setError('');
              notifyResourcesChanged();
            }}
            onError={setError}
          />
        )}
        {manualAgentOpen && (
          <ManualAgentDialog
            onClose={() => setManualAgentOpen(false)}
            onSaved={(operation) => {
              setStore(operation.store);
              setManualAgentOpen(false);
              setError(operationErrors(operation));
              notifyResourcesChanged();
            }}
            onError={setError}
          />
        )}
      </div>
    </div>,
    document.body,
  );
}

/**
 * 「详情」展开后的几行说明。单独一个组件只为让 vitest 直接
 * renderToStaticMarkup（详情默认收着，静态渲染进不去）。
 *
 * 修正轮 R1（DS 3）：provider 单列一行。单 provider 的 agent 不出组头，
 * 卡片上原来就没有别的地方写着它挂在哪个端点上。
 */
export function AgentCardModelMeta({ model }: { model: ResourceModel }) {
  return (
    <>
      {modelDetailLines(model).map((line, index) => (
        <div
          className={`agent-card-model-meta${line.startsWith('失败原因：') ? ' failure' : ''}`}
          key={`${index}:${line}`}
        >
          {line}
        </div>
      ))}
    </>
  );
}

export function FallbackOrderEditor({
  model,
  models,
  disabled = false,
  onChange,
}: {
  model: ResourceModel;
  models: ResourceModel[];
  disabled?: boolean;
  onChange: (order: string[]) => void;
}) {
  const order = model.fallback_order ?? [];
  const byKey = new Map(models.map((item) => [resourceModelKey(item), item]));
  const candidates = models.filter((item) => {
    const key = resourceModelKey(item);
    return key !== resourceModelKey(model) && !order.includes(key);
  });
  const move = (index: number, offset: number) => {
    onChange(moveFallbackOrder(order, index, offset));
  };
  return (
    <div className="fallback-order-editor">
      <label>
        备选顺序
        <select
          aria-label={`${model.id} 备选顺序`}
          value=""
          disabled={disabled || candidates.length === 0}
          onChange={(event) => {
            if (event.target.value) onChange([...order, event.target.value]);
          }}
        >
          <option value="">选择模型</option>
          {candidates.map((item) => (
            <option key={resourceModelKey(item)} value={resourceModelKey(item)}>
              {item.agent}/{item.provider ? `${item.provider}/` : ''}{item.id}{loginOf(item) === 'logged_out' ? '（未登录）' : ''}
            </option>
          ))}
        </select>
      </label>
      {order.map((key, index) => {
        const item = byKey.get(key);
        const label = item
          ? `${item.agent}/${item.provider ? `${item.provider}/` : ''}${item.id}${loginOf(item) === 'logged_out' ? '（未登录）' : ''}`
          : key;
        return (
          <div className="fallback-order-row" key={key}>
            <span>{index + 1}. {label}</span>
            <button type="button" disabled={disabled || index === 0} onClick={() => move(index, -1)}>上移</button>
            <button type="button" disabled={disabled || index === order.length - 1} onClick={() => move(index, 1)}>下移</button>
            <button type="button" disabled={disabled} onClick={() => onChange(order.filter((item) => item !== key))}>移除</button>
          </div>
        );
      })}
    </div>
  );
}

export function moveFallbackOrder(order: string[], index: number, offset: number): string[] {
  const next = [...order];
  const target = index + offset;
  if (target < 0 || target >= next.length) return next;
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

export function quotaClearVisible(model: ResourceModel): boolean {
  return quotaExhaustedLabel(model) !== '';
}

/** 接入页的 agent 卡片。导出只为让 vitest 直接 renderToStaticMarkup（TASK-09b）。 */
export function AgentCard({
  agent,
  models = agent.models,
  busy,
  note,
  onDetect,
  onVerifyAgent,
  onVerifyModel,
  onRecheckProvider,
  onUpdate,
  onRemove,
  onManual,
}: {
  agent: ResourceAgent;
  models?: ResourceModel[];
  busy: string | null;
  note: string;
  onDetect: (agent: ResourceAgent) => Promise<void>;
  onVerifyAgent: (agent: ResourceAgent) => Promise<void>;
  onVerifyModel: (model: ResourceModel) => Promise<void>;
  onRecheckProvider?: (provider: string) => Promise<void>;
  onUpdate: (
    model: ResourceModel,
    patch: Partial<Pick<ResourceModel, 'vendor' | 'tier' | 'note' | 'fallback_order'>> & { clear_quota_exhausted?: boolean },
  ) => Promise<void>;
  onRemove: (model: ResourceModel) => Promise<void>;
  onManual: () => void;
}) {
  const allGroups = providerGroups(agent.models);
  const { shown: groups, loggedOut, unknown } = agent.id === 'pi'
    ? splitPiProviderGroups(agent.models)
    : { shown: allGroups, loggedOut: [], unknown: [] };
  // 只有一个小组时不出组头，provider 改进详情块（修正轮 R1，DS 3）。
  const withHeads = showsGroupHeads([...groups, ...loggedOut, ...unknown]);
  const [openDetails, setOpenDetails] = useState<Record<string, boolean>>({});
  const [openUnverified, setOpenUnverified] = useState<Record<string, boolean>>({});
  const [loggedOutOpen, setLoggedOutOpen] = useState(false);
  const [unknownOpen, setUnknownOpen] = useState(false);
  const detecting = busy === `detect:${agent.id}`;
  const verifyingAgent = busy === `smoke-agent:${agent.id}`;
  // 原生 agent 的模型不验证，整卡与单条都不给「验证」按钮。
  const hasThirdParty = agent.models.some((model) => verificationOf(model) !== 'native');
  return (
    <article className="agent-card">
      <div className="agent-card-head">
        <AgentBadge agentId={agent.id} size="md" />
        <span className="agent-card-name">{agentDisplayName(agent.id, agent.name || agent.id)}</span>
        <span className={`detection ${agent.detected ? 'detected' : 'missing'}`}>
          {agent.detected ? '已检测' : '未检测'}
        </span>
      </div>
      <div className="agent-card-version" title={agent.version || '版本未知'}>
        {agent.version ? cliVersionNumber(agent.version) : '版本未知'}
      </div>
      <div className="agent-card-models">
        {groups.length === 0 && loggedOut.length === 0 && unknown.length === 0 && <div className="agent-empty">暂无模型</div>}
        {groups.map((group) => {
          const groupId = `${agent.id}/${group.provider}`;
          const { shown, collapsed } = splitUnverified(group.models);
          const unverifiedOpen = !!openUnverified[groupId];
          const visibleModels = unverifiedOpen ? [...shown, ...collapsed] : shown;
          return (
          <div className="agent-card-group-wrap" key={group.provider || 'models'}>
            {withHeads && group.provider !== '' && (
              <div className="agent-card-group">
                <span className="agent-card-group-name">{group.provider}</span>
                {agent.id === 'pi' && <span className="agent-card-group-login">{loginLabel(providerLoginOf(group.models))}</span>}
                <span className="agent-card-group-count">{group.models.length}</span>
              </div>
            )}
            {visibleModels.map((model) => {
              const key = `${model.agent}:${model.provider}:${model.id}`;
              const verification = verificationOf(model);
              const verifying = busy === `smoke:${key}`;
              const updating = busy === `update:${key}`;
              const removing = busy === `remove:${key}`;
              return (
                <div className="agent-card-model" key={`${model.provider}:${model.id}`}>
                  <div className="agent-card-model-main" title={modelHint(model)}>
                    <span className="agent-card-model-name">
                      {modelDisplayName(model)}
                    </span>
                    <span className="agent-card-model-state">
                      <VerificationMark verification={verification} />
                    </span>
                  </div>
                  <div className="agent-card-model-detail">
                    <button type="button" className="agent-card-detail-toggle" onClick={() => setOpenDetails((current) => ({ ...current, [key]: !current[key] }))}>详情</button>
                    {openDetails[key] && <div>
                    <div className="agent-card-model-fields">
                      <label>
                        厂商
                        <select
                          value={model.vendor}
                          disabled={busy !== null}
                          onChange={(event) =>
                            void onUpdate(model, { vendor: event.target.value })
                          }
                        >
                          {vendors.map((vendor) => (
                            <option key={vendor}>{vendor}</option>
                          ))}
                        </select>
                      </label>
                      <label>
                        档位
                        <select
                          value={model.tier}
                          disabled={busy !== null}
                          onChange={(event) =>
                            void onUpdate(model, { tier: event.target.value })
                          }
                        >
                          {tiers.map((tier) => (
                            <option value={tier} key={tier}>
                              {tier || '未设置'}
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                    <AgentCardModelMeta model={model} />
                    <FallbackOrderEditor
                      model={model}
                      models={models}
                      disabled={busy !== null}
                      onChange={(fallbackOrder) => void onUpdate(model, { fallback_order: fallbackOrder })}
                    />
                    <div className="agent-card-model-actions">
                      {quotaClearVisible(model) && (
                        <button
                          className="row-action"
                          type="button"
                          disabled={busy !== null}
                          onClick={() => void onUpdate(model, { clear_quota_exhausted: true })}
                        >
                          清除
                        </button>
                      )}
                      {verification !== 'native' && (
                        <button
                          className="row-action"
                          type="button"
                          disabled={busy !== null}
                          onClick={() => void onVerifyModel(model)}
                        >
                          {verifying ? '验证中…' : verification === 'unverified' ? '验证' : '重新验证'}
                        </button>
                      )}
                      <button
                        className="row-action danger"
                        type="button"
                        disabled={busy !== null}
                        onClick={() => void onRemove(model)}
                      >
                        {removing ? '移除中…' : '移除'}
                      </button>
                      {updating && <span className="agent-card-model-saving">保存中…</span>}
                    </div>
                    </div>}
                  </div>
                </div>
              );
            })}
            {collapsed.length > 0 && (
              <button
                type="button"
                className="agent-card-unverified-toggle"
                aria-expanded={unverifiedOpen}
                onClick={() => setOpenUnverified((current) => ({ ...current, [groupId]: !current[groupId] }))}
              >
                {unverifiedToggleLabel(collapsed.length, unverifiedOpen)}
              </button>
            )}
          </div>
          );
        })}
        {loggedOut.length > 0 && (
          <div className="agent-card-logged-out">
            <button
              type="button"
              className="agent-card-logged-out-toggle"
              aria-expanded={loggedOutOpen}
              onClick={() => setLoggedOutOpen((value) => !value)}
            >
              {loggedOutProvidersToggleLabel(loggedOut.length, loggedOutOpen)}
            </button>
            {loggedOutOpen && loggedOut.map((group) => (
              <div className="agent-card-logged-out-provider" key={group.provider}>
                <span>{group.provider}</span>
                <button
                  type="button"
                  className="row-action"
                  disabled={busy !== null || !onRecheckProvider}
                  onClick={() => void onRecheckProvider?.(group.provider)}
                >
                  {busy === `login:${group.provider}` ? '检查中…' : '重新检查'}
                </button>
              </div>
            ))}
          </div>
        )}
        {unknown.length > 0 && (
          <div className="agent-card-logged-out">
            <button
              type="button"
              className="agent-card-logged-out-toggle"
              aria-expanded={unknownOpen}
              onClick={() => setUnknownOpen((value) => !value)}
            >
              {unknownProvidersToggleLabel(unknown.length, unknownOpen)}
            </button>
            {unknownOpen && unknown.map((group) => (
              <div className="agent-card-logged-out-provider" key={group.provider}>
                <span>{group.provider}</span>
                {loginCheckHint(group.models) && <span>{loginCheckHint(group.models)}</span>}
                <button
                  type="button"
                  className="row-action"
                  disabled={busy !== null || !onRecheckProvider}
                  onClick={() => void onRecheckProvider?.(group.provider)}
                >
                  {busy === `login:${group.provider}` ? '检查中…' : '重新检查'}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="agent-card-note">{note}</div>
      <div className="agent-card-actions">
        <button
          className="res-btn"
          type="button"
          disabled={busy !== null}
          onClick={() => void onDetect(agent)}
        >
          {detecting ? '探测中…' : '探测'}
        </button>
        {hasThirdParty && (
          <button
            className="res-btn"
            type="button"
            disabled={busy !== null}
            onClick={() => void onVerifyAgent(agent)}
          >
            {verifyingAgent ? '验证中…' : '全部验证'}
          </button>
        )}
        <button className="res-btn" type="button" disabled={busy !== null} onClick={onManual}>
          手动接入模型
        </button>
      </div>
    </article>
  );
}

function ResourceModelDialog({
  agentId,
  agentName,
  onClose,
  onSaved,
  onError,
}: {
  agentId: string;
  agentName: string;
  onClose: () => void;
  onSaved: (store: ResourceStore) => void;
  onError: (message: string) => void;
}) {
  const [value, setValue] = useState(() => emptyModel(agentId));
  const [saving, setSaving] = useState(false);
  const save = async () => {
    if (!value.id.trim()) {
      onError('模型 ID 不能为空');
      return;
    }
    if (!value.vendor) {
      onError('请选择厂商');
      return;
    }
    if (agentId === 'pi' && !value.provider.trim()) {
      onError('pi 模型必须填写 provider');
      return;
    }
    setSaving(true);
    try {
      onSaved(
        await commands.addResourceModel({
          ...value,
          id: value.id.trim(),
          provider: value.provider.trim(),
          note: value.note.trim(),
        }),
      );
    } catch (error) {
      onError(`接入模型失败：${errorText(error)}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <ResourceDialog title={`手动接入模型：${agentName || agentId}`} onClose={onClose}>
      <label className="res-field">
        模型 ID
        <input
          autoFocus
          value={value.id}
          onChange={(event) => setValue({ ...value, id: event.target.value })}
        />
      </label>
      <label className="res-field">
        provider（可留空）
        <input
          value={value.provider}
          onChange={(event) => setValue({ ...value, provider: event.target.value })}
        />
      </label>
      <label className="res-field">
        厂商
        <select
          value={value.vendor}
          onChange={(event) => setValue({ ...value, vendor: event.target.value })}
        >
          {vendors.map((vendor) => (
            <option key={vendor}>{vendor}</option>
          ))}
        </select>
      </label>
      <label className="res-field">
        档位
        <select
          value={value.tier}
          onChange={(event) => setValue({ ...value, tier: event.target.value })}
        >
          {tiers.map((tier) => (
            <option value={tier} key={tier}>
              {tier || '未设置'}
            </option>
          ))}
        </select>
      </label>
      <label className="res-field">
        备注
        <input
          value={value.note}
          onChange={(event) => setValue({ ...value, note: event.target.value })}
        />
      </label>
      <ResourceDialogActions onClose={onClose} onSave={save} saving={saving} label="接入" />
    </ResourceDialog>
  );
}

function ManualAgentDialog({
  onClose,
  onSaved,
  onError,
}: {
  onClose: () => void;
  onSaved: (operation: ResourceOperation) => void;
  onError: (message: string) => void;
}) {
  const [value, setValue] = useState<AddManualAgentRequest>({ name: '', binary: '', args: [] });
  const [argsText, setArgsText] = useState('');
  const [saving, setSaving] = useState(false);
  const save = async () => {
    if (!value.name.trim() || !value.binary.trim()) {
      onError('名称和二进制名不能为空');
      return;
    }
    setSaving(true);
    try {
      onSaved(
        await commands.addManualAgent({
          ...value,
          name: value.name.trim(),
          binary: value.binary.trim(),
          args: parseCommandArgs(argsText),
        }),
      );
    } catch (error) {
      onError(`接入 agent 失败：${errorText(error)}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <ResourceDialog title="手动接入 agent" onClose={onClose}>
      <label className="res-field">
        名称
        <input
          autoFocus
          value={value.name}
          onChange={(event) => setValue({ ...value, name: event.target.value })}
        />
      </label>
      <label className="res-field">
        二进制名
        <input
          value={value.binary}
          onChange={(event) => setValue({ ...value, binary: event.target.value })}
        />
      </label>
      <label className="res-field">
        启动参数
        <input
          value={argsText}
          placeholder="含空格的参数可使用引号"
          onChange={(event) => setArgsText(event.target.value)}
        />
      </label>
      <ResourceDialogActions onClose={onClose} onSave={save} saving={saving} label="接入" />
    </ResourceDialog>
  );
}

function ResourceDialog({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="res-dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="res-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2>{title}</h2>
        {children}
      </section>
    </div>
  );
}

function ResourceDialogActions({
  onClose,
  onSave,
  saving,
  label,
}: {
  onClose: () => void;
  onSave: () => Promise<void>;
  saving: boolean;
  label: string;
}) {
  return (
    <div className="res-dialog-actions">
      <button className="res-btn" type="button" onClick={onClose}>
        取消
      </button>
      <button
        className="res-btn primary"
        type="button"
        disabled={saving}
        onClick={() => void onSave()}
      >
        {saving ? '保存中…' : label}
      </button>
    </div>
  );
}
