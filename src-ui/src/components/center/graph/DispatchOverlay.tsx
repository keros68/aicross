import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import { commands, isTauri, type DispatchNodeResult, type DispatchStageRequest, type GitChanges, type RoleSettings, type SwarmMergeResult } from '../../../tauri';
import { contextPartsOf, originalRequestOf, serializeContext, serializeCurrentMessage, upstreamArtifactsOf } from '../../../graph/context';
import {
  appendArbitration,
  appendDecision,
  buildNodeRequest,
  cacheReadOf,
  clearTargetSession,
  continuationSession,
  continuationRunner,
  dispatchableNodeIds,
  dirtyWorktreeHintOf,
  initialDispatchSelection,
  progressStatusLabel,
  rollbackBeforeOf,
  runningDetailOf,
  startPipelineHeadless,
  toggleRollbackOnReview,
  graphStage,
  groupResultsByStage,
  leftoverNodes,
  isFixRoundExecution,
  isSmallEconomyTask,
  loggedOutNodeIds,
  needsChainThinking,
  neutralDisputeSummary,
  pickOne,
  quotaBlockedNodeIds,
  failedNodesWithoutResults,
  redraftNode,
  routeLine,
  routeReason,
  sameParentsResultCount,
  setReplyAdopted,
} from '../../../graph/dispatch';
import type { DispatchProgressStore } from '../../../graph/dispatch-progress';
import { updateNodeFallback, updateNodeSwarm } from '../../../graph/mutate';
import {
  appendSwarmNodes,
  applySwarmMerge,
  arbitrationCandidates,
  normalizeSwarmMergeResults,
  parseSwarmPlan,
  swarmCardActions,
  swarmCheckInput,
  swarmCheckOf,
  swarmGenerateNotice,
  swarmMergeEntry,
  swarmMergeErrorResults,
  swarmMergeSummary,
  swarmPlanNotice,
  SWARM_PARSE_FAILED_TEXT,
  swarmCheckItems,
  swarmGate,
  swarmMergeItems,
  swarmNodeIdsOf,
  swarmOverlapItems,
  swarmPendingMergeStage,
  swarmStageOf,
  swarmWireSettings,
  type SwarmCheckState,
} from '../../../graph/swarm';
import { FallbackSkippedLine, IdleHint, PickOneButton, PinnedNote, PinnedStatus, QuotaHint, QuotaResetLine, ReviewPendingRow, ReviewPendingTag, RollbackBeforeHint, SwarmGateNotice, SwarmMergeRow, VerificationHint, WritableFallbackHint } from './DispatchBits';
import { SwarmEditor } from '../../right/orchestration/NodePanel';
import { presetNameOf } from '../../../graph/persist';
import { loadDispatchSettings } from '../../../graph/dispatch-settings';
import type { DispatchableModel, Graph, Target, Thinking } from '../../../graph/types';
import { findModel } from '../../../graph/validate';
import { modelStatusSuffix } from '../../../resources/model-picker';
import { agentModelDisplay } from '../../../resources/agent-display';
import { loginOf, quotaExhaustedLabel, verificationOf } from '../../../resources/types';

function displayRouteLine(node: Graph['nodes'][string], models: DispatchableModel[], isolation: string): string {
  let line = routeLine(node, models, isolation);
  for (const target of [node.target, node.target?.fallback]) {
    if (!target?.agent || !target.model) continue;
    const machine = `${target.agent}/${target.model}`;
    line = line.split(machine).join(agentModelDisplay(target.agent, target.model));
  }
  return line;
}

type Phase = 'confirm' | 'running' | 'results';

interface Props {
  graph: Graph;
  models: DispatchableModel[];
  projectDir: string;
  // 崩溃后对账判「未知」的节点（TASK-06-1）：确认屏标「待核对」，默认不勾选派发。
  needsReview?: string[];
  // 节点卡上按过「重派」的节点（TASK-06-2）：单写者的那些请求带 rollbackBefore。
  retryNodes?: string[];
  // 进度状态在 GraphTab 层（TASK-07C）：运行屏与图上的节点卡读同一份快照。
  progressStore: DispatchProgressStore;
  onCommit: (graph: Graph) => Graph;
  onClose: () => void;
  // 擂台「选它」与仲裁建节点后，把焦点/主列对齐到该节点（GraphTab 提供）
  onFocusNode?: (nodeId: string) => void;
  // 从图工具条「合并」进来（08E）：直接开结果屏，只做没并完的蜂群件的合并。
  mergeOnly?: boolean;
}

export const resultStatus = (status: string) =>
  status === 'ok' ? '完成' : status === 'quota_exhausted' ? '额度用尽' : status === 'timeout' ? '超时' : status === 'identity_mismatch' ? '真身不符' : status === 'validation_failed' ? '验证命令未通过' : '失败';

export function convergeQuotaSelection(
  initialIds: string[],
  current: string[],
  blocked: string[],
  touched: ReadonlySet<string>,
): string[] {
  return convergeBlockedSelection(initialIds, current, blocked, touched);
}

export function convergeBlockedSelection(
  initialIds: string[],
  current: string[],
  blocked: string[],
  touched: ReadonlySet<string>,
): string[] {
  const selected = new Set(current);
  const protectedIds = new Set(blocked);
  return initialIds.filter((id) => selected.has(id) && (touched.has(id) || !protectedIds.has(id)));
}

/** G1 初始选择：待核对与登录/额度受限的目标均默认不勾。 */
export function initialBlockedSelection(
  initialIds: string[],
  needsReview: string[] | undefined,
  blocked: string[],
): string[] {
  const protectedIds = new Set(blocked);
  return initialDispatchSelection(initialIds, needsReview).filter((id) => !protectedIds.has(id));
}

export function blockedNodePartitions(
  graph: Graph,
  initialIds: string[],
  models: DispatchableModel[],
): { quotaOnlyIds: string[]; loggedOutIds: string[] } {
  const loggedOutIds = loggedOutNodeIds(graph, initialIds, models);
  const loggedOut = new Set(loggedOutIds);
  const quotaOnlyIds = quotaBlockedNodeIds(graph, initialIds, models).filter((id) => !loggedOut.has(id));
  return { quotaOnlyIds, loggedOutIds };
}

export function initialResourceBlockedSelection(
  graph: Graph,
  initialIds: string[],
  needsReview: string[] | undefined,
  models: DispatchableModel[],
): string[] {
  const { quotaOnlyIds, loggedOutIds } = blockedNodePartitions(graph, initialIds, models);
  return initialBlockedSelection(initialIds, needsReview, [...quotaOnlyIds, ...loggedOutIds]);
}

export function DispatchOverlay({ graph, models, projectDir, needsReview, retryNodes, progressStore, onCommit, onClose, onFocusNode, mergeOnly }: Props) {
  const initialIds = useMemo(() => dispatchableNodeIds(graph), [graph]);
  const reviewIds = useMemo(() => new Set(needsReview ?? []), [needsReview]);
  const { quotaOnlyIds, loggedOutIds } = useMemo(() => blockedNodePartitions(graph, initialIds, models), [graph, initialIds, models]);
  const blockedIds = useMemo(() => [...quotaOnlyIds, ...loggedOutIds], [quotaOnlyIds, loggedOutIds]);
  const [nodeIds, setNodeIds] = useState(() => initialResourceBlockedSelection(graph, initialIds, needsReview, models));
  // 原名沿用额度分区；未登录目标也使用同一套「用户已手动勾选」收敛规则。
  const quotaSelectionTouched = useRef(new Set<string>());
  const [phase, setPhase] = useState<Phase>(mergeOnly ? 'results' : 'confirm');
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
  // G2 疑似空转（TASK-07-1）：快照与时钟在 GraphTab 层的共享 store 里
  // （TASK-07C），本屏只消费。状态词也从同一份快照读，心跳还没到的行显示「排队」。
  const [results, setResults] = useState<DispatchNodeResult[]>([]);
  const [runLeftoverIds, setRunLeftoverIds] = useState<string[]>([]);
  const [runError, setRunError] = useState('');
  const [restartNodes, setRestartNodes] = useState<Set<string>>(new Set());
  const [settings] = useState(loadDispatchSettings);
  const [collapsed, setCollapsed] = useState(false);
  const [currentNodes, setCurrentNodes] = useState<DispatchStageRequest['nodes']>([]);
  const [userEdited, setUserEdited] = useState<Set<string>>(new Set());
  // 单写者重派：请求带 rollbackBefore，引擎先回滚上次改动再派（TASK-06-2）。
  const [rollbackNodes, setRollbackNodes] = useState<Set<string>>(() => new Set(
    (retryNodes ?? []).filter((id) => rollbackBeforeOf(graph.nodes[id]?.target, true)),
  ));
  // 项目目录未提交改动提示：用现有只读命令 gitChanges，读不出来就不显示。
  const [gitChanges, setGitChanges] = useState<GitChanges | null>(null);
  // 仲裁编排（按结果阶段行）：双方选择 + 中性描述（可改）+ 第三家目标
  const [arbRows, setArbRows] = useState<Record<number, { aId: string; bId: string; text: string; targetKey: string; error: string }>>({});
  // 蜂群（TASK-08）：G1 的只读清单核对结果，G3 的逐件合并结果。
  // 核对分态（R1-1）：在核对 / 回来了 / 核对不了，窗口期内不放行。初值是
  // pending 而不是 idle（08C-4）：确认屏第一帧 effect 还没跑，idle 会放行一帧。
  const [swarmCheck, setSwarmCheck] = useState<SwarmCheckState>({ phase: 'pending' });
  const [swarmProceed, setSwarmProceed] = useState(false);
  const [mergeResults, setMergeResults] = useState<SwarmMergeResult[] | null>(null);
  const [merging, setMerging] = useState(false);
  // 「按清单生成执行节点」的反馈行（解析失败 / 截断 / 已生成几个），按拆分节点记。
  const [planNotice, setPlanNotice] = useState<Record<string, string>>({});
  // 生成的执行节点要带「执行」角色默认目标，角色配置在引擎侧（同 OrchestrationPanel）。
  const [roles, setRoles] = useState<RoleSettings | null>(null);
  useEffect(() => {
    setNodeIds((current) => convergeQuotaSelection(initialIds, current, blockedIds, quotaSelectionTouched.current));
  }, [initialIds, blockedIds]);
  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    commands.loadRoles().then((value) => { if (!cancelled) setRoles(value); }).catch(() => {});
    return () => { cancelled = true; };
  }, []);

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
  const exceedsRequestLimit = nodeIds.length > settings.maxRequests;

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
      acpEnabled: settings.acpEnabled,
      fixRoundLimit: settings.fixRoundLimit,
    },
    // 节点装配在 graph/dispatch.ts 的 buildNodeRequest（R1-5：请求体形状可测）
    nodes: ids.map((id) => {
      const node = sourceGraph.nodes[id];
      const selected = serializeContext(sourceGraph, id, selection[id]);
      return buildNodeRequest(id, node.target!, {
        models,
        prompt: selected.prompt,
        materials: selected.materials,
        currentMessage: serializeCurrentMessage(node.user),
        originalRequest: originalRequestOf(sourceGraph),
        upstreamArtifacts: upstreamArtifactsOf(sourceGraph, id),
        sessionId: restartNodes.has(id) ? '' : continuationSession(sourceGraph, id) ?? '',
        previousRunner: restartNodes.has(id) ? '' : continuationRunner(sourceGraph, id) ?? '',
        rollback: rollbackNodes.has(id),
        stage: graphStage(sourceGraph, id),
        fixRound: isFixRoundExecution(sourceGraph, id),
      });
    }),
  });

  const request = useMemo<DispatchStageRequest>(
    () => buildRequest(graphWithTargets, nodeIds),
    // buildRequest only closes over the listed immutable render values.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [graphWithTargets, models, nodeIds, projectDir, restartNodes, rollbackNodes, runId, selection, settings],
  );

  // G1 未提交改动提示（TASK-06-2）：只读，走既有的 gitChanges 命令；
  // 不是仓库 / 命令不可用时不显示。R1-13：只在本批含 worktree 节点时才出这句，
  // 所以读回来的原始状态进 state，提示文案在渲染期按本批节点算。
  useEffect(() => {
    if (phase !== 'confirm' || projectDir === '') return;
    let cancelled = false;
    commands.gitChanges(projectDir)
      .then((changes) => { if (!cancelled) setGitChanges(changes); })
      .catch(() => { if (!cancelled) setGitChanges(null); });
    return () => { cancelled = true; };
  }, [phase, projectDir]);

  const hasWorktreeNode = nodeIds.some((id) => (targets[id] ?? graph.nodes[id]?.target)?.write === 'worktree');
  const dirtyHint = dirtyWorktreeHintOf(gitChanges, hasWorktreeNode);

  // G1 蜂群清单核对（TASK-08 B-4）：本批含 swarm 节点才调，只读。核对回来之前
  // 按 pending 拦住「确认派发」，核对不了时说「无法核对」并让用户自己决定要不要
  // 继续（R1-1）。改了 files 之后 graph 变化会重跑这个 effect，重叠消失即解禁。
  const swarmIds = useMemo(() => swarmNodeIdsOf(graph, nodeIds), [graph, nodeIds]);
  useEffect(() => {
    if (phase !== 'confirm' || swarmIds.length === 0 || projectDir === '') return;
    let cancelled = false;
    setSwarmCheck({ phase: 'pending' });
    setSwarmProceed(false);
    commands.checkSwarmPlan(projectDir, swarmCheckItems(graph, swarmIds), swarmWireSettings(settings))
      .then((value) => { if (!cancelled) setSwarmCheck({ phase: 'ok', check: value }); })
      .catch((error) => { if (!cancelled) setSwarmCheck({ phase: 'error', message: String(error) }); });
    return () => { cancelled = true; };
  }, [graph, phase, projectDir, settings, swarmIds]);
  // 闸门读的状态由「有几件 + 有没有工作目录 + 命令状态」推出来（08C-4）：
  // 没有蜂群件不拦；没选工作目录时命令发不出去，直接阻断并说清原因。
  const swarmCheckState = swarmCheckInput(swarmIds.length, projectDir, swarmCheck);
  const swarmGateState = swarmGate(swarmCheckState, swarmProceed);
  const overlapItems = new Set(swarmOverlapItems(swarmCheckOf(swarmCheckState)));

  useEffect(() => {
    if (phase !== 'confirm' || request.nodes.length === 0 || hasMissingTargets) return;
    let cancelled = false;
    commands.previewDispatch(request)
      .then((value) => { if (!cancelled) { setPreview(value); setPreviewError(''); } })
      .catch((error) => { if (!cancelled) setPreviewError(String(error)); });
    return () => { cancelled = true; };
  }, [hasMissingTargets, phase, request]);

  // 待核对节点：对账判 unknown 且这一屏本来就可派的那些。勾选后加回派发队列，
  // 顺序按 initialIds（与路由卡顺序一致）。
  const reviewNodeIds = initialIds.filter((id) => reviewIds.has(id));
  const toggleReviewNode = (id: string) => {
    const checked = !nodeIds.includes(id);
    setNodeIds((current) => {
      const next = new Set(current);
      if (checked) next.add(id);
      else next.delete(id);
      return initialIds.filter((item) => next.has(item));
    });
    // R1-8：勾回来的单写者节点也要先回滚——上次可能已经改过文件。
    setRollbackNodes((current) =>
      toggleRollbackOnReview(current, targets[id] ?? graph.nodes[id]?.target, id, checked));
  };
  const toggleBlockedNode = (id: string) => {
    quotaSelectionTouched.current.add(id);
    setNodeIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return initialIds.filter((item) => next.has(item));
    });
  };

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
    setRunLeftoverIds([]);
    // 新一轮先清进度（R1-2）：快照以 nodeId 为键、跨轮存活，上一轮没收到终态
    // 的快照（叫停、抛错、丢事件）会让新一行一出现就显示上一轮的耗时、甚至
    // 立刻挂空转胶囊。beginRun 同时把共享层的 runId 换成本轮的。
    setCurrentNodes([]);
    // R1-7：确认屏取消勾选的「待核对」节点不进实际派发集合——这一层不派，
    // 后续层重算时也不再捡起来。
    const skipIds = new Set([...reviewIds].filter((id) => !nodeIds.includes(id)));
    const pipeline = await startPipelineHeadless({
      graph: graphWithTargets,
      settings,
      skip: skipIds,
      begin: (id) => progressStore.beginRun(id),
      end: () => progressStore.endRun(),
      deps: {
        runId,
        buildRequest,
        preview: (stageRequest: DispatchStageRequest) => commands.previewDispatch(stageRequest),
        dispatch: (stageRequest: DispatchStageRequest) => commands.dispatchGraphStage(stageRequest),
        commit: onCommit,
        onPreview: (stagePreview) => { setPreview(stagePreview); setPreviewError(''); },
        onStage: (stageRequest) => setCurrentNodes(stageRequest.nodes),
        onResults: (collected) => setResults([...collected]),
      },
      onError: (active, collected, message, working) => failedNodesWithoutResults(active, collected, message)
        .map((node) => ({ ...node, stage: graphStage(working, node.nodeId), rawStdout: '', rawStderr: '', cacheRead: 0, exitCode: null, timedOut: false, truncated: false, redacted: 0, isolation: '部分', runner: 'unknown' as const, workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', validationOutput: '', artifacts: null, fallbackFrom: '', fallbackTo: '', fallbackSkipped: '' })),
    });
    if (pipeline.outcome.kind === 'missing-target') {
      const missingIds = pipeline.outcome.nodeIds;
      setNodeIds(missingIds);
      setTargets((current) => ({ ...current, ...Object.fromEntries(missingIds.map((id) => [id, { ...pipeline.graph.nodes[id].target! }])) }));
      setRunError(pipeline.outcome.error);
      setPhase('confirm');
      return;
    }
    if (pipeline.error) setRunError(pipeline.error);
    setRunLeftoverIds(pipeline.leftoverIds);
    setResults([...pipeline.results]);
    setPhase('results');
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
    const nextRunId = crypto.randomUUID();
    setRunId(nextRunId);
    setRestartNodes(new Set());
    // 重派复用同一个 nodeId：上一轮的进度快照不能留到这一轮（R1-2）。
    progressStore.beginRun(nextRunId);
    setCurrentNodes([]);
    // 单写者节点重派/升级：这一路带 rollbackBefore（TASK-06-2）。
    setRollbackNodes(rollbackBeforeOf(node.target, true) ? new Set([nodeId]) : new Set());
    setResults([]);
    setRunLeftoverIds([]);
    setPhase('confirm');
  };

  const handoff = () => {
    const summary = results.map((result) => {
      const node = graph.nodes[result.nodeId];
      const route = node?.target ? `${node.target.agent}/${node.target.model}` : result.nodeId;
      return `### ${route}\n${result.status === 'ok' ? result.text : result.status === 'quota_exhausted' ? `额度用尽：${result.error}` : `失败：${result.error}`}`;
    }).join('\n\n');
    onCommit(appendDecision(graph, results.map((result) => result.nodeId), `请裁决以下结果。\n\n${summary}`));
    onClose();
  };

  const startNewSession = (nodeId: string) => {
    onCommit(clearTargetSession(graphWithTargets, nodeId));
    setRestartNodes((current) => new Set(current).add(nodeId));
  };

  // ── 仲裁（TASK-05B）：同阶段 ≥2 路成功结果时，可选双方、改中性描述、
  // 选一个与两边都不同厂商的第三家，建盲仲裁节点（parents=共同材料祖先） ──
  const arbFor = (stage: number, okRows: DispatchNodeResult[]) => {
    const existing = arbRows[stage];
    if (existing) return existing;
    const aId = okRows[0]?.nodeId ?? '';
    const bId = okRows[1]?.nodeId ?? '';
    return {
      aId,
      bId,
      text: aId && bId ? neutralDisputeSummary(graph, aId, bId) : '',
      targetKey: '',
      error: '',
    };
  };
  const setArbRow = (stage: number, row: { aId: string; bId: string; text: string; targetKey: string; error: string }) =>
    setArbRows((current) => ({ ...current, [stage]: row }));
  const createArbitration = (stage: number, row: { aId: string; bId: string; text: string; targetKey: string; error: string }) => {
    if (!row.targetKey) {
      setArbRow(stage, { ...row, error: '请选择仲裁目标' });
      return;
    }
    const [agent, provider, modelId] = JSON.parse(row.targetKey) as string[];
    const target: Target = {
      agent,
      provider: provider || undefined,
      model: modelId,
      role: '仲裁',
      thinking: '中',
      visibility: '盲',
      write: '只读',
    };
    const outcome = appendArbitration(graph, row.aId, row.bId, row.text, target, models);
    if (outcome.error || !outcome.nodeId) {
      setArbRow(stage, { ...row, error: outcome.error ?? '建仲裁节点失败' });
      return;
    }
    onCommit(outcome.graph);
    onFocusNode?.(outcome.nodeId);
    onClose();
  };

  // ── 蜂群 G3（TASK-08 B-3/B-5）：按清单生成执行节点；全部件终态后逐件合并 ──
  // R1-2：再点一次不重复建同件号的节点，件数上限在生成这一步也生效。
  const generateSwarmNodes = (splitNodeId: string) => {
    const plan = parseSwarmPlan(graph.nodes[splitNodeId]?.reply?.text ?? '', settings.swarmLimit);
    if (!plan) {
      setPlanNotice((current) => ({ ...current, [splitNodeId]: SWARM_PARSE_FAILED_TEXT }));
      return;
    }
    const outcome = appendSwarmNodes(graph, splitNodeId, plan.items, roles?.execution, settings.swarmLimit);
    if (outcome.nodeIds.length > 0) onCommit(outcome.graph);
    const notice = [swarmPlanNotice(plan, settings.swarmLimit), swarmGenerateNotice(outcome, settings.swarmLimit)]
      .filter((line): line is string => !!line)
      .join('；');
    setPlanNotice((current) => ({ ...current, [splitNodeId]: notice }));
  };

  // 合并入口按图派生（08C-2）：本阶段图上的全部蜂群件，不是本次 results 里的那些。
  // 关屏重开、单件重派之后清单都按当前图算，冲突件不会因为换了一批结果就没地方再并。
  // 从工具条「合并」进来时阶段在开屏那一刻钉住（08E）：并完最后一件后清单不会消失，结果行留着给人看。
  const [pinnedSwarmStage] = useState(() => (mergeOnly ? swarmPendingMergeStage(graph) : null));
  const swarmStage = swarmStageOf(graph, [...results.map((result) => result.nodeId), ...initialIds]) ?? pinnedSwarmStage;
  const mergeEntry = swarmStage === null ? null : swarmMergeEntry(graph, swarmStage);
  const swarmReady = mergeEntry?.ready === true;
  // 已并入的件不再进下一次请求（R1-3）：没剩下要并的件时按钮禁用。
  const swarmPendingItems = swarmMergeItems(graph, mergeEntry?.nodeIds ?? []);
  const mergeResultOf = (nodeId: string) => (mergeResults ?? []).find((row) => row.nodeId === nodeId);
  const runSwarmMerge = async () => {
    if (merging || swarmPendingItems.length === 0) return;
    const items = swarmPendingItems;
    setMerging(true);
    try {
      const raw = await commands.mergeSwarmItems(projectDir, runId, items, settings.validationCommand, swarmWireSettings(settings));
      const merged = normalizeSwarmMergeResults(raw);
      setMergeResults(merged);
      if (merged.length > 0) onCommit(applySwarmMerge(graph, merged));
    } catch (error) {
      // 引擎没这条命令才说「未接通」；其它错误照原文说（R1-3）。
      setMergeResults(swarmMergeErrorResults(items, error));
    } finally {
      setMerging(false);
    }
  };

  const plannedRoles = new Set(initialIds.map((id) => graph.nodes[id]?.target?.role));
  const economy = initialIds.some((id) => presetNameOf(graph.id, id) === '经济模式')
    || (plannedRoles.has('出方案') && plannedRoles.has('执行') && plannedRoles.has('验收'));
  const smallTask = economy && isSmallEconomyTask(originalRequestOf(graph), graph.nodes[graph.root]?.user.attachments ?? []);
  const resultStages = groupResultsByStage(graph, results);
  const skippedNodes = Object.values(graph.nodes).filter((node) => !!node.reply?.skipped_reason);
  // 遗留节点（TASK-12）：本轮验收结果到修正轮上限后生成的那些，结果屏展示同一清单。
  const currentLeftoverIds = new Set(runLeftoverIds);
  const leftovers = leftoverNodes(graph).filter((node) => currentLeftoverIds.has(node.id));
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
        // TASK-07-3：cache_read 优先读图上的 tokens_cache_read（对账收回来的
        // 那一路只有图字段），小计与逐行同一个口径。
        cacheRead: rows.reduce((sum, result) => sum + cacheReadOf(graph.nodes[result.nodeId]?.reply, result), 0),
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
            {/* 常规提示走 dispatch-reason，红字只留给异常（TASK-07-5） */}
            {smallTask && <div className="dispatch-reason">单步小任务，建议直接 @ 一个执行者</div>}
            {nodeIds.length === 0 && <div className="dispatch-empty">当前没有父节点已完成的待派节点。</div>}
            {exceedsRequestLimit && <div className="dispatch-warning">本层有 {nodeIds.length} 路，超过总请求上限 {settings.maxRequests}，不能确认派发。</div>}
            {dirtyHint && <div className="dispatch-reason">{dirtyHint}</div>}
            <SwarmGateNotice gate={swarmGateState} proceed={swarmProceed} onProceed={setSwarmProceed} />
            {reviewNodeIds.length > 0 && (
              <section className="dispatch-route-card">
                <div className="dispatch-stage-heading">待核对</div>
                <div className="dispatch-reason">上次派发结果未知，默认不派。核对项目目录后可勾选加入本次派发。</div>
                {/* R1-10：未勾选的节点在下面没有路由卡，这里连路由行一起显示 */}
                {reviewNodeIds.map((id) => (
                  <ReviewPendingRow
                    key={id}
                    line={displayRouteLine(graphWithTargets.nodes[id], models, '核对中')}
                    checked={nodeIds.includes(id)}
                    onToggle={() => toggleReviewNode(id)}
                  />
                ))}
              </section>
            )}
            {quotaOnlyIds.length > 0 && (
              <section className="dispatch-route-card">
                <div className="dispatch-stage-heading">额度用尽</div>
                <div className="dispatch-reason">恢复时间前默认不派，可勾选后继续。</div>
                {quotaOnlyIds.map((id) => {
                  const target = graph.nodes[id]?.target;
                  const picked = target ? findModel(models, target) : undefined;
                  return (
                    <label className="dispatch-review-row" key={id}>
                      <input type="checkbox" checked={nodeIds.includes(id)} onChange={() => toggleBlockedNode(id)} />
                      <span className="dispatch-route-line">{displayRouteLine(graph.nodes[id], models, '核对中')}</span>
                      <span className="hint-capsule">{picked ? quotaExhaustedLabel(picked) : ''}</span>
                    </label>
                  );
                })}
              </section>
            )}
            {loggedOutIds.length > 0 && (
              <section className="dispatch-route-card">
                <div className="dispatch-stage-heading">未登录</div>
                <div className="dispatch-reason">该 provider 未登录，默认不派，可勾选后继续。</div>
                {loggedOutIds.map((id) => {
                  const target = graph.nodes[id]?.target;
                  const picked = target ? findModel(models, target) : undefined;
                  return (
                    <label className="dispatch-review-row" key={id}>
                      <input type="checkbox" checked={nodeIds.includes(id)} onChange={() => toggleBlockedNode(id)} />
                      <span className="dispatch-route-line">{displayRouteLine(graph.nodes[id], models, '核对中')}</span>
                      <span className="hint-capsule">{picked && loginOf(picked) === 'logged_out' ? `未登录${quotaExhaustedLabel(picked) ? '，额度同时受限' : ''}` : ''}</span>
                    </label>
                  );
                })}
              </section>
            )}
            {nodeIds.map((id) => {
              const node = graphWithTargets.nodes[id];
              const target = targets[id];
              const blind = target.visibility === '盲';
              const missingTarget = !target.agent.trim() || !target.model.trim();
              const isolation = preview.nodes.find((item) => item.nodeId === id)?.isolation ?? '核对中';
              const parts = contextPartsOf(graphWithTargets, id);
              const canRestart = !!continuationSession(graphWithTargets, id) && !restartNodes.has(id);
              const vendor = models.find((item) => item.agent === target.agent && item.id === target.model && (item.provider ?? '') === (target.provider ?? ''))?.vendor;
              // models 现在含未冒烟条目（TASK-08b），G1 决策行要能看出这一点。
              // provider 先精确匹配，匹配不上再退到 agent+id——节点面板选模型时不写 provider。
              const picked = models.find((item) => item.agent === target.agent && item.id === target.model && (item.provider ?? '') === (target.provider ?? ''))
                ?? models.find((item) => item.agent === target.agent && item.id === target.model);
              // 09-15 口径：未验证可以直接派，引擎在首次派发前自己验一次；
              // 验失败过的不自动重试，提示用户去接入页手动重验。
              const verification = picked && verificationOf({
                agent: picked.agent,
                status: picked.status,
                verification: picked.verification ?? '',
              });
              const sameVendorReview = target.role === '验收' && !!vendor && initialIds.some((otherId) => {
                const other = targets[otherId];
                if (!other || other.role !== '执行') return false;
                return models.find((item) => item.agent === other.agent && item.id === other.model && (item.provider ?? '') === (other.provider ?? ''))?.vendor === vendor;
              });
              return (
                <section key={id} className={`dispatch-route-card${node.swarm && overlapItems.has(node.swarm.item) ? ' swarm-overlap' : ''}`}>
                  <div className="dispatch-route-line">{displayRouteLine(node, models, isolation)}<ReviewPendingTag visible={reviewIds.has(id)} /></div>
                  {routeReason(node) && <div className="dispatch-reason">{routeReason(node)}</div>}
                  <RollbackBeforeHint target={target} retrying={rollbackNodes.has(id)} />
                  <div className="dispatch-reason">理由：{userEdited.has(id) ? '用户指定' : '角色默认'}</div>
                  {missingTarget && <div className="dispatch-warning">未指定模型，不能确认派发。</div>}
                  {target.role === '执行' && ['关', '低'].includes(graph.nodes[id].target?.thinking ?? '') && target.thinking === '中' && (
                    <div className="dispatch-warning">任务含数字或链式关键词，thinking 已调为中。</div>
                  )}
                  {sameVendorReview && <div className="dispatch-reason">复核，非独立验证</div>}
                  {/* 未验证是第三方模型的常态，走 dispatch-reason；红字只留给 failed（修正轮 R1） */}
                  <VerificationHint verification={verification} />
                  <QuotaHint model={picked} />
                  {/* R1-5：判出重叠就在这张卡上改文件范围，改完自动重核对，不用关屏回节点面板 */}
                  {node.swarm && overlapItems.has(node.swarm.item) && (
                    <SwarmEditor
                      swarm={node.swarm}
                      editable={node.reply?.status === 'draft'}
                      onChange={(next) => onCommit(updateNodeSwarm(graph, id, next))}
                    />
                  )}
                  <WritableFallbackHint target={target} models={models} />
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
                          {target.agent && target.model ? agentModelDisplay(target.agent, target.model, target.provider) : '未指定'}
                        </option>
                      )}
                      {models.map((item) => (
                        <option key={JSON.stringify([item.agent, item.provider, item.id])} value={JSON.stringify([item.agent, item.provider, item.id])}>
                          {agentModelDisplay(item.agent, item.id, item.provider)}{modelStatusSuffix(item)}{loginOf(item) === 'logged_out' ? '（未登录）' : ''}
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
                    {models.length > 0 && <label>备选<select
                      value={JSON.stringify(target.fallback ? [target.fallback.agent, target.fallback.provider ?? '', target.fallback.model] : ['', '', ''])}
                      onChange={(event) => {
                        const [agent, provider, modelId] = JSON.parse(event.target.value) as string[];
                        const fallback = modelId === '' ? undefined : { agent, provider, model: modelId };
                        setUserEdited((current) => new Set(current).add(id));
                        setTargets((current) => ({
                          ...current,
                          [id]: { ...target, fallback },
                        }));
                        // 即时写回图节点：不点「确认派发」直接关弹窗也不丢。
                        // R2-12：只写回备选字段，本地改过但未确认的主目标不落盘。
                        onCommit(updateNodeFallback(graph, id, fallback));
                      }}
                    >
                      <option value={JSON.stringify(['', '', ''])}>无</option>
                      {target.fallback && !models.some((item) => item.agent === target.fallback!.agent && item.provider === (target.fallback!.provider ?? '') && item.id === target.fallback!.model) && (
                        <option value={JSON.stringify([target.fallback.agent, target.fallback.provider ?? '', target.fallback.model])}>
                          {agentModelDisplay(target.fallback.agent, target.fallback.model)}
                        </option>
                      )}
                      {models.map((item) => (
                        <option key={JSON.stringify([item.agent, item.provider, item.id])} value={JSON.stringify([item.agent, item.provider, item.id])}>
                          {agentModelDisplay(item.agent, item.id, item.provider)}{modelStatusSuffix(item)}{loginOf(item) === 'logged_out' ? '（未登录）' : ''}
                        </option>
                      ))}
                    </select></label>}
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
              <button className="dispatch-primary" disabled={nodeIds.length === 0 || hasMissingTargets || exceedsRequestLimit || swarmGateState.blocked || !!previewError} onClick={start}>确认派发</button>
            </div>
          </div>
        )}

        {phase === 'running' && (
          <div className="dispatch-body">
            {currentNodes.map((node) => {
              // 只认本轮的快照（R1-2）：上一轮残留的读数与胶囊都不渲染。共享层
              // 的 snapshotOf 与节点卡是同一个入口（TASK-07C）。
              const snapshot = progressStore.snapshotOf(node.nodeId);
              const detail = runningDetailOf(snapshot, progressStore.now);
              return (
                <div key={node.nodeId} className="dispatch-running-row">
                  <div className="dispatch-running-main">
                    <span>{node.role || node.nodeId} · {agentModelDisplay(node.agent, node.model)}</span>
                    {detail !== '' && <span className="dispatch-running-detail">{detail}</span>}
                  </div>
                  <span>{snapshot ? progressStatusLabel(snapshot.status) : '排队'}</span>
                  <span className="dispatch-running-actions">
                    <IdleHint progress={snapshot} now={progressStore.now} />
                    <button onClick={() => commands.cancelDispatchNode(runId, node.nodeId).catch(() => false)}>叫停</button>
                  </span>
                </div>
              );
            })}
          </div>
        )}

        {phase === 'results' && (
          <div className="dispatch-body">
            {runError && <div className="dispatch-error">{runError}</div>}
            <div className="dispatch-result-lines">
              {results.map((result) => <div key={result.nodeId}>{displayRouteLine(graph.nodes[result.nodeId], models, result.isolation)}{routeReason(graph.nodes[result.nodeId]) && <div className="dispatch-reason">{routeReason(graph.nodes[result.nodeId])}</div>}</div>)}
              {skippedNodes.map((node) => <div key={node.id}>{node.target?.role || node.id} · 已跳过：{node.reply?.skipped_reason}</div>)}
            </div>
            <div className="dispatch-result-grid" style={{ gridTemplateColumns: `repeat(${Math.max(1, resultStages.length)}, minmax(260px, 1fr))` }}>
              {resultStages.map((group) => (
                <div key={group.stage} className="dispatch-stage-column">
                  <div className="dispatch-stage-heading">{group.title}</div>
              {group.rows.map((result) => {
                const node = graph.nodes[result.nodeId];
                const adopted = node?.reply?.adopted === true;
                // R1-4：蜂群件不是擂台对手，未并入不给「采纳」，已并入不给「重派 / 升级」。
                const cardActions = swarmCardActions(node, mergeResultOf(result.nodeId));
                return (
                <section key={result.nodeId} className={`dispatch-result-card${adopted ? ' adopted' : ''}`}>
                  <div className="dispatch-result-head">
                    <span>{node?.target ? agentModelDisplay(node.target.agent, node.target.model) : result.nodeId}{node?.target?.agent === 'cc-switch' ? ' · 单轮' : ''} · {result.runner === 'acp' ? '长驻' : result.runner === 'oneshot' ? '一次性' : '未运行'}</span>
                    <span className="dispatch-result-status">
                      <span>{resultStatus(result.status)}</span>
                      <PinnedStatus pinned={result.pinned} />
                    </span>
                  </div>
                  {result.truncated && <div className="dispatch-warning">输出已截断</div>}
                  {result.tokensIn > 50_000 && <div className="dispatch-warning">输入超过 50k tokens</div>}
                  {result.fallbackFrom && <div className="dispatch-reason">已切备选：{result.fallbackFrom} → {result.fallbackTo}</div>}
                  {result.runnerNote?.includes('额度用尽') && <div className="dispatch-reason">{result.runnerNote}</div>}
                  <QuotaResetLine resetsAt={result.quotaResetsAt} />
                  <FallbackSkippedLine result={result} />
                  <PinnedNote result={result} />
                  <div className="dispatch-result-text">{result.text || result.error}</div>
                  {result.artifacts && (
                    <details className="dispatch-artifacts">
                      <summary>产物 · {(result.artifacts.changed_files ?? []).length} 个文件</summary>
                      {result.artifacts.worktree && <div className="dispatch-worktree">采纳前请手动合并：{result.artifacts.worktree}</div>}
                      {!result.artifacts.worktree && <div className="dispatch-worktree">单写者产物已在项目目录，采纳即可。</div>}
                      <pre>{result.artifacts.diff || '无文件改动'}</pre>
                    </details>
                  )}
                  {result.validationOutput && <details><summary>验证结果</summary><pre>{result.validationOutput}</pre></details>}
                  <div className="dispatch-card-actions">
                    <PickOneButton
                      visible={cardActions.showPickOne && sameParentsResultCount(graph, group.rows, result.nodeId) >= 2}
                      adopted={adopted}
                      onPick={() => {
                        onCommit(pickOne(graph, result.nodeId));
                        onFocusNode?.(result.nodeId);
                      }}
                    />
                    {cardActions.showAdopt && <button onClick={() => {
                      onCommit(setReplyAdopted(graph, result.nodeId, !adopted));
                    }}>{adopted ? '取消采纳' : '采纳'}</button>}
                    {cardActions.showRetry && <button onClick={() => retry(result.nodeId, false)}>重派</button>}
                    {cardActions.showRetry && <button onClick={() => retry(result.nodeId, true)}>升级</button>}
                    {cardActions.note !== '' && <span className="dispatch-reason">{cardActions.note}</span>}
                    {node?.target?.role === '拆分' && result.status === 'ok' && (
                      <button onClick={() => generateSwarmNodes(result.nodeId)}>按清单生成执行节点</button>
                    )}
                  </div>
                  {planNotice[result.nodeId] && <div className="dispatch-reason">{planNotice[result.nodeId]}</div>}
                </section>
              );})}
                </div>
              ))}
            </div>
            {leftovers.map((node) => (
              <section key={node.id} className="dispatch-route-card">
                <div className="dispatch-stage-heading">遗留</div>
                <div className="dispatch-reason">{node.user.text}；已到修正轮上限，不再派修正。</div>
                <ol className="dispatch-leftover">
                  {node.leftover!.items.map((item, index) => <li key={`${index}-${item}`}>{item}</li>)}
                </ol>
              </section>
            ))}
            {mergeEntry && mergeEntry.nodeIds.length > 0 && (
              <section className="dispatch-route-card">
                <div className="dispatch-stage-heading">蜂群合并</div>
                <div className="dispatch-reason">按件号顺序 cherry-pick，每并一件跑一次验收命令；冲突与验收未通过的件由用户裁决，不自动重派。</div>
                {!swarmReady && <div className="dispatch-reason">还有件没跑完，全部结束后可合并。</div>}
                {swarmReady && swarmPendingItems.length === 0 && <div className="dispatch-reason">全部件已并入。</div>}
                {/* 逐件一行按图上的状态出（08C-2）：本次合并过的件用带验收输出的行 */}
                {mergeEntry.rows.map((row) => {
                  const fresh = mergeResultOf(row.nodeId);
                  return fresh
                    ? <SwarmMergeRow key={row.nodeId} result={fresh} />
                    : (
                      <div key={row.nodeId} className={row.state === 'merged' ? 'dispatch-reason' : row.state === 'pending' ? 'dispatch-reason' : 'dispatch-warning'}>
                        {row.line}
                      </div>
                    );
                })}
                {mergeResults && <div className="dispatch-reason">{swarmMergeSummary(mergeResults)}</div>}
                <div className="dispatch-actions">
                  <button disabled={!swarmReady || merging || swarmPendingItems.length === 0} onClick={runSwarmMerge}>{merging ? '合并中' : '合并'}</button>
                </div>
              </section>
            )}
            {resultStages.map((group) => {
              // 蜂群件不进仲裁：件与件是分工不是分歧（GLM 4）。
              const okRows = arbitrationCandidates(graph, group.rows);
              if (okRows.length < 2) return null;
              const row = arbFor(group.stage, okRows);
              const sideVendors = new Set(
                [row.aId, row.bId]
                  .map((nid) => {
                    const side = graph.nodes[nid]?.target;
                    return side ? findModel(models, side)?.vendor : undefined;
                  })
                  .filter((vendor): vendor is string => !!vendor),
              );
              const arbModels = models.filter((item) => !sideVendors.has(item.vendor));
              const routeOf = (nodeId: string) => {
                const target = graph.nodes[nodeId]?.target;
                return target ? agentModelDisplay(target.agent, target.model) : nodeId;
              };
              const pickSide = (side: 'aId' | 'bId', value: string) => {
                const nextIds = { ...row, [side]: value };
                const text = nextIds.aId && nextIds.bId && nextIds.aId !== nextIds.bId
                  ? neutralDisputeSummary(graph, nextIds.aId, nextIds.bId)
                  : row.text;
                setArbRow(group.stage, { ...row, [side]: value, text, error: '' });
              };
              return (
                <section key={`arb-${group.stage}`} className="dispatch-route-card">
                  <div className="dispatch-stage-heading">第三方仲裁 · 第 {group.stage} 阶段</div>
                  <div className="dispatch-reason">甲乙双方由用户选，分歧交第三家裁决</div>
                  <div className="dispatch-controls">
                    <label>甲方<select value={row.aId} onChange={(event) => pickSide('aId', event.target.value)}>
                      {okRows.map((result) => <option key={result.nodeId} value={result.nodeId}>{routeOf(result.nodeId)}</option>)}
                    </select></label>
                    <label>乙方<select value={row.bId} onChange={(event) => pickSide('bId', event.target.value)}>
                      {okRows.map((result) => <option key={result.nodeId} value={result.nodeId}>{routeOf(result.nodeId)}</option>)}
                    </select></label>
                    <label>目标<select value={row.targetKey} onChange={(event) => setArbRow(group.stage, { ...row, targetKey: event.target.value, error: '' })}>
                      <option value="">选择第三家（与双方不同厂商）</option>
                      {arbModels.map((item) => (
                        <option key={JSON.stringify([item.agent, item.provider, item.id])} value={JSON.stringify([item.agent, item.provider, item.id])}>
                          {agentModelDisplay(item.agent, item.id, item.provider)}{modelStatusSuffix(item)}
                        </option>
                      ))}
                    </select></label>
                  </div>
                  <textarea
                    className="dispatch-arb-text"
                    rows={3}
                    value={row.text}
                    onChange={(event) => setArbRow(group.stage, { ...row, text: event.target.value })}
                  />
                  {row.error && <div className="dispatch-error">{row.error}</div>}
                  <div className="dispatch-actions">
                    <button onClick={() => createArbitration(group.stage, row)}>仲裁</button>
                  </div>
                </section>
              );
            })}
            <table className="dispatch-usage"><thead><tr><th>阶段</th><th>agent/模型</th><th>in / out tokens</th><th>cache_read</th><th>耗时</th><th>结果</th></tr></thead>
              <tbody>{usageGroups.map((group) => <Fragment key={group.stage}>
                {group.rows.map((result) => { const target = graph.nodes[result.nodeId]?.target; return <tr key={result.nodeId}><td>{result.stage}</td><td>{target ? agentModelDisplay(target.agent, target.model) : result.nodeId}{target?.agent === 'cc-switch' ? ' · 单轮' : ''} · {result.runner === 'acp' ? '长驻' : result.runner === 'oneshot' ? '一次性' : '未运行'}</td><td>{result.tokensIn || '—'} / {result.tokensOut || '—'}</td><td>{cacheReadOf(graph.nodes[result.nodeId]?.reply, result) || '—'}</td><td>{result.durationMs} ms</td><td>{resultStatus(result.status)}</td></tr>; })}
                <tr className="dispatch-usage-subtotal"><td>{group.stage} 小计</td><td>{group.rows.length} 路</td><td>{group.tokensIn} / {group.tokensOut}</td><td>{group.cacheRead}</td><td>{group.durationMs} ms</td><td>—</td></tr>
              </Fragment>)}</tbody>
            </table>
            <details className="dispatch-records"><summary>派发记录</summary>
              {results.map((result) => <details key={result.nodeId}><summary>{result.dispatchFile || `${result.nodeId}（未写入）`}</summary><pre>{result.frontmatter}{'\n\n'}{result.outputPreview}</pre></details>)}
            </details>
            <div className="dispatch-actions"><button className="dispatch-primary" onClick={handoff}>汇总裁决</button></div>
          </div>
        )}
      </div>
    </div>
  );
}
