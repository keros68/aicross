// DispatchOverlay 的三处小组件（TASK-05B R2-5）：抽出来是为了能用
// react-dom/server 做组件级断言，不只停在纯函数层。

import { fallbackSkippedLineOf, idleHint, pinnedLabelOf, rollbackHintOf, runningDetailOf, writableFallbackHintOf, type BackendNodeResult, type ProgressSnapshot } from '../../../graph/dispatch';
import { swarmResultLine, type SwarmGateState } from '../../../graph/swarm';
import type { SwarmMergeResult } from '../../../tauri';
import type { Target } from '../../../graph/types';
import type { DispatchableModel } from '../../../graph/types';
import { quotaExhaustedLabel, verificationHint, type Verification } from '../../../resources/types';

// G1 确认屏：可写节点配了备选时的额度回滚提示。
export function WritableFallbackHint({ target, models = [] }: { target: Target; models?: DispatchableModel[] }) {
  const hint = writableFallbackHintOf(target, models);
  if (!hint) return null;
  return <div className="dispatch-warning">{hint}</div>;
}

// 结果卡：可写节点主失败但未切备选时的说明行。
export function FallbackSkippedLine({ result }: { result: Pick<BackendNodeResult, 'fallbackSkipped' | 'status'> }) {
  const line = fallbackSkippedLineOf(result);
  if (!line) return null;
  return <div className="dispatch-warning">{line}</div>;
}

export function PinnedStatus({ pinned }: { pinned?: BackendNodeResult['pinned'] }) {
  const label = pinnedLabelOf(pinned);
  if (!label) return null;
  return <span className={`dispatch-pin-status${pinned === 'unpinned' ? ' unpinned' : ''}`}>{label}</span>;
}

export function PinnedNote({ result }: { result: Pick<BackendNodeResult, 'pinnedNote' | 'status'> }) {
  if (!result.pinnedNote) return null;
  return <div className={result.status === 'error' ? 'dispatch-warning' : 'dispatch-pin-note'}>{result.pinnedNote}</div>;
}

// G1 确认屏：目标模型的验证档提示（修正轮 R1）。未验证是第三方模型的常态，
// 走 dispatch-reason；红字只留给 failed。文案与 tone 都在 verificationHint 里，
// 这里只负责渲染。
export function VerificationHint({ verification }: { verification?: Verification }) {
  const hint = verificationHint(verification);
  if (!hint) return null;
  return <div className={hint.tone === 'warning' ? 'dispatch-warning' : 'dispatch-reason'}>{hint.text}</div>;
}

export function QuotaHint({ model }: { model?: { quota_exhausted_until?: string | null } }) {
  if (!model) return null;
  const label = quotaExhaustedLabel(model);
  if (!label) return null;
  return <div className="dispatch-warning">{label}，默认不派。</div>;
}

export function QuotaResetLine({ resetsAt }: { resetsAt?: string | null }) {
  if (!resetsAt) return null;
  return <div className="dispatch-reason">恢复时间：{new Date(resetsAt).toLocaleString('zh-CN')}</div>;
}

// G1 确认屏：单写者节点重派时，说明引擎会先回滚上次改动（TASK-06-2）。
// 这是常规提示不是异常，用 dispatch-reason 不用红字（R1-13）。
export function RollbackBeforeHint({ target, retrying }: { target?: Target; retrying: boolean }) {
  const hint = rollbackHintOf(target, retrying);
  if (!hint) return null;
  return <div className="dispatch-reason">{hint}</div>;
}

// G1 确认屏：崩溃后对账判「未知」的节点标签（TASK-06-1）。
export function ReviewPendingTag({ visible }: { visible: boolean }) {
  if (!visible) return null;
  return <span className="hint-capsule">待核对</span>;
}

// G1「待核对」区的一行（R1-10）：勾不勾都看得到路由行与标签——未勾选的节点
// 在下面的路由卡列表里没有位置。
export function ReviewPendingRow(
  { line, checked, onToggle }: { line: string; checked: boolean; onToggle: () => void },
) {
  return (
    <label className="dispatch-review-row">
      <input type="checkbox" checked={checked} onChange={onToggle} />
      <span className="dispatch-route-line">{line}</span>
      <ReviewPendingTag visible />
    </label>
  );
}

// G2 运行中：疑似空转的胶囊提示（TASK-07-1）。只提示不阻止，不用红字；
// 用户要停就按旁边的「叫停」。
export function IdleHint({ progress, now }: { progress?: ProgressSnapshot; now: number }) {
  const hint = idleHint(progress, now);
  if (!hint) return null;
  return <span className="hint-capsule">{hint}</span>;
}

// 图上的节点卡：运行中那一行（TASK-07C）。与运行屏读同一份快照、用同一套
// 判定，只是排版不同——卡片定宽，读数过长时省略，完整文本在 title 里；胶囊
// 不压缩。既没有读数也没有提示时不占行。
export function NodeProgressRow({ progress, now }: { progress?: ProgressSnapshot; now: number }) {
  const detail = runningDetailOf(progress, now);
  if (detail === '' && idleHint(progress, now) === null) return null;
  return (
    <div className="graph-node-row graph-node-progress" title={detail || undefined}>
      {detail !== '' && <span className="graph-node-elapsed">{detail}</span>}
      <IdleHint progress={progress} now={now} />
    </div>
  );
}

// 结果卡：擂台「选定」。visible 由调用方按 sameParentsResultCount >= 2 决定。
export function PickOneButton({ visible, adopted, onPick }: { visible: boolean; adopted: boolean; onPick: () => void }) {
  if (!visible) return null;
  return (
    <button disabled={adopted} onClick={onPick}>
      {adopted ? '已选定' : '选定'}
    </button>
  );
}

// G1 确认屏：蜂群清单闸门（TASK-08 B-4）。重叠与脏树是阻断，用警告样式；
// 「正在核对」与 glob 没匹配到文件只提示，走常规灰字。核对不了（命令失败）时
// 也阻断，但给一个「仍要派发」的选择（R1-1）。
export function SwarmGateNotice(
  { gate, proceed, onProceed }: {
    gate: SwarmGateState;
    proceed?: boolean;
    onProceed?: (value: boolean) => void;
  },
) {
  if (gate.reasons.length === 0 && gate.hints.length === 0) return null;
  return (
    <div className="dispatch-swarm-gate">
      {gate.reasons.map((reason) => (
        <div key={reason} className="dispatch-warning">{reason}</div>
      ))}
      {gate.hints.map((hint) => (
        <div key={hint} className="dispatch-reason">{hint}</div>
      ))}
      {gate.canProceed && onProceed && (
        <label className="dispatch-swarm-proceed">
          <input
            type="checkbox"
            checked={proceed === true}
            onChange={(event) => onProceed(event.target.checked)}
          />
          仍要派发
        </label>
      )}
    </div>
  );
}

// G3 结果屏：逐件合并结果的一行（TASK-08 B-5）。测试红时把输出折起来放在行下。
export function SwarmMergeRow({ result }: { result: SwarmMergeResult }) {
  return (
    <div className="dispatch-swarm-row">
      <div className={result.result === 'merged' ? 'dispatch-reason' : 'dispatch-warning'}>
        {swarmResultLine(result)}
      </div>
      {result.testOutput !== '' && (
        <details><summary>验收输出</summary><pre>{result.testOutput}</pre></details>
      )}
    </div>
  );
}
