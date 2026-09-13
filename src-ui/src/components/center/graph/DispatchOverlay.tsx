import { useEffect, useMemo, useState } from 'react';
import { commands, type DispatchNodeResult, type DispatchStageRequest } from '../../../tauri';
import { contextPartsOf, serializeContext, serializeCurrentMessage } from '../../../graph/context';
import {
  appendDecision,
  applyDispatchResults,
  clearTargetSession,
  continuationSession,
  dispatchableNodeIds,
  markPending,
  redraftNode,
  routeLine,
  setReplyAdopted,
} from '../../../graph/dispatch';
import { loadDispatchSettings } from '../../../graph/dispatch-settings';
import type { DispatchableModel, Graph, Thinking } from '../../../graph/types';

type Phase = 'confirm' | 'running' | 'results';

interface Props {
  graph: Graph;
  models: DispatchableModel[];
  projectDir: string;
  onCommit: (graph: Graph) => Graph;
  onClose: () => void;
}

const resultStatus = (status: string) =>
  status === 'ok' ? '完成' : status === 'timeout' ? '超时' : '失败';

export function DispatchOverlay({ graph, models, projectDir, onCommit, onClose }: Props) {
  const initialIds = useMemo(() => dispatchableNodeIds(graph), [graph]);
  const [nodeIds, setNodeIds] = useState(initialIds);
  const [phase, setPhase] = useState<Phase>('confirm');
  const [runId, setRunId] = useState(() => crypto.randomUUID());
  const [targets, setTargets] = useState(() => Object.fromEntries(
    initialIds.map((id) => [id, { ...graph.nodes[id].target! }]),
  ));
  const [selection, setSelection] = useState<Record<string, { messageKeys: string[]; attachments: string[] }>>(() =>
    Object.fromEntries(initialIds.map((id) => {
      const parts = contextPartsOf(graph, id);
      return [id, {
        messageKeys: parts.map((part) => part.key),
        attachments: [...new Set(parts.flatMap((part) => part.message.attachments))],
      }];
    })),
  );
  const [preview, setPreview] = useState<{ totalRedacted: number; nodes: { nodeId: string; redacted: number; isolation: string }[] }>({ totalRedacted: 0, nodes: [] });
  const [previewError, setPreviewError] = useState('');
  const [statuses, setStatuses] = useState<Record<string, string>>({});
  const [results, setResults] = useState<DispatchNodeResult[]>([]);
  const [runError, setRunError] = useState('');
  const [restartNodes, setRestartNodes] = useState<Set<string>>(new Set());
  const [settings] = useState(loadDispatchSettings);

  const graphWithTargets = useMemo(() => {
    const nodes = { ...graph.nodes };
    for (const id of nodeIds) {
      if (nodes[id] && targets[id]) nodes[id] = { ...nodes[id], target: { ...targets[id] } };
    }
    return { ...graph, nodes };
  }, [graph, nodeIds, targets]);

  const request = useMemo<DispatchStageRequest>(() => ({
    runId,
    stage: 1,
    projectDir,
    settings: {
      timeoutSecs: settings.timeoutSecs,
      concurrency: settings.concurrency,
      redact: settings.redact,
      writeProjectDispatch: settings.writeProjectDispatch,
      ccSwitchPath: settings.ccSwitchPath,
    },
    nodes: nodeIds.map((id) => {
      const node = graphWithTargets.nodes[id];
      const target = node.target!;
      const selected = serializeContext(graphWithTargets, id, selection[id]);
      const model = models.find((item) =>
        item.agent === target.agent && item.id === target.model &&
        (item.provider ?? '') === (target.provider ?? ''));
      return {
        nodeId: id,
        agent: target.agent,
        provider: target.provider ?? '',
        model: target.model,
        vendor: model?.vendor ?? '',
        billingSource: model?.billing_source ?? '',
        role: target.role,
        tier: target.tier ?? model?.tier ?? '',
        thinking: target.thinking,
        visibility: target.visibility,
        prompt: selected.prompt,
        currentMessage: serializeCurrentMessage(node.user),
        materials: selected.materials,
        sessionId: restartNodes.has(id) ? '' : continuationSession(graphWithTargets, id) ?? '',
      };
    }),
  }), [graphWithTargets, models, nodeIds, projectDir, restartNodes, runId, selection, settings]);

  useEffect(() => {
    if (phase !== 'confirm' || request.nodes.length === 0) return;
    let cancelled = false;
    commands.previewDispatch(request)
      .then((value) => { if (!cancelled) { setPreview(value); setPreviewError(''); } })
      .catch((error) => { if (!cancelled) setPreviewError(String(error)); });
    return () => { cancelled = true; };
  }, [phase, request]);

  useEffect(() => {
    if (phase !== 'running') return;
    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/event').then(({ listen }) => listen<{ runId: string; nodeId: string; status: string }>(
      'dispatch-progress',
      (event) => {
        if (event.payload.runId !== runId) return;
        setStatuses((current) => ({ ...current, [event.payload.nodeId]: event.payload.status }));
      },
    )).then((stop) => { unlisten = stop; }).catch(() => {});
    return () => { unlisten?.(); };
  }, [phase, runId]);

  const toggleMessage = (nodeId: string, key: string) => setSelection((current) => {
    const row = current[nodeId];
    const has = row.messageKeys.includes(key);
    return { ...current, [nodeId]: { ...row, messageKeys: has ? row.messageKeys.filter((item) => item !== key) : [...row.messageKeys, key] } };
  });
  const toggleAttachment = (nodeId: string, path: string) => setSelection((current) => {
    const row = current[nodeId];
    const has = row.attachments.includes(path);
    return { ...current, [nodeId]: { ...row, attachments: has ? row.attachments.filter((item) => item !== path) : [...row.attachments, path] } };
  });

  const start = async () => {
    if (request.nodes.length === 0) return;
    const pending = markPending(graphWithTargets, nodeIds);
    onCommit(pending);
    setPhase('running');
    setStatuses(Object.fromEntries(nodeIds.map((id) => [id, '排队'])));
    setRunError('');
    try {
      const output = await commands.dispatchGraphStage(request);
      setResults(output.results);
      onCommit(applyDispatchResults(pending, output.results));
      setPhase('results');
    } catch (error) {
      const message = String(error);
      setRunError(message);
      const failed = request.nodes.map((node) => ({
        nodeId: node.nodeId, status: 'error', text: '', modelReported: '', sessionId: '',
        tokensIn: 0, tokensOut: 0, durationMs: 0, error: message,
      }));
      onCommit(applyDispatchResults(pending, failed));
      setPhase('results');
    }
  };

  const retry = (nodeId: string, upgrade: boolean) => {
    const next = redraftNode(graph, nodeId, upgrade);
    onCommit(next);
    const node = next.nodes[nodeId];
    setNodeIds([nodeId]);
    setTargets({ [nodeId]: { ...node.target! } });
    const parts = contextPartsOf(next, nodeId);
    setSelection({ [nodeId]: {
      messageKeys: parts.map((part) => part.key),
      attachments: [...new Set(parts.flatMap((part) => part.message.attachments))],
    } });
    setRunId(crypto.randomUUID());
    setRestartNodes(new Set());
    setResults([]);
    setPhase('confirm');
  };

  const handoff = () => {
    const summary = results.map((result) => {
      const node = graph.nodes[result.nodeId];
      const route = node?.target ? `${node.target.agent}/${node.target.model}` : result.nodeId;
      return `### ${route}\n${result.status === 'ok' ? result.text : `失败：${result.error}`}`;
    }).join('\n\n');
    onCommit(appendDecision(graph, results.map((result) => result.nodeId), `请裁决以下结果。\n\n${summary}`));
    onClose();
  };

  const startNewSession = (nodeId: string) => {
    onCommit(clearTargetSession(graphWithTargets, nodeId));
    setRestartNodes((current) => new Set(current).add(nodeId));
  };

  return (
    <div className="dispatch-overlay" onMouseDown={phase === 'running' ? undefined : onClose}>
      <div className="dispatch-modal" onMouseDown={(event) => event.stopPropagation()}>
        <header className="dispatch-head">
          <div>
            <div className="dispatch-title">{phase === 'confirm' ? '确认派发' : phase === 'running' ? '执行中' : '本阶段结果'}</div>
            <div className="dispatch-sub">run {runId}</div>
          </div>
          {phase !== 'running' && <button className="dispatch-close" onClick={onClose}>关闭</button>}
        </header>

        {phase === 'confirm' && (
          <div className="dispatch-body">
            {nodeIds.length === 0 && <div className="dispatch-empty">当前没有父节点已完成的待派节点。</div>}
            {nodeIds.map((id) => {
              const node = graphWithTargets.nodes[id];
              const target = targets[id];
              const blind = target.visibility === '盲';
              const isolation = preview.nodes.find((item) => item.nodeId === id)?.isolation ?? '核对中';
              const parts = contextPartsOf(graphWithTargets, id);
              const canRestart = !!continuationSession(graphWithTargets, id) && !restartNodes.has(id);
              return (
                <section key={id} className="dispatch-route-card">
                  <div className="dispatch-route-line">{routeLine(node, models, isolation)}</div>
                  <div className="dispatch-controls">
                    <label>目标<select
                      value={JSON.stringify([target.agent, target.provider ?? '', target.model])}
                      onChange={(event) => {
                        const [agent, provider, modelId] = JSON.parse(event.target.value) as string[];
                        const hit = models.find((item) => item.agent === agent && item.provider === provider && item.id === modelId);
                        setTargets((current) => ({
                          ...current,
                          [id]: { ...target, agent, provider, model: modelId, tier: hit?.tier || target.tier },
                        }));
                      }}
                    >
                      {models.map((item) => (
                        <option key={JSON.stringify([item.agent, item.provider, item.id])} value={JSON.stringify([item.agent, item.provider, item.id])}>
                          {item.agent}/{item.provider ? `${item.provider}/` : ''}{item.id}
                        </option>
                      ))}
                    </select></label>
                    <label>档位<input value={target.tier ?? ''} onChange={(event) => setTargets((current) => ({ ...current, [id]: { ...target, tier: event.target.value } }))} /></label>
                    <label>thinking<select value={target.thinking} onChange={(event) => setTargets((current) => ({ ...current, [id]: { ...target, thinking: event.target.value as Thinking } }))}>
                      <option>关</option><option>低</option><option>中</option><option>高</option>
                    </select></label>
                    {canRestart && <button onClick={() => startNewSession(id)}>新开会话</button>}
                    {target.agent === 'cc-switch' && <span>单轮</span>}
                  </div>
                  {blind && (
                    <div className="dispatch-materials">
                      <div className="dispatch-material-title">本次外发</div>
                      {parts.map((part) => (
                        <div key={part.key} className="dispatch-material-row">
                          {part.message.text !== '' && <label><input type="checkbox" checked={selection[id]?.messageKeys.includes(part.key) ?? false} onChange={() => toggleMessage(id, part.key)} />{part.label}：{part.message.text.slice(0, 100)}</label>}
                          {part.message.attachments.map((path) => <label key={path}><input type="checkbox" checked={selection[id]?.attachments.includes(path) ?? false} onChange={() => toggleAttachment(id, path)} />附件：{path}</label>)}
                        </div>
                      ))}
                    </div>
                  )}
                </section>
              );
            })}
            <div className="dispatch-authorization">
              材料将发送至 {[...new Set(request.nodes.map((node) => node.vendor || node.agent))].join('、') || '所选厂商'}
              {preview.totalRedacted > 0 ? `；已脱敏 ${preview.totalRedacted} 处` : ''}
            </div>
            {previewError && <div className="dispatch-error">{previewError}</div>}
            <div className="dispatch-actions">
              <button onClick={onClose}>取消</button>
              <button className="dispatch-primary" disabled={nodeIds.length === 0 || !!previewError} onClick={start}>确认派发</button>
            </div>
          </div>
        )}

        {phase === 'running' && (
          <div className="dispatch-body">
            {request.nodes.map((node) => (
              <div key={node.nodeId} className="dispatch-running-row">
                <span>{node.role || node.nodeId} · {node.agent}/{node.model}</span><span>{statuses[node.nodeId] ?? '排队'}</span>
                <button onClick={() => commands.cancelDispatchNode(runId, node.nodeId).catch(() => false)}>取消</button>
              </div>
            ))}
          </div>
        )}

        {phase === 'results' && (
          <div className="dispatch-body">
            {runError && <div className="dispatch-error">{runError}</div>}
            <div className="dispatch-result-lines">
              {results.map((result) => <div key={result.nodeId}>{routeLine(graph.nodes[result.nodeId], models, result.isolation)}</div>)}
            </div>
            <div className="dispatch-result-grid">
              {results.map((result) => {
                const node = graph.nodes[result.nodeId];
                const adopted = node?.reply?.adopted === true;
                return (
                <section key={result.nodeId} className={`dispatch-result-card${adopted ? ' adopted' : ''}`}>
                  <div className="dispatch-result-head">
                    <span>{node?.target?.agent}/{node?.target?.model}{node?.target?.agent === 'cc-switch' ? ' · 单轮' : ''}</span>
                    <span>{resultStatus(result.status)}</span>
                  </div>
                  {result.truncated && <div className="dispatch-warning">输出已截断</div>}
                  <div className="dispatch-result-text">{result.status === 'ok' ? result.text : result.error}</div>
                  <div className="dispatch-card-actions">
                    <button onClick={() => onCommit(setReplyAdopted(graph, result.nodeId, !adopted))}>{adopted ? '取消采纳' : '采纳'}</button>
                    <button onClick={() => retry(result.nodeId, false)}>重派</button>
                    <button onClick={() => retry(result.nodeId, true)}>升级</button>
                  </div>
                </section>
              );})}
            </div>
            <table className="dispatch-usage"><thead><tr><th>阶段</th><th>agent/模型</th><th>in / out tokens</th><th>cache_read</th><th>耗时</th><th>结果</th></tr></thead>
              <tbody>{results.map((result) => { const target = graph.nodes[result.nodeId]?.target; return <tr key={result.nodeId}><td>{request.stage}</td><td>{target?.agent}/{target?.model}{target?.agent === 'cc-switch' ? ' · 单轮' : ''}</td><td>{result.tokensIn || '—'} / {result.tokensOut || '—'}</td><td>{result.cacheRead || '—'}</td><td>{result.durationMs} ms</td><td>{resultStatus(result.status)}</td></tr>; })}</tbody>
            </table>
            <details className="dispatch-records"><summary>留痕</summary>
              {results.map((result) => <details key={result.nodeId}><summary>{result.dispatchFile || `${result.nodeId}（未写入）`}</summary><pre>{result.frontmatter}{'\n\n'}{result.outputPreview}</pre></details>)}
            </details>
            <div className="dispatch-actions"><button className="dispatch-primary" onClick={handoff}>交人裁决</button></div>
          </div>
        )}
      </div>
    </div>
  );
}
