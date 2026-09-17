import { useEffect, useMemo, useRef, useState } from 'react';
import { useAppState, type TerminalSession } from '../../store/app-state';
import { commands, isTauri } from '../../tauri';
import type { DispatchableModel } from '../../graph/types';
import { agentDisplayName } from '../../resources/agent-display';
import { RESOURCES_CHANGED_EVENT } from '../../resources/mention';
import {
  consoleResultMessage,
  createConsoleApplyHandler,
  dispatchConsoleTurnWithFallback,
} from '../../console-chat';
import { selectConsoleGraph } from '../../console-channel';
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
  const busyRef = useRef(false);
  const appliedRef = useRef(session.lastAppliedMessageId);
  const endRef = useRef<HTMLDivElement>(null);
  const target = session.targetAgent;
  const messages = session.messages ?? [];

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
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [messages.length, busy]);

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
  const showMentions = draft.startsWith('@') && !draft.includes('\n') && mentions.length > 0;
  const append = (message: ConsoleMessage) => {
    dispatch({ type: 'APPEND_CONSOLE_MESSAGE', id: session.id, message });
  };

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
      onOutcome: () => {},
    });
    window.addEventListener(CONSOLE_APPLY_EVENT, apply);
    return () => window.removeEventListener(CONSOLE_APPLY_EVENT, apply);
  }, [dispatch, draft, messages, session.consoleGraphId, session.folderPath, session.id, state]);

  if (!target) return null;
  return (
    <div className="console-chat">
      <div className="console-chat-head">
        <span>{agentDisplayName(target.agent)}</span>
        <span>{target.provider ? `${target.provider}/${target.model}` : target.model}</span>
        <span>{session.folderPath || '未选目录'}</span>
      </div>
      <div className="console-message-list" aria-live="polite">
        {messages.length === 0 && (
          <div className="console-message-empty">直接输入可单线聊；输入 @ 可临时改派。控制台聊天当前只读，要改文件请用模式派发。</div>
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
          </div>
        ))}
        {busy && <div className="console-message-running">处理中…</div>}
        <div ref={endRef} />
      </div>
      <div className="console-composer">
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
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              void send();
            }
          }}
          placeholder={`发给 ${agentDisplayName(target.agent)}，输入 @ 可临时改派`}
          rows={3}
        />
        <button type="button" disabled={busy || draft.trim() === ''} onClick={() => void send()}>
          发送
        </button>
      </div>
    </div>
  );
}
