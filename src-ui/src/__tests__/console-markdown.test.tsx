import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { ConsoleMarkdown } from '../components/center/ConsoleMarkdown';

describe('控制台回复按 Markdown 渲染', () => {
  it('标题、粗体、表格、代码都渲染成元素，不再显示原文符号', () => {
    const html = renderToStaticMarkup(
      <ConsoleMarkdown text={'### 结论\n\n**通过**，见 `alpha.py`\n\n| 测试 | 结果 |\n|---|---|\n| add | 通过 |'} />,
    );
    expect(html).toContain('<h3>结论</h3>');
    expect(html).toContain('<strong>通过</strong>');
    expect(html).toContain('<code>alpha.py</code>');
    expect(html).toContain('<table>');
    expect(html).not.toContain('###');
    expect(html).not.toContain('|---|');
  });

  it('链接不可点走、图片不加载、原始 HTML 不执行', () => {
    const html = renderToStaticMarkup(
      <ConsoleMarkdown text={'[官网](https://example.com) ![图](https://example.com/a.png)\n\n<script>alert(1)</script>'} />,
    );
    expect(html).not.toContain('<a ');
    expect(html).toContain('title="https://example.com"');
    expect(html).not.toContain('<img');
    expect(html).not.toContain('<script');
  });
});
