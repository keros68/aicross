// 验证状态标记（TASK-09）。按 PRODUCT.md「视觉纪律」的状态胶囊一条：
// 胶囊只给异常状态，常态与完成态用色点或纯文字。
//
//   native      无点无字（原生 CLI 清单说有就是有，不占视觉）
//   verified    绿点，左栏不带字
//   unverified  灰点 +「未验证」
//   failed      红胶囊「验证失败」

import { verificationLabel, type Verification } from '../../resources/types';

export function VerificationMark({
  verification,
  showLabel = false,
}: {
  verification: Verification;
  showLabel?: boolean;
}) {
  if (verification === 'native') {
    return showLabel ? <span className="verify-text">原生</span> : null;
  }
  if (verification === 'failed') {
    return <span className="verify-pill">{verificationLabel(verification)}</span>;
  }
  const label = verificationLabel(verification);
  // 未验证一定带字：左栏里一个灰点看不出是什么。已验证的绿点在左栏不带字，
  // 状态词只留给读屏（视觉纪律：完成态不占版面）。
  const withLabel = showLabel || verification === 'unverified';
  return (
    <span className={`verify-dot-wrap verify-${verification}`}>
      <span className="verify-dot" aria-hidden="true" />
      {withLabel ? (
        <span className="verify-text">{label}</span>
      ) : (
        <span className="resources-sr-only">{label}</span>
      )}
    </span>
  );
}
