import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';

// 模型回复按 Markdown 渲染（标题、粗体、列表、表格、代码）。不开 raw HTML。
// 链接只显示文字、悬停看地址：在 Tauri 窗口里点链接会把整个应用导航走。图片不加载远程资源，只显示替代文字。
const components: Components = {
  a: ({ href, children }) => <span className="console-md-link" title={href}>{children}</span>,
  img: ({ alt }) => <span className="console-md-image">[图片{alt ? `：${alt}` : ''}]</span>,
  table: ({ children }) => (
    <div className="console-md-table">
      <table>{children}</table>
    </div>
  ),
};

export function ConsoleMarkdown({ text }: { text: string }) {
  return (
    <div className="console-message-text console-markdown">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>{text}</ReactMarkdown>
    </div>
  );
}
