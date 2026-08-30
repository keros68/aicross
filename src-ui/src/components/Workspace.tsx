import { useEffect, useState } from "react";
import { api } from "../api";
import type { FileNode } from "../types";

export function Workspace({ projectPath }: { projectPath: string }) {
  const [nodes, setNodes] = useState<FileNode[]>([]);
  const [selected, setSelected] = useState<FileNode | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    setSelected(null);
    setNodes([]);
    api.listDirectory("")
      .then((value) => {
        setNodes(value);
        setError("");
      })
      .catch((value) => setError(String(value)));
  }, [projectPath]);

  return (
    <div className="pane-body workspace-pane" key={projectPath}>
      <div className="tree" role="tree">
        {nodes.map((node) => (
          <TreeNode key={node.relativePath} node={node} onSelect={setSelected} onError={setError} />
        ))}
      </div>
      {selected && !selected.isDir && (
        <div className="file-actions">
          <div className="file-name">{selected.name}</div>
          <div className="muted-text mono path-wrap">{selected.absolutePath}</div>
          <div className="action-row">
            <button
              className="button small"
              onClick={() => navigator.clipboard.writeText(selected.absolutePath)}
            >
              复制路径
            </button>
            <button
              className="button small"
              onClick={() => api.openFile(selected.relativePath).then(() => setError("")).catch((value) => setError(String(value)))}
            >
              打开
            </button>
          </div>
        </div>
      )}
      {error && <div className="error-line">{error}</div>}
    </div>
  );
}

function TreeNode({
  node,
  onSelect,
  onError,
}: {
  node: FileNode;
  onSelect: (node: FileNode) => void;
  onError: (message: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<FileNode[] | null>(null);

  const activate = async () => {
    onSelect(node);
    if (!node.isDir) return;
    if (!expanded && children === null) {
      try {
        setChildren(await api.listDirectory(node.relativePath));
        onError("");
      } catch (value) {
        onError(String(value));
        return;
      }
    }
    setExpanded((value) => !value);
  };

  return (
    <div role="treeitem" aria-expanded={node.isDir ? expanded : undefined}>
      <button className={`tree-row ${node.muted ? "muted-node" : ""}`} onClick={activate}>
        <span className="tree-caret">{node.isDir ? (expanded ? "▾" : "▸") : ""}</span>
        <span className="tree-kind">{node.isSymlink ? "↗" : node.isDir ? "□" : "·"}</span>
        <span className="tree-label">{node.name}</span>
      </button>
      {expanded && children && (
        <div className="tree-children" role="group">
          {children.map((child) => (
            <TreeNode key={child.relativePath} node={child} onSelect={onSelect} onError={onError} />
          ))}
        </div>
      )}
    </div>
  );
}
