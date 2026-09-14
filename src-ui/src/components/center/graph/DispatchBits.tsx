// DispatchOverlay 的三处小组件（TASK-05B R2-5）：抽出来是为了能用
// react-dom/server 做组件级断言，不只停在纯函数层。

import { fallbackSkippedLineOf, idleHint, rollbackHintOf, writableFallbackHintOf, type BackendNodeResult, type ProgressSnapshot } from '../../../graph/dispatch';
import type { Target } from '../../../graph/types';

// G1 确认屏：可写节点（write != 只读）配了备选时的提示。
export function WritableFallbackHint({ target }: { target: Target }) {
  const hint = writableFallbackHintOf(target);
  if (!hint) return null;
  return <div className="dispatch-warning">{hint}</div>;
}

// 结果卡：可写节点主失败但未切备选时的说明行。
export function FallbackSkippedLine({ result }: { result: Pick<BackendNodeResult, 'fallbackSkipped'> }) {
  const line = fallbackSkippedLineOf(result);
  if (!line) return null;
  return <div className="dispatch-warning">{line}</div>;
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

// 结果卡：擂台「选它」。visible 由调用方按 sameParentsResultCount >= 2 决定。
export function PickOneButton({ visible, adopted, onPick }: { visible: boolean; adopted: boolean; onPick: () => void }) {
  if (!visible) return null;
  return (
    <button disabled={adopted} onClick={onPick}>
      {adopted ? '已选中' : '选它'}
    </button>
  );
}
