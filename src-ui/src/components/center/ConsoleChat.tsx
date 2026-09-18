import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useAppState, type TerminalSession } from '../../store/app-state';
import { commands, isTauri } from '../../tauri';
import type { DispatchableModel } from '../../graph/types';
import { agentDisplayName } from '../../resources/agent-display';
import { RESOURCES_CHANGED_EVENT } from '../../resources/mention';
import { ConsoleRunDetail } from './ConsoleRunDetail';
import {
  consoleResultMessage,
  createConsoleApplyHandler,
  dispatchConsoleTurnWithFallback,
  newConsoleChatRunId,
} from '../../console-chat';
import {
  ACP_COMMANDS_EVENT,
  ACP_PERMISSION_CLOSED_EVENT,
  ACP_PERMISSION_EVENT,
  commandListKey,
  commandText,
  completeSlashCommand,
  filterCommands,
  needsFallbackReject,
  permissionBelongsTo,
  slashQuery,
  subscribeAcpEvent,
  supportsSlashCommands,
  type AcpCommand,
  type AcpPermissionRequest,
} from '../../console-acp';
import { selectConsoleGraph } from '../../console-channel';
import {
  consoleScrollRestoreTop,
  consoleScrollSnapshot,
  consoleAutoScrollStep,
  runDetailAvailabilityAction,
  runDetailFocusTarget,
  type ConsoleScrollSnapshot,
} from '../../console-run-detail';
import {
  CONSOLE_APPLY_EVENT,
  completeConsoleMention,
  consoleMessage,
  consoleResumeId,
  consoleResumeKey,
  mentionTargets,
  parseConsoleMention,
  type ConsoleMessage,
} from '../../console-session';
import './ConsoleChat.css';

interface Props {
  session: TerminalSession;
}

export function ConsoleChat({ session }: Props) {
  const { state, dispatch } = useAppState();
  const [draft, setDraft] = useState('');
  const [models, setModels] = useState<DispatchableModel[]>([]);
  const [busy, setBusy] = useState(false);
  const [openRunId, setOpenRunId] = useState('');
  const [permissions, setPermissions] = useState<AcpPermissionRequest[]>([]);
  // 每家 agent × 目录的 `/` 命令清单：undefined 未取、'loading' 取中、数组为结果（空数组 = 没有）。
  const [commandLists, setCommandLists] = useState<Record<string, AcpCommand[] | 'loading'>>({});
  const chatRunIdRef = useRef('');
  // 取到空清单（适配器暂时没推）的时间：5 分钟后再输入 / 会重取，不把空结果一直留着。
  const emptyCommandsAtRef = useRef<Record<string, number>>({});
  // 与 permissions 同步维护、但在事件回调里立即更新：卸载时据它拒绝，不漏掉还没渲染的请求。
  const permissionsRef = useRef<AcpPermissionRequest[]>([]);
  const busyRef = useRef(false);
  const appliedRef = useRef(session.lastAppliedMessageId);
  const endRef = useRef<HTMLDivElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const scrollSnapshotRef = useRef<ConsoleScrollSnapshot | null>(null);
  const skipAutoScrollRef = useRef(false);
  const focusBeforeRunRef = useRef<HTMLElement | null>(null);
  const restoreFocusPendingRef = useRef(false);
  const target = session.targetAgent;
  const messages = session.messages ?? [];
  const runRecords = session.runRecords ?? [];
  const runsById = useMemo(() => new Map(runRecords.map((run) => [run.id, run])), [runRecords]);
  const openRun = runsById.get(openRunId);
  const activeRun = [...runRecords].reverse().find((run) => run.status === 'running');

  useEffect(() => {
    if (!isTauri) return;
    let dead = false;
    const refresh = () => {
      commands.listDispatchableModels()
        .then((result) => { if (!dead) setModels(result.models); })
        .catch(() => { if (!dead) setModels([]); });
    };
    refresh();
    window.addEventListener(RESOURCES_CHANGED_EVENT, refresh);
    return () => {
      dead = true;
      window.removeEventListener(RESOURCES_CHANGED_EVENT, refresh);
    };
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    const opened = subscribeAcpEvent<AcpPermissionRequest>(ACP_PERMISSION_EVENT, (request) => {
      if (!permissionBelongsTo(request, chatRunIdRef.current)) return;
      permissionsRef.current = [...permissionsRef.current.filter((item) => item.requestId !== request.requestId), request];
      setPermissions(permissionsRef.current);
    });
    const closed = subscribeAcpEvent<{ requestId: string }>(ACP_PERMISSION_CLOSED_EVENT, ({ requestId }) => {
      permissionsRef.current = permissionsRef.current.filter((item) => item.requestId !== requestId);
      setPermissions(permissionsRef.current);
    });
    const listed = subscribeAcpEvent<{ agent: string; cwd: string; commands: AcpCommand[] }>(ACP_COMMANDS_EVENT, ({ agent, cwd, commands: list }) => {
      setCommandLists((current) => ({ ...current, [commandListKey(agent, cwd)]: list }));
    });
    return () => {
      opened.dispose();
      closed.dispose();
      listed.dispose();
      // 关掉 tab 时卡片再无人回答：本轮挂着的与之后再到的权限请求一律由引擎按拒绝回，
      // 别让它等到整轮超时还占着进程锁（事件还没送到界面的那条也算）。
      for (const request of permissionsRef.current) {
        commands.answerAcpPermission(request.requestId, null).catch(() => {});
      }
      if (chatRunIdRef.current) commands.closeAcpPermissions(chatRunIdRef.current).catch(() => {});
    };
  }, []);

  useEffect(() => {
    if (openRunId) return;
    const step = consoleAutoScrollStep(skipAutoScrollRef.current);
    skipAutoScrollRef.current = step.skipNext;
    if (!step.shouldScroll) return;
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [messages.length, busy, openRunId]);

  useLayoutEffect(() => {
    if (openRunId) return;
    if (scrollSnapshotRef.current && messageListRef.current) {
      const list = messageListRef.current;
      list.scrollTop = consoleScrollRestoreTop(scrollSnapshotRef.current, {
        scrollHeight: list.scrollHeight,
        clientHeight: list.clientHeight,
        messageCount: messages.length,
      });
      scrollSnapshotRef.current = null;
      skipAutoScrollRef.current = true;
    }
    if (restoreFocusPendingRef.current) {
      restoreFocusPendingRef.current = false;
      const previous = focusBeforeRunRef.current;
      const focusTarget = runDetailFocusTarget(Boolean(previous?.isConnected));
      (focusTarget === 'previous' ? previous : composerRef.current)?.focus();
      focusBeforeRunRef.current = null;
    }
  }, [messages.length, openRunId]);

  useEffect(() => { appliedRef.current = session.lastAppliedMessageId; }, [session.lastAppliedMessageId]);

  useEffect(() => {
    const graphId = session.consoleGraphId;
    if (!isTauri || !graphId || state.graphs[graphId]) return;
    let dead = false;
    commands.loadGraph(graphId).then((graph) => {
      if (!dead && graph) dispatch({ type: 'SET_GRAPH', graph });
    }).catch(() => {});
    return () => { dead = true; };
  }, [dispatch, session.consoleGraphId, state.graphs]);

  const recentTargets = useMemo(() => [
    ...[...messages].reverse().flatMap((message) => message.agent && (message.dispatchModel || message.model) ? [{
      agent: message.agent,
      provider: message.provider ?? '',
      model: message.dispatchModel ?? message.model ?? '',
    }] : []),
    ...(target ? [target] : []),
  ], [messages, target]);
  const mentions = useMemo(() => mentionTargets(models, recentTargets), [models, recentTargets]);
  const slash = target ? slashQuery(draft, target, mentions) : null;
  const slashAgent = slash && supportsSlashCommands(slash.target.agent) ? slash.target.agent.trim().toLowerCase() : '';
  const slashKey = slashAgent ? commandListKey(slashAgent, session.folderPath ?? '') : '';
  const slashList = slashKey ? commandLists[slashKey] : undefined;
  const slashMatches = slash && Array.isArray(slashList) ? filterCommands(slashList, slash.prefix) : [];
  const showMentions = !slash && draft.startsWith('@') && !draft.includes('\n') && mentions.length > 0;

  useEffect(() => {
    if (!isTauri || !slashKey) return;
    const known = commandLists[slashKey];
    const emptyAt = emptyCommandsAtRef.current[slashKey] ?? 0;
    const staleEmpty = Array.isArray(known) && known.length === 0 && Date.now() - emptyAt > 5 * 60 * 1000;
    if (known !== undefined && !staleEmpty) return;
    const settle = (list: AcpCommand[]) => {
      if (list.length === 0) emptyCommandsAtRef.current[slashKey] = Date.now();
      setCommandLists((current) => ({ ...current, [slashKey]: list }));
    };
    setCommandLists((current) => ({ ...current, [slashKey]: 'loading' }));
    commands.acpCommands(slashAgent, session.folderPath ?? '')
      .then(settle)
      .catch(() => settle([]));
  }, [commandLists, session.folderPath, slashAgent, slashKey]);

  const answerPermission = (request: AcpPermissionRequest, optionId: string | null) => {
    permissionsRef.current = permissionsRef.current.filter((item) => item.requestId !== request.requestId);
    setPermissions(permissionsRef.current);
    commands.answerAcpPermission(request.requestId, optionId).catch(() => {});
  };
  const append = (message: ConsoleMessage) => {
    dispatch({ type: 'APPEND_CONSOLE_MESSAGE', id: session.id, message });
  };
  const openRunDetail = (runId: string) => {
    focusBeforeRunRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const list = messageListRef.current;
    if (list) {
      scrollSnapshotRef.current = consoleScrollSnapshot({
        scrollTop: list.scrollTop,
        scrollHeight: list.scrollHeight,
        clientHeight: list.clientHeight,
        messageCount: messages.length,
      });
    }
    setOpenRunId(runId);
  };
  const closeRunDetail = () => {
    restoreFocusPendingRef.current = true;
    setOpenRunId('');
  };

  useLayoutEffect(() => {
    if (runDetailAvailabilityAction(openRunId, Boolean(openRun)) === 'close') closeRunDetail();
  }, [openRun, openRunId]);

  const send = async () => {
    const raw = draft.trim();
    if (busyRef.current || raw === '' || !target) return;
    const parsed = parseConsoleMention(raw, mentions);
    const selected = parsed.target ?? target;
    const outbound = parsed.recognized ? parsed.text.trim() : raw;
    if (outbound === '') {
      append(consoleMessage('system', '请输入要发送的内容。'));
      return;
    }
    append(consoleMessage('user', raw));
    setDraft('');
    busyRef.current = true;
    setBusy(true);
    const runId = newConsoleChatRunId();
    chatRunIdRef.current = runId;
    try {
      const turn = await dispatchConsoleTurnWithFallback(
        session.folderPath ?? '',
        outbound,
        selected,
        consoleResumeId(session.resumeId, selected),
        () => {
          dispatch({
            type: 'SET_CONSOLE_RESUME_ID',
            id: session.id,
            key: consoleResumeKey(selected),
            resumeId: '',
          });
          append(consoleMessage('system', '上次的会话已失效，已开新会话'));
        },
        commands,
        runId,
      );
      if (turn.result.sessionId) {
        dispatch({
          type: 'SET_CONSOLE_RESUME_ID',
          id: session.id,
          key: consoleResumeKey(selected),
          resumeId: turn.result.sessionId,
        });
      }
      append(consoleResultMessage(selected, turn.result));
    } catch (error) {
      append(consoleMessage('system', String(error).replace(/^Error:\s*/, '')));
    } finally {
      chatRunIdRef.current = '';
      permissionsRef.current = [];
      setPermissions([]);
      busyRef.current = false;
      setBusy(false);
    }
  };

  useEffect(() => {
    const apply = createConsoleApplyHandler({
      sessionId: session.id,
      project: session.folderPath ?? '',
      snapshot: () => ({
        busy: busyRef.current,
        draft,
        messages,
        lastAppliedMessageId: appliedRef.current,
        existingGraph: session.consoleGraphId
          ? state.graphs[session.consoleGraphId]
          : selectConsoleGraph(state, session.folderPath ?? ''),
      }),
      append,
      clearDraft: () => setDraft(''),
      setBusy: (next) => {
        busyRef.current = next;
        setBusy(next);
      },
      markApplied: (messageId) => {
        appliedRef.current = messageId;
        dispatch({ type: 'SET_CONSOLE_APPLIED_MESSAGE', id: session.id, messageId });
      },
      commitGraph: (graph) => dispatch({ type: 'SET_GRAPH', graph }),
      rememberGraph: (graphId) => dispatch({ type: 'SET_CONSOLE_GRAPH_ID', id: session.id, graphId }),
      onRunUpdate: (run) => dispatch({ type: 'UPSERT_CONSOLE_RUN', id: session.id, run }),
      onRunProgress: (run) => dispatch({ type: 'UPDATE_CONSOLE_RUN_PROGRESS', id: session.id, run }),
      onOutcome: () => {},
    });
    window.addEventListener(CONSOLE_APPLY_EVENT, apply);
    return () => window.removeEventListener(CONSOLE_APPLY_EVENT, apply);
  }, [dispatch, draft, messages, session.consoleGraphId, session.folderPath, session.id, state]);

  if (!target) return null;
  if (openRun) return <ConsoleRunDetail record={openRun} onClose={closeRunDetail} />;
  return (
    <div className="console-chat">
      <div className="console-chat-head">
        <span>{agentDisplayName(target.agent)}</span>
        <span>{target.provider ? `${target.provider}/${target.model}` : target.model}</span>
        <span>{session.folderPath || '未选目录'}</span>
      </div>
      <div ref={messageListRef} className="console-message-list" aria-live="polite">
        {messages.length === 0 && (
          <div className="console-message-empty">直接输入可单线聊；输入 @ 可临时改派，输入 / 可用各家的命令与技能。聊天不改文件，执行命令或调用 MCP 工具前会先请你确认；要改文件请用模式派发。</div>
        )}
        {messages.map((message) => (
          <div key={message.id} className={`console-message console-message-${message.kind}`}>
            <div className="console-message-label">
              {message.kind === 'user'
                ? '用户'
                : message.kind === 'system'
                  ? '系统提示'
                  : agentDisplayName(message.agent ?? '')}
            </div>
            <div className="console-message-text">{message.text}</div>
            {(message.agent || message.model || message.role || message.route || message.durationMs !== undefined) && (
              <div className="console-message-meta">
                {[message.agent ? agentDisplayName(message.agent) : '', message.model, message.role,
                  message.route, message.durationMs !== undefined ? `${(message.durationMs / 1000).toFixed(1)} 秒` : '']
                  .filter(Boolean).join(' · ')}
              </div>
            )}
            {message.runId && runsById.has(message.runId) && (
              <button
                type="button"
                className="console-message-action"
                onClick={() => openRunDetail(message.runId ?? '')}
              >
                {runsById.get(message.runId)?.status === 'running' ? '查看进度' : '查看详情'}
              </button>
            )}
          </div>
        ))}
        {permissions.map((request) => (
          <div key={request.requestId} className="console-permission" role="alertdialog" aria-label="工具调用确认">
            <div className="console-message-label">{agentDisplayName(request.agent)} 请求调用工具</div>
            <div className="console-permission-title">{request.title}</div>
            {request.detail && <pre className="console-permission-detail">{request.detail}</pre>}
            <div className="console-permission-actions">
              {request.options.map((option) => (
                <button
                  type="button"
                  key={option.optionId}
                  className={option.kind === 'allow_once' ? 'console-permission-allow' : undefined}
                  onClick={() => answerPermission(request, option.optionId)}
                >
                  {option.name}
                </button>
              ))}
              {needsFallbackReject(request.options) && (
                <button type="button" onClick={() => answerPermission(request, null)}>拒绝</button>
              )}
            </div>
          </div>
        ))}
        {busy && (
          <div className="console-message-running">
            <span>处理中…</span>
            {activeRun && (
              <button type="button" className="console-running-action" onClick={() => openRunDetail(activeRun.id)}>
                查看进度
              </button>
            )}
          </div>
        )}
        <div ref={endRef} />
      </div>
      <div className="console-composer">
        {slash && slashAgent && (
          <div className="console-mentions console-commands" role="listbox" aria-label="可用命令">
            {slashList === undefined || slashList === 'loading'
              ? <span className="console-commands-note">正在读取命令…</span>
              : slashMatches.length === 0
                ? <span className="console-commands-note">没有匹配的命令</span>
                : slashMatches.map((command) => (
                  <button
                    type="button"
                    key={command.name}
                    title={command.description}
                    onClick={() => {
                      setDraft(completeSlashCommand(slash, command));
                      composerRef.current?.focus();
                    }}
                  >
                    <span className="console-command-name">{commandText(command)}</span>
                    {command.description && <span className="console-command-desc">{command.description}</span>}
                  </button>
                ))}
          </div>
        )}
        {showMentions && (
          <div className="console-mentions" role="listbox" aria-label="可改派的 agent">
            {mentions.map((item) => (
              <button
                type="button"
                key={item.agent}
                onClick={() => setDraft(completeConsoleMention(draft, item))}
              >
                @{agentDisplayName(item.agent)}
              </button>
            ))}
          </div>
        )}
        <textarea
          ref={composerRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Tab' && slash && slashMatches.length > 0) {
              event.preventDefault();
              setDraft(completeSlashCommand(slash, slashMatches[0]));
              return;
            }
            if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              void send();
            }
          }}
          placeholder={`发给 ${agentDisplayName(target.agent)}，输入 @ 可临时改派，输入 / 用命令`}
          rows={3}
        />
        <button type="button" disabled={busy || draft.trim() === ''} onClick={() => void send()}>
          发送
        </button>
      </div>
    </div>
  );
}
