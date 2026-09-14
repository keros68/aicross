// DispatchOverlay 的三处小组件（TASK-05B R2-5）：抽出来是为了能用
// react-dom/server 做组件级断言，不只停在纯函数层。

import { fallbackSkippedLineOf, writableFallbackHintOf, type BackendNodeResult } from '../../../graph/dispatch';
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

// 结果卡：擂台「选它」。visible 由调用方按 sameParentsResultCount >= 2 决定。
export function PickOneButton({ visible, adopted, onPick }: { visible: boolean; adopted: boolean; onPick: () => void }) {
  if (!visible) return null;
  return (
    <button disabled={adopted} onClick={onPick}>
      {adopted ? '已选中' : '选它'}
    </button>
  );
}
