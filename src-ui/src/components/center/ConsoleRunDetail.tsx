import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import {
  acceptanceLabel,
  consoleRunHeaderLabel,
  consoleRunTotals,
  liveConsoleRunDurationMs,
  reportedModelLabel,
  runDetailKeyAction,
  runDetailTabPlan,
  runRowStatusLabel,
  tokenUnreported,
  type ConsoleRunRecord,
  type ConsoleRunRow,
} from '../../console-run-detail';
import { agentDisplayName } from '../../resources/agent-display';

interface Props {
  record: ConsoleRunRecord;
  onClose: () => void;
}

export const RUN_TABLE_COLUMN_WIDTHS = [66, 62, 126, 118, 106, 58, 84, 84, 86, 90];

const FOCUSABLE_SELECTOR = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function seconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)} 秒`;
}

export function compactRunNumber(value: number): string {
  if (value < 10_000) return String(value);
  if (value < 1_000_000) return `${Number((value / 1000).toFixed(1))}k`;
  return `${Number((value / 1_000_000).toFixed(1))}m`;
}

function tokenValue(row: ConsoleRunRow, value: number): string {
  if (tokenUnreported(row)) return '未报告';
  if (row.status === 'pending' || row.status === 'running') return value > 0 ? compactRunNumber(value) : '—';
  return compactRunNumber(value);
}

function RunRow({ row, now }: { row: ConsoleRunRow; now: number }) {
  const fallback = row.fallbackFrom && row.fallbackTo
    ? `主 → 备：${row.fallbackFrom} → ${row.fallbackTo}`
    : '';
  const tool = row.toolCalls > 0 || row.lastTool
    ? `最近工具：${row.lastTool || '未报告'} · ${row.toolCalls} 次`
    : '';
  return (
    <tr>
      <td>{row.role || '未指定角色'}</td>
      <td>{agentDisplayName(row.agent)}</td>
      <td className="console-run-mono">
        <div>{row.requestedModel}</div>
        {fallback && <div className="console-run-route">{fallback}</div>}
      </td>
      <td className="console-run-mono">{reportedModelLabel(row)}</td>
      <td>
        <div>{runRowStatusLabel(row.status)}</div>
        {tool && <div className="console-run-tool">{tool}</div>}
      </td>
      <td>{acceptanceLabel(row.acceptance)}</td>
      <td className="console-run-number" title={String(row.tokensIn)}>{tokenValue(row, row.tokensIn)}</td>
      <td className="console-run-number" title={String(row.tokensOut)}>{tokenValue(row, row.tokensOut)}</td>
      <td className="console-run-number" title={String(row.cacheRead)}>{tokenValue(row, row.cacheRead)}</td>
      <td className="console-run-number">{row.status === 'pending' ? '—' : seconds(liveConsoleRunDurationMs(row, now))}</td>
    </tr>
  );
}

export function ConsoleRunDetail({ record, onClose }: Props) {
  const rootRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const [now, setNow] = useState(() => Date.now());
  const running = record.status === 'running';

  useLayoutEffect(() => {
    (closeRef.current ?? rootRef.current)?.focus();
  }, [record.id]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (runDetailKeyAction(event) === 'close') {
        event.preventDefault();
        event.stopPropagation();
        onClose();
        return;
      }
      if (event.key !== 'Tab' || event.isComposing) return;
      const focusable = [...(rootRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? [])];
      const plan = runDetailTabPlan(focusable.indexOf(document.activeElement as HTMLElement), focusable.length, event.shiftKey);
      event.preventDefault();
      event.stopPropagation();
      if (plan.kind === 'root') rootRef.current?.focus();
      else focusable[plan.index].focus();
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [onClose]);

  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  const stages = [...new Set(record.rows.map((row) => row.stage))].sort((a, b) => a - b);
  const totals = consoleRunTotals(record, now);
  return (
    <div
      ref={rootRef}
      className="console-run-detail"
      role="dialog"
      aria-modal="true"
      aria-label="运行详情"
      tabIndex={-1}
    >
      <header className="console-run-head">
        <div>
          <h2>运行详情</h2>
          <div className="console-run-sub">
            <span>{consoleRunHeaderLabel(record)}</span>
          </div>
        </div>
        <button ref={closeRef} type="button" onClick={onClose}>返回对话</button>
      </header>

      <div className="console-run-body">
        <section className="console-run-summary" aria-label="运行合计">
          <div><span>结论</span><strong>{record.conclusion || '进行中'}</strong></div>
          <div><span>路数</span><strong>{totals.routes}</strong></div>
          <div><span>输入 token</span><strong title={String(totals.tokensIn)}>{compactRunNumber(totals.tokensIn)}</strong></div>
          <div><span>输出 token</span><strong title={String(totals.tokensOut)}>{compactRunNumber(totals.tokensOut)}</strong></div>
          <div><span>缓存命中</span><strong title={String(totals.cacheRead)}>{compactRunNumber(totals.cacheRead)}</strong></div>
          <div><span>墙钟耗时</span><strong>{seconds(totals.wallDurationMs)}</strong></div>
          <div><span>各路耗时合计</span><strong>{seconds(totals.routeDurationMs)}</strong></div>
          {totals.unreported > 0 && (
            <div><span>token 说明</span><strong>{totals.unreported} 路未报告，未计入</strong></div>
          )}
        </section>
        <div className="console-run-note">墙钟耗时按整次运行起止计算；各路耗时合计为每一路耗时之和。</div>

        {stages.map((stage) => {
          const rows = record.rows.filter((row) => row.stage === stage);
          const subtotal = consoleRunTotals({ ...record, rows }, now);
          return (
            <section className="console-run-stage" key={stage}>
              <div className="console-run-stage-head">
                <h3>第 {stage} 阶段</h3>
                <span>{rows.length} 路</span>
              </div>
              <div className="console-run-table-wrap">
                <table className="console-run-table">
                  <colgroup>
                    {RUN_TABLE_COLUMN_WIDTHS.map((width, index) => (
                      <col key={index} style={{ width }} />
                    ))}
                  </colgroup>
                  <thead><tr>
                    <th>角色</th><th>agent</th><th>请求模型</th><th>实际模型</th><th>状态</th>
                    <th>结论</th><th>输入 token</th><th>输出 token</th><th>缓存命中</th><th>耗时</th>
                  </tr></thead>
                  <tbody>
                    {rows.map((row) => <RunRow key={row.nodeId} row={row} now={now} />)}
                  </tbody>
                  <tfoot><tr>
                    <td colSpan={2}>阶段小计</td>
                    <td colSpan={4}>{subtotal.routes} 路{subtotal.unreported > 0 ? `；${subtotal.unreported} 路未报告，未计入` : ''}</td>
                    <td className="console-run-number" title={String(subtotal.tokensIn)}>{compactRunNumber(subtotal.tokensIn)}</td>
                    <td className="console-run-number" title={String(subtotal.tokensOut)}>{compactRunNumber(subtotal.tokensOut)}</td>
                    <td className="console-run-number" title={String(subtotal.cacheRead)}>{compactRunNumber(subtotal.cacheRead)}</td>
                    <td className="console-run-number">{seconds(subtotal.routeDurationMs)}</td>
                  </tr></tfoot>
                </table>
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}
