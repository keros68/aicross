import { Fragment, useEffect, useMemo, useState } from 'react';
import { commands, type DispatchNodeResult, type DispatchStageRequest } from '../../../tauri';
import { contextPartsOf, originalRequestOf, serializeContext, serializeCurrentMessage, upstreamArtifactsOf } from '../../../graph/context';
import {
  appendDecision,
  applyDispatchResults,
  clearTargetSession,
  continuationSession,
  dispatchableNodeIds,
  markPending,
  graphStage,
  groupResultsByStage,
  isSmallEconomyTask,
  needsChainThinking,
  promptForRole,
  reducePipelineStep,
  redraftNode,
  routeLine,
  setReplyAdopted,
  withinRequestLimit,
} from '../../../graph/dispatch';
import { presetNameOf } from '../../../graph/persist';
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
  status === 'ok' ? '完成' : status === 'timeout' ? '超时' : status === 'identity_mismatch' ? '真身不符' : status === 'validation_failed' ? '验证失败' : '失败';

export function DispatchOverlay({ graph, models, projectDir, onCommit, onClose }: Props) {
  const initialIds = useMemo(() => dispatchableNodeIds(graph), [graph]);
  const [nodeIds, setNodeIds] = useState(initialIds);
  const [phase, setPhase] = useState<Phase>('confirm');
  const [runId, setRunId] = useState(() => crypto.randomUUID());
  const [targets, setTargets] = useState(() => Object.fromEntries(initialIds.map((id) => {
    const target = { ...graph.nodes[id].target! };
    if (target.role === '执行' && (target.thinking === '关' || target.thinking === '低') && needsChainThinking(originalRequestOf(graph))) {
      target.thinking = '中';
    }
    return [id, target];
  })));
  const [selection, setSelection] = useState<Record<string, { messageKeys: string[]; attachments: string[] }>>(() =>
    Object.fromEntries(initialIds.map((id) => {
      const parts = contextPartsOf(graph, id);
      return [id, {
        messageKeys: parts.filter((part) => part.defaultSelected !== false).map((part) => part.key),
        attachments: [...new Set(parts.flatMap((part) => part.message.attachments))],
      }];
    })),
  );
  const [preview, setPreview] = useState<{ totalRedacted: number; broadRedacted: number; nodes: { nodeId: string; redacted: number; isolation: string }[] }>({ totalRedacted: 0, broadRedacted: 0, nodes: [] });
  const [previewError, setPreviewError] = useState('');
  const [statuses, setStatuses] = useState<Record<string, string>>({});
  const [results, setResults] = useState<DispatchNodeResult[]>([]);
  const [runError, setRunError] = useState('');
  const [restartNodes, setRestartNodes] = useState<Set<string>>(new Set());
  const [settings] = useState(loadDispatchSettings);
  const [collapsed, setCollapsed] = useState(false);
  const [currentNodes, setCurrentNodes] = useState<DispatchStageRequest['nodes']>([]);
  const [userEdited, setUserEdited] = useState<Set<string>>(new Set());

  const graphWithTargets = useMemo(() => {
    const nodes = { ...graph.nodes };
    for (const id of nodeIds) {
      if (nodes[id] && targets[id]) nodes[id] = { ...nodes[id], target: { ...targets[id] } };
    }
    return { ...graph, nodes };
  }, [graph, nodeIds, targets]);
  const hasMissingTargets = nodeIds.some((id) => {
    const target = graphWithTargets.nodes[id]?.target;
    return !target?.agent.trim() || !target.model.trim();
  });

  const buildRequest = (sourceGraph: Graph, ids: string[]): DispatchStageRequest => ({
    runId,
    stage: ids.length > 0 ? Math.min(...ids.map((id) => graphStage(sourceGraph, id))) : 1,
    projectDir,
    settings: {
      timeoutSecs: settings.timeoutSecs,
      concurrency: settings.concurrency,
      maxRequests: settings.maxRequests,
      redact: settings.redact,
      writeProjectDispatch: settings.writeProjectDispatch,
      ccSwitchPath: settings.ccSwitchPath,
      validationCommand: settings.validationCommand,
    },
    nodes: ids.map((id) => {
      const node = sourceGraph.nodes[id];
      const target = node.target!;
      const selected = serializeContext(sourceGraph, id, selection[id]);
      const model = models.find((item) =>
        item.agent === target.agent && item.id === target.model &&
        (item.provider ?? '') === (target.provider ?? ''));
      return {
        nodeId: id,
        stage: graphStage(sourceGraph, id),
        agent: target.agent,
        provider: target.provider ?? '',
        model: target.model,
        vendor: model?.vendor ?? '',
        billingSource: model?.billing_source ?? '',
        role: target.role,
        tier: target.tier ?? model?.tier ?? '',
        thinking: target.thinking,
        visibility: target.visibility,
        prompt: promptForRole(target.role, selected.prompt),
        currentMessage: serializeCurrentMessage(node.user),
        materials: selected.materials,
        sessionId: restartNodes.has(id) ? '' : continuationSession(sourceGraph, id) ?? '',
        write: target.write,
        originalRequest: originalRequestOf(sourceGraph),
        upstreamArtifacts: upstreamArtifactsOf(sourceGraph, id),
      };
    }),
  });

  const request = useMemo<DispatchStageRequest>(
    () => buildRequest(graphWithTargets, nodeIds),
    // buildRequest only closes over the listed immutable render values.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [graphWithTargets, models, nodeIds, projectDir, restartNodes, runId, selection, settings],
  );

  useEffect(() => {
    if (phase !== 'confirm' || request.nodes.length === 0 || hasMissingTargets) return;
    let cancelled = false;
    commands.previewDispatch(request)
      .then((value) => { if (!cancelled) { setPreview(value); setPreviewError(''); } })
      .catch((error) => { if (!cancelled) setPreviewError(String(error)); });
    return () => { cancelled = true; };
  }, [hasMissingTargets, phase, request]);

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
    if (request.nodes.length === 0 || hasMissingTargets) return;
    setPhase('running');
    setRunError('');
    let working = graphWithTargets;
    const collected: DispatchNodeResult[] = [];
    let activeNodes: DispatchStageRequest['nodes'] = [];
    let validationRetries: Record<string, number> = {};
    try {
      while (true) {
        const ids = dispatchableNodeIds(working);
        if (ids.length === 0) break;
        if (!withinRequestLimit(collected.length, ids.length, settings.maxRequests)) {
          setRunError(`已达总请求上限 ${settings.maxRequests}，继续派发需重新确认。`);
          break;
        }
        const stageRequest = buildRequest(working, ids);
        const stagePreview = await commands.previewDispatch(stageRequest);
        setPreview(stagePreview);
        setPreviewError('');
        activeNodes = stageRequest.nodes;
        setCurrentNodes(stageRequest.nodes);
        setStatuses((current) => ({ ...current, ...Object.fromEntries(ids.map((id) => [id, '排队'])) }));
        const pending = markPending(working, ids);
        working = onCommit(pending);
        const output = await commands.dispatchGraphStage(stageRequest);
        collected.push(...output.results);
        setResults([...collected]);
        working = onCommit(applyDispatchResults(working, output.results));
        const step = reducePipelineStep(working, output.results, validationRetries);
        validationRetries = step.validationRetries;
        if (step.graph !== working) working = onCommit(step.graph);
        if (!step.continueRunning) break;
      }
      setPhase('results');
    } catch (error) {
      const message = String(error);
      setRunError(message);
      const failed = activeNodes.map((node) => ({
        nodeId: node.nodeId, stage: graphStage(working, node.nodeId), status: 'error', text: '', rawStdout: '', rawStderr: '', modelReported: '', sessionId: '',
        tokensIn: 0, tokensOut: 0, cacheRead: 0, durationMs: 0, exitCode: null, timedOut: false, truncated: false,
        redacted: 0, isolation: '部分', workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', error: message,
        artifacts: null, validationOutput: '',
      }));
      if (failed.length > 0) working = onCommit(applyDispatchResults(working, failed));
      setResults([...collected, ...failed]);
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
      messageKeys: parts.filter((part) => part.defaultSelected !== false).map((part) => part.key),
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

  const plannedRoles = new Set(initialIds.map((id) => graph.nodes[id]?.target?.role));
  const economy = initialIds.some((id) => presetNameOf(graph.id, id) === '经济模式')
    || (plannedRoles.has('出方案') && plannedRoles.has('执行') && plannedRoles.has('验收'));
  const smallTask = economy && isSmallEconomyTask(originalRequestOf(graph), graph.nodes[graph.root]?.user.attachments ?? []);
  const resultStages = groupResultsByStage(graph, results);
  const vendorNames = [...new Set(request.nodes
    .map((node) => (node.vendor || node.agent).trim())
    .filter(Boolean))];
  const usageGroups = [...new Set(results.map((result) => result.stage))]
    .sort((a, b) => a - b)
    .map((stage) => {
      const rows = results.filter((result) => result.stage === stage);
      return {
        stage,
        rows,
        tokensIn: rows.reduce((sum, result) => sum + result.tokensIn, 0),
        tokensOut: rows.reduce((sum, result) => sum + result.tokensOut, 0),
        cacheRead: rows.reduce((sum, result) => sum + result.cacheRead, 0),
        durationMs: rows.reduce((sum, result) => sum + result.durationMs, 0),
      };
    });

  if (phase === 'running' && collapsed) {
    return <button className="dispatch-collapsed" onClick={() => setCollapsed(false)}>展开派发</button>;
  }

  return (
    <div className="dispatch-overlay" onMouseDown={phase === 'running' ? undefined : onClose}>
      <div className="dispatch-modal" onMouseDown={(event) => event.stopPropagation()}>
        <header className="dispatch-head">
          <div>
            <div className="dispatch-title">{phase === 'confirm' ? '确认派发' : phase === 'running' ? '执行中' : '流水线结果'}</div>
            <div className="dispatch-sub">run {runId}</div>
          </div>
          {phase === 'running'
            ? <button className="dispatch-close" onClick={() => setCollapsed(true)}>收起</button>
            : <button className="dispatch-close" onClick={onClose}>关闭</button>}
        </header>

        {phase === 'confirm' && (
          <div className="dispatch-body">
            {smallTask && <div className="dispatch-warning">单步小任务，外派固定足迹不划算，建议直接 @ 一个执行者</div>}
            {nodeIds.length === 0 && <div className="dispatch-empty">当前没有父节点已完成的待派节点。</div>}
            {nodeIds.map((id) => {
              const node = graphWithTargets.nodes[id];
              const target = targets[id];
              const blind = target.visibility === '盲';
              const missingTarget = !target.agent.trim() || !target.model.trim();
              const isolation = preview.nodes.find((item) => item.nodeId === id)?.isolation ?? '核对中';
              const parts = contextPartsOf(graphWithTargets, id);
              const canRestart = !!continuationSession(graphWithTargets, id) && !restartNodes.has(id);
              const vendor = models.find((item) => item.agent === target.agent && item.id === target.model && (item.provider ?? '') === (target.provider ?? ''))?.vendor;
              const sameVendorReview = target.role === '验收' && !!vendor && initialIds.some((otherId) => {
                const other = targets[otherId];
                if (!other || other.role !== '执行') return false;
                return models.find((item) => item.agent === other.agent && item.id === other.model && (item.provider ?? '') === (other.provider ?? ''))?.vendor === vendor;
              });
              return (
                <section key={id} className="dispatch-route-card">
                  <div className="dispatch-route-line">{routeLine(node, models, isolation)}</div>
                  <div className="dispatch-reason">理由：{userEdited.has(id) ? '用户指定' : '角色默认'}</div>
                  {missingTarget && <div className="dispatch-warning">未指定模型，不能确认派发。</div>}
                  {target.role === '执行' && ['关', '低'].includes(graph.nodes[id].target?.thinking ?? '') && target.thinking === '中' && (
                    <div className="dispatch-warning">任务含数字或链式关键词，thinking 已调为中；可修改。</div>
                  )}
                  {sameVendorReview && <div className="dispatch-warning">复核，非独立验证</div>}
                  <div className="dispatch-controls">
                    {models.length > 0 ? <label>目标<select
                      value={JSON.stringify([target.agent, target.provider ?? '', target.model])}
                      onChange={(event) => {
                        const [agent, provider, modelId] = JSON.parse(event.target.value) as string[];
                        const hit = models.find((item) => item.agent === agent && item.provider === provider && item.id === modelId);
                        setUserEdited((current) => new Set(current).add(id));
                        setTargets((current) => ({
                          ...current,
                          [id]: { ...target, agent, provider, model: modelId, tier: hit?.tier || target.tier },
                        }));
                      }}
                    >
                      {!models.some((item) => item.agent === target.agent && item.provider === (target.provider ?? '') && item.id === target.model) && (
                        <option value={JSON.stringify([target.agent, target.provider ?? '', target.model])}>
                          {target.agent && target.model ? `${target.agent}/${target.provider ? `${target.provider}/` : ''}${target.model}` : '未指定'}
                        </option>
                      )}
                      {models.map((item) => (
                        <option key={JSON.stringify([item.agent, item.provider, item.id])} value={JSON.stringify([item.agent, item.provider, item.id])}>
                          {item.agent}/{item.provider ? `${item.provider}/` : ''}{item.id}
                        </option>
                      ))}
                    </select></label> : <label className="dispatch-manual-target">目标
                      <input aria-label="agent" value={target.agent} placeholder="agent" onChange={(event) => { setUserEdited((current) => new Set(current).add(id)); setTargets((current) => ({ ...current, [id]: { ...target, agent: event.target.value } })); }} />
                      <input aria-label="model" value={target.model} placeholder="model" onChange={(event) => { setUserEdited((current) => new Set(current).add(id)); setTargets((current) => ({ ...current, [id]: { ...target, model: event.target.value } })); }} />
                    </label>}
                    <label>档位<input value={target.tier ?? ''} onChange={(event) => { setUserEdited((current) => new Set(current).add(id)); setTargets((current) => ({ ...current, [id]: { ...target, tier: event.target.value } })); }} /></label>
                    <label>thinking<select value={target.thinking} onChange={(event) => { setUserEdited((current) => new Set(current).add(id)); setTargets((current) => ({ ...current, [id]: { ...target, thinking: event.target.value as Thinking } })); }}>
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
                          {part.message.text !== '' && <label><input type="checkbox" checked={selection[id]?.messageKeys.includes(part.key) ?? false} onChange={() => toggleMessage(id, part.key)} />{part.defaultSelected === false ? '可选上游 · ' : ''}{part.label}：{part.message.text.slice(0, 100)}</label>}
                          {part.message.attachments.map((path) => <label key={path}><input type="checkbox" checked={selection[id]?.attachments.includes(path) ?? false} onChange={() => toggleAttachment(id, path)} />附件：{path}</label>)}
                        </div>
                      ))}
                    </div>
                  )}
                </section>
              );
            })}
            <div className="dispatch-authorization">
              材料将发送至 {vendorNames.join('、') || '所选厂商'}
              {preview.totalRedacted > 0 ? `；已脱敏 ${preview.totalRedacted} 处` : ''}
            </div>
            {preview.broadRedacted > 0 && <div className="dispatch-warning">可能连带替换正常内容，{preview.broadRedacted} 处</div>}
            {previewError && <div className="dispatch-error">{previewError}</div>}
            <div className="dispatch-actions">
              <button onClick={onClose}>取消</button>
              <button className="dispatch-primary" disabled={nodeIds.length === 0 || hasMissingTargets || !!previewError} onClick={start}>确认派发</button>
            </div>
          </div>
        )}

        {phase === 'running' && (
          <div className="dispatch-body">
            {currentNodes.map((node) => (
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
            <div className="dispatch-result-grid" style={{ gridTemplateColumns: `repeat(${Math.max(1, resultStages.length)}, minmax(260px, 1fr))` }}>
              {resultStages.map((group) => (
                <div key={group.stage} className="dispatch-stage-column">
                  <div className="dispatch-stage-heading">{group.title}</div>
              {group.rows.map((result) => {
                const node = graph.nodes[result.nodeId];
                const adopted = node?.reply?.adopted === true;
                return (
                <section key={result.nodeId} className={`dispatch-result-card${adopted ? ' adopted' : ''}`}>
                  <div className="dispatch-result-head">
                    <span>{node?.target?.agent}/{node?.target?.model}{node?.target?.agent === 'cc-switch' ? ' · 单轮' : ''}</span>
                    <span>{resultStatus(result.status)}</span>
                  </div>
                  {result.truncated && <div className="dispatch-warning">输出已截断</div>}
                  {result.tokensIn > 50_000 && <div className="dispatch-warning">输入超过 50k tokens</div>}
                  <div className="dispatch-result-text">{result.text || result.error}</div>
                  {result.artifacts && (
                    <details className="dispatch-artifacts">
                      <summary>产物 · {result.artifacts.changed_files.length} 个文件</summary>
                      {result.artifacts.worktree && <div className="dispatch-worktree">采纳前请手动合并：{result.artifacts.worktree}</div>}
                      {!result.artifacts.worktree && <div className="dispatch-worktree">单写者产物已在项目目录，标记采纳即可。</div>}
                      <pre>{result.artifacts.diff || '无文件改动'}</pre>
                    </details>
                  )}
                  {result.validationOutput && <details><summary>验证结果</summary><pre>{result.validationOutput}</pre></details>}
                  <div className="dispatch-card-actions">
                    <button onClick={() => onCommit(setReplyAdopted(graph, result.nodeId, !adopted))}>{adopted ? '取消采纳' : '采纳'}</button>
                    <button onClick={() => retry(result.nodeId, false)}>重派</button>
                    <button onClick={() => retry(result.nodeId, true)}>升级</button>
                  </div>
                </section>
              );})}
                </div>
              ))}
            </div>
            <table className="dispatch-usage"><thead><tr><th>阶段</th><th>agent/模型</th><th>in / out tokens</th><th>cache_read</th><th>耗时</th><th>结果</th></tr></thead>
              <tbody>{usageGroups.map((group) => <Fragment key={group.stage}>
                {group.rows.map((result) => { const target = graph.nodes[result.nodeId]?.target; return <tr key={result.nodeId}><td>{result.stage}</td><td>{target?.agent}/{target?.model}{target?.agent === 'cc-switch' ? ' · 单轮' : ''}</td><td>{result.tokensIn || '—'} / {result.tokensOut || '—'}</td><td>{result.cacheRead || '—'}</td><td>{result.durationMs} ms</td><td>{resultStatus(result.status)}</td></tr>; })}
                <tr className="dispatch-usage-subtotal"><td>{group.stage} 小计</td><td>{group.rows.length} 路</td><td>{group.tokensIn} / {group.tokensOut}</td><td>{group.cacheRead}</td><td>{group.durationMs} ms</td><td>—</td></tr>
              </Fragment>)}</tbody>
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
