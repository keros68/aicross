// 讲解（TASK-02 C2.6）：全屏覆盖层，说明体中文，内容抽自 docs/PRODUCT.md。

import { createPortal } from 'react-dom';
import './OrchestrationPanel.css';

export function ExplainOverlay({ onClose }: { onClose: () => void }) {
  return createPortal(
    <div className="explain-backdrop">
      <div className="explain-page">
        <button className="explain-close" onClick={onClose}>
          关闭
        </button>
        <h1>对话图与编排</h1>

        <section>
          <h2>什么是节点与分叉</h2>
          <p>
            节点是一轮对话：一条用户消息，加一个模型的回复。节点上带模型、角色、可见性与写权限。
          </p>
          <p>
            分叉就是在任意节点上 @ 一个模型，从那里长出一条新分支；分支的上下文是根到分叉点的路径，不用再配。同一节点
            @ 多家，就是扇出比对。
          </p>
          <p>
            扇入只有一种节点：验收。它有多个父节点，收「用户原话 + 各父分支的产物」。没有自由连线，节点只能从已有节点分叉，或盖预设展开。
          </p>
        </section>

        <section>
          <h2>上下文怎么算</h2>
          <p>共享：祖先链上的用户消息与模型回复全部带上。</p>
          <p>盲：只留用户消息与附件，祖先链上的模型回复全部剔除。</p>
          <p>指定上游：只留指定上游节点的用户消息与模型回复，加本节点自己的消息。</p>
          <p>上下文取全部祖先（沿所有父链回溯）加自身；验收节点的直接父节点回复始终保留，验收要看产物。</p>
        </section>

        <section>
          <h2>内置预设</h2>
          <p>
            <strong>共享模式</strong>：一段一节点，角色执行，上下文共享。默认的对话方式：在图上接着聊，全部可见。
          </p>
          <p>
            <strong>经济模式</strong>：出方案（高档、thinking 高）→ 执行（低档、thinking 低）→ 验收阶段两路并行：验收（盲、只读，看产物给 PASS / NEEDS_FIX）与验证（盲审同一份产物）。派盲审前由执行节点跑一次验证命令并附上输出。修正轮只看验收那一路，到「修正轮上限」后未闭合的条目生成遗留节点。
          </p>
          <p>
            <strong>盲审模式</strong>：一段三路，盲、只读：一路验收看产物给 PASS / NEEDS_FIX，两路验证盲审同一份材料。三路都要在确认派发页选择目标；派盲审前由执行节点跑一次验证命令并附上输出，互不等待，结果并排看。验证角色未指定目标时，除盲审模式与评审模式外，沿用验收角色的默认目标。
          </p>
          <p>
            <strong>擂台模式</strong>：同一材料发给三家执行，共享上下文，结果并排，选定一路继续。
          </p>
          <p>
            <strong>评审模式</strong>：三个盲验证节点，分别从安全、性能与自定义视角审查同一份材料。
          </p>
          <p>
            <strong>蜂群模式</strong>：拆分节点把任务切成几件，各件在自己的 worktree 里执行，做完按件合并。
          </p>
        </section>

        <section>
          <h2>预设是图章</h2>
          <p>
            预设不是固定流程，而是盖在高亮节点上的一个子图。盖完每个节点都是普通节点：可以单独改模型与开关，也可以再往下分叉、再盖预设。
          </p>
        </section>
      </div>
    </div>,
    document.body,
  );
}
