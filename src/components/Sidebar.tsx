import { ALL_TOOLS } from "../lib/params";
import type { ToolId } from "../lib/types";

export type DialogView = "about" | "license" | "network" | "paywall" | "selfcheck";

interface SidebarProps {
  active: ToolId;
  onSelect: (tool: ToolId) => void;
  onOpen: (view: DialogView) => void;
  /** 收起左侧工具栏，把空间让给文档画布（顶栏「工具」开关可再展开）。 */
  onCollapse?: () => void;
}

const FOOTER_ITEMS: { view: DialogView; label: string }[] = [
  { view: "about", label: "关于" },
  { view: "selfcheck", label: "运行自检" },
  { view: "license", label: "授权" },
  { view: "network", label: "网络活动" },
];

export function Sidebar({ active, onSelect, onOpen, onCollapse }: SidebarProps) {
  return (
    <nav className="sidebar" aria-label="功能导航">
      <div className="brand">
        <div className="brand-row">
          <span className="brand-name">PayOnce PDF</span>
          {onCollapse ? (
            <button
              type="button"
              className="panel-collapse"
              aria-label="收起左侧工具栏"
              title="收起工具栏，给文档腾地方（顶栏「工具」可再展开）"
              onClick={onCollapse}
            >‹</button>
          ) : null}
        </div>
        <span className="brand-sub">一次付费 PDF</span>
      </div>

      <ul className="tool-list">
        {ALL_TOOLS.map((tool) => (
          <li key={tool.id}>
            <button
              type="button"
              className={`tool-btn ${active === tool.id ? "is-active" : ""}`}
              aria-current={active === tool.id ? "page" : undefined}
              title={tool.hint}
              onClick={() => onSelect(tool.id)}
            >
              <span className="tool-btn-label">{tool.label}</span>
              <span className="tool-btn-hint">{tool.hint}</span>
            </button>
          </li>
        ))}
      </ul>

      <div className="sidebar-footer">
        <span className="footer-heading">信息</span>
        {FOOTER_ITEMS.map((item) => (
          <button
            type="button"
            key={item.view}
            className="footer-btn"
            onClick={() => onOpen(item.view)}
          >
            {item.label}
          </button>
        ))}
      </div>
    </nav>
  );
}
