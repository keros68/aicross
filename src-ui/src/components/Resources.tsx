import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type {
  AddManualAgentRequest,
  ResourceModelRequest,
  CcSwitchProvider,
  ResourceAgent,
  ResourceModel,
  ResourceOperation,
  ResourceStore,
  Tool,
} from "../types";

const vendors = ["Anthropic", "OpenAI", "Moonshot", "Zhipu", "DeepSeek", "MiniMax", "Alibaba", "StepFun", "其他"];
const tiers = ["", "低", "中", "高"];

const agentNames: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  kimi: "Kimi Code CLI",
  pi: "pi",
  "cc-switch": "cc-switch",
};

const emptyModel = (agentId: string): ResourceModelRequest => ({
  agent_id: agentId,
  id: "",
  provider: "",
  vendor: "其他",
  tier: "",
  note: "",
});

function notifyResourcesChanged() {
  window.dispatchEvent(new Event("aicross:resources-changed"));
}

function notifyToolsChanged() {
  window.dispatchEvent(new Event("aicross:tools-changed"));
}

function operationErrors(operation: ResourceOperation) {
  return operation.errors.length > 0 ? operation.errors.join("\n") : "";
}

function parseCommandArgs(text: string) {
  const args: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  let started = false;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      if (character === quote) {
        quote = null;
      } else if (quote === '"' && character === "\\" && (text[index + 1] === '"' || text[index + 1] === "\\")) {
        current += text[index + 1];
        index += 1;
      } else {
        current += character;
      }
      started = true;
    } else if (/\s/.test(character)) {
      if (started) {
        args.push(current);
        current = "";
        started = false;
      }
    } else if (character === '"' || character === "'") {
      quote = character;
      started = true;
    } else {
      current += character;
      started = true;
    }
  }
  if (quote) throw new Error("启动参数引号未闭合");
  if (started) args.push(current);
  return args;
}

export function Resources() {
  const [store, setStore] = useState<ResourceStore | null>(null);
  const [toolNames, setToolNames] = useState<Record<string, string>>({});
  const [expanded, setExpanded] = useState<string | null>(null);
  const [modelAgent, setModelAgent] = useState<string | null>(null);
  const [manualAgentOpen, setManualAgentOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    try {
      const [value, tools] = await Promise.all([api.resources(), api.tools()]);
      setStore(value);
      setToolNames(Object.fromEntries(tools.map((tool) => [tool.id, tool.name])));
      setError("");
    } catch (value) {
      setError(`读取资源失败：${String(value)}`);
    }
  }, []);

  useEffect(() => {
    void load();
    window.addEventListener("aicross:resources-changed", load);
    return () => window.removeEventListener("aicross:resources-changed", load);
  }, [load]);

  const detect = async (agent: ResourceAgent) => {
    setBusy(`detect:${agent.id}`);
    try {
      const value = await api.detectResourceAgent(agent.id);
      setStore(value.store);
      setError(operationErrors(value));
      notifyResourcesChanged();
    } catch (value) {
      setError(`探测失败：${String(value)}`);
    } finally {
      setBusy(null);
    }
  };

  const updateModel = async (model: ResourceModel, patch: Partial<Pick<ResourceModel, "vendor" | "tier" | "note">>) => {
    const key = `update:${model.agent}:${model.provider}:${model.id}`;
    setBusy(key);
    try {
      const value = await api.updateResourceModel({
        agent_id: model.agent,
        id: model.id,
        provider: model.provider,
        vendor: patch.vendor ?? model.vendor,
        tier: patch.tier ?? model.tier,
        note: patch.note ?? model.note,
      });
      setStore(value);
      setError("");
      notifyResourcesChanged();
    } catch (value) {
      setError(`更新资源失败：${String(value)}`);
    } finally {
      setBusy(null);
    }
  };

  const smoke = async (model: ResourceModel) => {
    const key = `smoke:${model.agent}:${model.provider}:${model.id}`;
    setBusy(key);
    try {
      const value = await api.smokeResourceModel({ agent_id: model.agent, id: model.id, provider: model.provider });
      setStore(value);
      setError("");
      notifyResourcesChanged();
    } catch (value) {
      setError(`冒烟失败：${String(value)}`);
    } finally {
      setBusy(null);
    }
  };

  const remove = async (model: ResourceModel) => {
    const key = `remove:${model.agent}:${model.provider}:${model.id}`;
    setBusy(key);
    try {
      const value = await api.removeResourceModel({ agent_id: model.agent, id: model.id, provider: model.provider });
      setStore(value);
      setError("");
      notifyResourcesChanged();
    } catch (value) {
      setError(`移除资源失败：${String(value)}`);
    } finally {
      setBusy(null);
    }
  };

  if (!store) return <div className="empty-state">正在读取资源…</div>;

  return (
    <div className="resources-pane pane-body">
      <div className="pane-actions"><div className="action-row"><button className="button small" onClick={() => void load()}>刷新</button><button className="button small" onClick={() => setManualAgentOpen(true)}>手动添加</button></div></div>
      {error && <div className="error-line">{error}</div>}
      <div className="resource-list">
        {store.agents.map((agent) => {
          const isExpanded = expanded === agent.id;
          const detecting = busy === `detect:${agent.id}`;
          return (
            <article className="resource-agent" key={agent.id}>
              <div className="resource-agent-head">
                <button className="resource-agent-summary" onClick={() => setExpanded(isExpanded ? null : agent.id)}>
                  <span className="resource-caret">{isExpanded ? "⌄" : "›"}</span>
                  <span><strong>{agentNames[agent.id] ?? toolNames[agent.id] ?? agent.id}</strong><small>{agent.version || "版本未知"}</small></span>
                </button>
                <span className={`detection ${agent.detected ? "detected" : "missing"}`}>{agent.detected ? "已检测" : "未检测"}</span>
              </div>
              <div className="resource-agent-actions">
                <button className="button small" disabled={detecting} onClick={() => void detect(agent)}>{detecting ? "探测中…" : "探测"}</button>
                <button className="button small" onClick={() => setModelAgent(agent.id)}>手动输入</button>
              </div>
              {isExpanded && <div className="resource-model-list">
                {agent.models.length === 0 ? <div className="empty-state">暂无模型</div> : agent.models.map((model) => (
                  <ModelRow key={`${model.provider}:${model.id}`} model={model} busy={busy} onUpdate={updateModel} onSmoke={smoke} onRemove={remove} />
                ))}
              </div>}
            </article>
          );
        })}
      </div>
      {modelAgent && <ResourceModelDialog agentId={modelAgent} onClose={() => setModelAgent(null)} onSaved={(value) => { setStore(value); setModelAgent(null); setError(""); notifyResourcesChanged(); }} onError={setError} />}
      {manualAgentOpen && <ManualAgentDialog onClose={() => setManualAgentOpen(false)} onSaved={(value) => { setStore(value); setManualAgentOpen(false); setError(""); notifyResourcesChanged(); notifyToolsChanged(); }} onError={setError} />}
    </div>
  );
}

function ModelRow({ model, busy, onUpdate, onSmoke, onRemove }: {
  model: ResourceModel;
  busy: string | null;
  onUpdate: (model: ResourceModel, patch: Partial<Pick<ResourceModel, "vendor" | "tier" | "note">>) => Promise<void>;
  onSmoke: (model: ResourceModel) => Promise<void>;
  onRemove: (model: ResourceModel) => Promise<void>;
}) {
  const key = `${model.agent}:${model.provider}:${model.id}`;
  const smoking = busy === `smoke:${key}`;
  const updating = busy === `update:${key}`;
  const removing = busy === `remove:${key}`;
  return (
    <div className="resource-model">
      <div className="resource-model-title mono" title={model.id}>{model.id}</div>
      <div className="resource-model-meta">{model.provider && <span>{model.provider}</span>}<StatusTag status={model.status} /></div>
      <div className="resource-model-fields">
        <label>厂商<select value={model.vendor} disabled={updating} onChange={(event) => void onUpdate(model, { vendor: event.target.value })}>{vendors.map((vendor) => <option key={vendor}>{vendor}</option>)}</select></label>
        <label>档位<select value={model.tier} disabled={updating} onChange={(event) => void onUpdate(model, { tier: event.target.value })}>{tiers.map((tier) => <option value={tier} key={tier}>{tier || "未设置"}</option>)}</select></label>
      </div>
      <div className="resource-source" title={model.source}>{model.source || "来源未记录"}</div>
      {model.smoked_at && <div className="resource-smoked">最近冒烟：{model.smoked_at}{model.smoke_model ? `（${model.smoke_model}）` : ""}</div>}
      {model.meta && <div className="resource-meta">{model.meta}</div>}
      {model.note && <div className="resource-note">{model.note}</div>}
      <div className="resource-model-actions">
        {model.status === "已接入" ? <>
          <button className="button small" disabled={smoking || removing} onClick={() => void onRemove(model)}>{removing ? "移除中…" : "移除"}</button>
          <button className="button small" disabled={smoking || removing} onClick={() => void onSmoke(model)}>{smoking ? "冒烟中…" : "重新冒烟"}</button>
        </> : <button className="button small" disabled={smoking || removing} onClick={() => void onSmoke(model)}>{smoking ? "冒烟中…" : "冒烟"}</button>}
      </div>
    </div>
  );
}

function StatusTag({ status }: { status: string }) {
  const className = status === "已接入" ? "connected" : status === "冒烟失败" ? "failed" : "pending";
  return <span className={`resource-status ${className}`}>{status || "未接入"}</span>;
}

function ResourceModelDialog({ agentId, onClose, onSaved, onError }: { agentId: string; onClose: () => void; onSaved: (store: ResourceStore) => void; onError: (message: string) => void }) {
  const [value, setValue] = useState(() => emptyModel(agentId));
  const [saving, setSaving] = useState(false);
  const save = async () => {
    if (!value.id.trim()) { onError("模型 ID 不能为空"); return; }
    if (!value.vendor) { onError("请选择权重厂商"); return; }
    if (agentId === "pi" && !value.provider.trim()) { onError("pi 模型必须填写 provider"); return; }
    setSaving(true);
    try {
      onSaved(await api.addResourceModel({ ...value, id: value.id.trim(), provider: value.provider.trim(), note: value.note.trim() }));
    } catch (error) {
      onError(`添加资源失败：${String(error)}`);
    } finally {
      setSaving(false);
    }
  };
  return <Dialog title={`手动输入：${agentNames[agentId] ?? agentId}`} onClose={onClose}>
    <label className="field-label">模型 ID<input autoFocus value={value.id} onChange={(event) => setValue({ ...value, id: event.target.value })} /></label>
    <label className="field-label">provider（可留空）<input value={value.provider} onChange={(event) => setValue({ ...value, provider: event.target.value })} /></label>
    <label className="field-label">权重厂商<select value={value.vendor} onChange={(event) => setValue({ ...value, vendor: event.target.value })}>{vendors.map((vendor) => <option key={vendor}>{vendor}</option>)}</select></label>
    <label className="field-label">档位<select value={value.tier} onChange={(event) => setValue({ ...value, tier: event.target.value })}>{tiers.map((tier) => <option value={tier} key={tier}>{tier || "未设置"}</option>)}</select></label>
    <label className="field-label">备注<input value={value.note} onChange={(event) => setValue({ ...value, note: event.target.value })} /></label>
    <DialogActions onClose={onClose} onSave={save} saving={saving} label="添加" />
  </Dialog>;
}

function ManualAgentDialog({ onClose, onSaved, onError }: { onClose: () => void; onSaved: (store: ResourceStore) => void; onError: (message: string) => void }) {
  const [value, setValue] = useState<AddManualAgentRequest>({ name: "", binary: "", args: [] });
  const [argsText, setArgsText] = useState("");
  const [saving, setSaving] = useState(false);
  const save = async () => {
    if (!value.name.trim() || !value.binary.trim()) { onError("名称和二进制名不能为空"); return; }
    setSaving(true);
    try {
      onSaved(await api.addManualAgent({ ...value, name: value.name.trim(), binary: value.binary.trim(), args: parseCommandArgs(argsText) }));
    } catch (error) {
      onError(`添加 agent 失败：${String(error)}`);
    } finally {
      setSaving(false);
    }
  };
  return <Dialog title="手动添加 agent" onClose={onClose}>
    <label className="field-label">名称<input autoFocus value={value.name} onChange={(event) => setValue({ ...value, name: event.target.value })} /></label>
    <label className="field-label">二进制名<input value={value.binary} onChange={(event) => setValue({ ...value, binary: event.target.value })} /></label>
    <label className="field-label">启动参数<input value={argsText} placeholder="含空格的参数可使用引号" onChange={(event) => setArgsText(event.target.value)} /></label>
    <DialogActions onClose={onClose} onSave={save} saving={saving} label="添加" />
  </Dialog>;
}

export function ResourceConnectionDialog({ onClose }: { onClose: () => void }) {
  const [store, setStore] = useState<ResourceStore | null>(null);
  const [tools, setTools] = useState<Tool[]>([]);
  const [providers, setProviders] = useState<CcSwitchProvider[]>([]);
  const [agentIds, setAgentIds] = useState<string[]>([]);
  const [providerNames, setProviderNames] = useState<string[]>([]);
  const [busy, setBusy] = useState<"detect" | "smoke" | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    Promise.all([api.resources(), api.tools()]).then(([resourceStore, detectedTools]) => {
      setStore(resourceStore);
      setTools(detectedTools);
    }).catch((value) => setError(`读取 agent 状态失败：${String(value)}`));
    api.connectionOverview().then((overview) => {
      setProviders(overview.cc_switch_providers);
    }).catch((value) => setError(`读取 cc-switch 失败：${String(value)}`));
  }, []);

  const toggle = (value: string, values: string[], setValues: (next: string[]) => void) => setValues(values.includes(value) ? values.filter((item) => item !== value) : [...values, value]);
  const detectAll = async () => {
    setBusy("detect");
    try {
      const value = await api.detectAllResources([]);
      setStore(value.store);
      if (value.cc_switch_providers !== null) setProviders(value.cc_switch_providers);
      setError(operationErrors(value));
      notifyResourcesChanged();
    } catch (value) { setError(`全部探测失败：${String(value)}`); } finally { setBusy(null); }
  };
  const smokeSelected = async () => {
    setBusy("smoke");
    try {
      const value = await api.smokeSelectedResources({ agent_ids: agentIds, cc_switch_providers: providerNames });
      setStore(value.store);
      if (value.cc_switch_providers !== null) setProviders(value.cc_switch_providers);
      setError(operationErrors(value));
      notifyResourcesChanged();
    } catch (value) { setError(`冒烟失败：${String(value)}`); } finally { setBusy(null); }
  };

  return <Dialog title="接入资源" onClose={onClose} wide>
    {error && <div className="error-line modal-error">{error}</div>}
    <section className="connection-section"><h3>检测到的 agent</h3>{store ? store.agents.filter((agent) => agent.detected && agent.id !== "cc-switch").map((agent) => <label className="check-row" key={agent.id}><input type="checkbox" checked={agentIds.includes(agent.id)} onChange={() => toggle(agent.id, agentIds, setAgentIds)} />{agentNames[agent.id] ?? tools.find((tool) => tool.id === agent.id)?.name ?? agent.id}<small>已检测</small></label>) : <div className="hint">正在读取…</div>}</section>
    <section className="connection-section"><h3>cc-switch 里的 provider</h3>{providers.length === 0 ? <div className="hint">未检测到 provider</div> : providers.map((provider) => <label className="check-row" key={provider.name}><input type="checkbox" checked={providerNames.includes(provider.name)} onChange={() => toggle(provider.name, providerNames, setProviderNames)} />{provider.name}<small>{provider.has_token ? "已配置" : "未配置"}</small></label>)}</section>
    <div className="modal-actions"><button className="button" onClick={onClose}>关闭</button><button className="button" disabled={busy !== null} onClick={() => void detectAll()}>{busy === "detect" ? "探测中…" : "全部探测"}</button><button className="button primary" disabled={busy !== null || (agentIds.length === 0 && providerNames.length === 0)} onClick={() => void smokeSelected()}>{busy === "smoke" ? "冒烟中…" : "冒烟所选"}</button></div>
  </Dialog>;
}

function Dialog({ title, onClose, wide = false, children }: { title: string; onClose: () => void; wide?: boolean; children: React.ReactNode }) {
  return <div className="modal-backdrop" role="presentation" onMouseDown={onClose}><section className={`modal ${wide ? "modal-wide" : ""}`} role="dialog" aria-modal="true" aria-label={title} onMouseDown={(event) => event.stopPropagation()}><h2>{title}</h2>{children}</section></div>;
}

function DialogActions({ onClose, onSave, saving, label }: { onClose: () => void; onSave: () => Promise<void>; saving: boolean; label: string }) {
  return <div className="modal-actions"><button className="button" onClick={onClose}>取消</button><button className="button primary" disabled={saving} onClick={() => void onSave()}>{saving ? "保存中…" : label}</button></div>;
}
