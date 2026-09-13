// 接入页（TASK-07）：全屏覆盖层，与「讲解」ExplainOverlay 同形态。
// agent 卡片网格：标记、名称、版本、检测状态、全部模型与状态；
// 卡底「探测 / 冒烟 / 手动输入」。冒烟通过（已接入）的模型才进左栏。
// cc-switch 降级为发现提示：只显示「cc-switch 里有 X，建议加进 pi」。

import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { commands, isTauri } from '../../tauri';
import { loadDispatchSettings } from '../../graph/dispatch-settings';
import {
  agentMark,
  errorText,
  groupModelsByProvider,
  parseCommandArgs,
  type AddManualAgentRequest,
  type CcSwitchHint,
  type ResourceAgent,
  type ResourceModel,
  type ResourceModelRequest,
  type ResourceOperation,
  type ResourceStore,
} from '../../resources/types';
import { notifyResourcesChanged } from '../../resources/mention';
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

  const smokeAgent = async (agent: ResourceAgent) => {
    if (agent.models.length === 0) {
      setNotes((value) => ({ ...value, [agent.id]: '没有可冒烟的模型' }));
      return;
    }
    setBusy(`smoke-agent:${agent.id}`);
    setNotes((value) => ({ ...value, [agent.id]: '冒烟中…' }));
    try {
      const result = await commands.smokeAgentModels(agent.id);
      finishOperation(result);
      setNotes((value) => ({
        ...value,
        [agent.id]: result.errors.length ? '冒烟完成，有失败项' : '冒烟完成',
      }));
    } catch (value) {
      const message = `冒烟失败：${errorText(value)}`;
      setError(message);
      setNotes((notesValue) => ({ ...notesValue, [agent.id]: message }));
    } finally {
      setBusy(null);
    }
  };

  const smokeModel = async (model: ResourceModel) => {
    setBusy(`smoke:${model.agent}:${model.provider}:${model.id}`);
    setNotes((value) => ({ ...value, [model.agent]: '冒烟中…' }));
    try {
      const value = await commands.smokeResourceModel({
        agent_id: model.agent,
        id: model.id,
        provider: model.provider,
      });
      setStore(value);
      setError('');
      setNotes((notesValue) => ({ ...notesValue, [model.agent]: `${model.id} 冒烟完成` }));
      notifyResourcesChanged();
    } catch (value) {
      const message = `冒烟失败：${errorText(value)}`;
      setError(message);
      setNotes((notesValue) => ({ ...notesValue, [model.agent]: message }));
    } finally {
      setBusy(null);
    }
  };

  const updateModel = async (
    model: ResourceModel,
    patch: Partial<Pick<ResourceModel, 'vendor' | 'tier' | 'note'>>,
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
          检测本机已安装的 agent，冒烟通过后加入左栏可派发清单。
          <button
            className="res-btn resources-detect-all"
            type="button"
            disabled={busy !== null}
            onClick={() => void detectAll()}
          >
            {busy === 'detect:all' ? '探测中…' : '全部探测'}
          </button>
        </p>
        {error && <div className="resources-error">{error}</div>}
        {hints.length > 0 && (
          <div className="resources-hints">
            {hints.map((hint) => (
              <div className="resources-hint" key={hint.name}>
                cc-switch 里有 {hint.name}（{hint.models.length} 个模型
                {hint.has_token ? '' : '，未配置密钥'}），建议加进 pi
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
                busy={busy}
                note={notes[agent.id] || ''}
                onDetect={detect}
                onSmokeAgent={smokeAgent}
                onSmokeModel={smokeModel}
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
            手动添加 agent
          </button>
        </div>
        {store && modelAgent && (
          <ResourceModelDialog
            agentId={modelAgent}
            agentName={
              store.agents.find((agent) => agent.id === modelAgent)?.name || modelAgent
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

function AgentCard({
  agent,
  busy,
  note,
  onDetect,
  onSmokeAgent,
  onSmokeModel,
  onUpdate,
  onRemove,
  onManual,
}: {
  agent: ResourceAgent;
  busy: string | null;
  note: string;
  onDetect: (agent: ResourceAgent) => Promise<void>;
  onSmokeAgent: (agent: ResourceAgent) => Promise<void>;
  onSmokeModel: (model: ResourceModel) => Promise<void>;
  onUpdate: (
    model: ResourceModel,
    patch: Partial<Pick<ResourceModel, 'vendor' | 'tier' | 'note'>>,
  ) => Promise<void>;
  onRemove: (model: ResourceModel) => Promise<void>;
  onManual: () => void;
}) {
  const groups = groupModelsByProvider(agent.models);
  const detecting = busy === `detect:${agent.id}`;
  const smokingAgent = busy === `smoke-agent:${agent.id}`;
  return (
    <article className="agent-card">
      <div className="agent-card-head">
        <span className="agent-card-mark">{agentMark(agent.id)}</span>
        <span className="agent-card-name">{agent.name || agent.id}</span>
        <span className={`detection ${agent.detected ? 'detected' : 'missing'}`}>
          {agent.detected ? '已检测' : '未检测'}
        </span>
      </div>
      <div className="agent-card-version">{agent.version || '版本未知'}</div>
      <div className="agent-card-models">
        {groups.length === 0 && <div className="agent-empty">暂无模型</div>}
        {groups.map((group) => (
          <div className="agent-card-group-wrap" key={group.label || 'models'}>
            {group.label && <div className="agent-card-group">{group.label}</div>}
            {group.models.map((model) => {
              const key = `${model.agent}:${model.provider}:${model.id}`;
              const smoking = busy === `smoke:${key}`;
              const updating = busy === `update:${key}`;
              const removing = busy === `remove:${key}`;
              return (
                <div className="agent-card-model" key={`${model.provider}:${model.id}`}>
                  <div className="agent-card-model-main">
                    <span className="agent-card-model-id" title={model.id}>
                      {model.id}
                    </span>
                    <StatusTag status={model.status} />
                  </div>
                  <details className="agent-card-model-detail">
                    <summary>详情</summary>
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
                    <div className="agent-card-model-meta" title={model.source}>
                      {model.source || '来源未记录'}
                    </div>
                    {model.smoked_at && (
                      <div className="agent-card-model-meta">
                        最近冒烟：{model.smoked_at}
                        {model.smoke_model ? `（${model.smoke_model}）` : ''}
                      </div>
                    )}
                    {model.meta && <div className="agent-card-model-meta">{model.meta}</div>}
                    {model.smoke_failure && (
                      <div className="agent-card-model-meta failure">
                        冒烟失败：{model.smoke_failure}
                      </div>
                    )}
                    {model.note && (
                      <div className="agent-card-model-meta">备注：{model.note}</div>
                    )}
                    <div className="agent-card-model-actions">
                      <button
                        className="row-action"
                        type="button"
                        disabled={busy !== null}
                        onClick={() => void onSmokeModel(model)}
                      >
                        {smoking ? '冒烟中…' : model.status === '已接入' ? '重新冒烟' : '冒烟'}
                      </button>
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
                  </details>
                </div>
              );
            })}
          </div>
        ))}
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
        <button
          className="res-btn"
          type="button"
          disabled={busy !== null || agent.models.length === 0}
          onClick={() => void onSmokeAgent(agent)}
        >
          {smokingAgent ? '冒烟中…' : '冒烟'}
        </button>
        <button className="res-btn" type="button" disabled={busy !== null} onClick={onManual}>
          手动输入
        </button>
      </div>
    </article>
  );
}

function StatusTag({ status }: { status: string }) {
  const normalized = status || '未接入';
  const className =
    normalized === '已接入'
      ? 'connected'
      : normalized === '冒烟失败'
        ? 'failed'
        : normalized === '待冒烟'
          ? 'pending'
          : 'disconnected';
  return <span className={`resource-status ${className}`}>{normalized}</span>;
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
      onError(`添加资源失败：${errorText(error)}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <ResourceDialog title={`手动输入：${agentName || agentId}`} onClose={onClose}>
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
      <ResourceDialogActions onClose={onClose} onSave={save} saving={saving} label="添加" />
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
      onError(`添加 agent 失败：${errorText(error)}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <ResourceDialog title="手动添加 agent" onClose={onClose}>
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
      <ResourceDialogActions onClose={onClose} onSave={save} saving={saving} label="添加" />
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
