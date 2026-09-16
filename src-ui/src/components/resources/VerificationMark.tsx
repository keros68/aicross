// 验证状态标记（TASK-09；TASK-09b 收紧左栏）。按 PRODUCT.md「视觉纪律」的
// 状态胶囊一条：胶囊只给异常状态，常态与完成态用色点或纯文字。
//
// 模型行只留一个点，文字进 `aria-label` / `title`。用户
// 09-15 反馈 pi 底下几十行都跟着「未验证」三个字，读起来是噪声。
//   native 无点  ·  verified 绿点  ·  unverified 灰点  ·  failed 红点

import { verificationLabel, type Verification } from '../../resources/types';

export function VerificationMark({ verification }: { verification: Verification }) {
  const label = verificationLabel(verification);
  if (verification === 'native') return null;
  // 点是唯一可见元素：`role="img"` + `aria-label` 让读屏念出状态词，
  // `title` 让鼠标用户悬停看到同一句（两者缺一都会漏掉一半用户）。
  return (
    <span
      className={`verify-dot verify-${verification}`}
      role="img"
      aria-label={label}
      title={label}
    />
  );
}
