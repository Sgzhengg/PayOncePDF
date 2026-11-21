import type { ReactNode } from "react";
import { ALL_TOOLS } from "../lib/params";
import type { ParamsState } from "../lib/params";
import type { ToolId } from "../lib/types";
import { MergePanel } from "../panels/MergePanel";
import { SplitPanel } from "../panels/SplitPanel";
import { CompressPanel } from "../panels/CompressPanel";
import { ConvertPanel } from "../panels/ConvertPanel";
import { OcrPanel } from "../panels/OcrPanel";
import { SignPanel } from "../panels/SignPanel";
import { BatchPanel } from "../panels/BatchPanel";
import {
  AnnotatePanel,
  DecoratePanel,
  EditPanel,
  ExportPanel,
  PagesPanel,
} from "../panels/ExtrasPanels";
import { CreatePanel, ObjectsPanel, SharePanel } from "../panels/MorePanels";
import { ComparePanel } from "../panels/ComparePanel";

export type ParamOnChange = {
  [K in ToolId]: (patch: Partial<ParamsState[K]>) => void;
};

interface ParamPanelProps {
  tool: ToolId;
  params: ParamsState;
  onChange: ParamOnChange;
  fileCount: number;
  fileNames: string[];
  filePaths: string[];
  onSortByName: () => void;
  /** 收起右侧参数面板，把空间让给文档画布（顶栏「参数」开关可再展开）。 */
  onCollapse?: () => void;
}

export function ParamPanel({
  tool,
  params,
  onChange,
  fileCount,
  fileNames,
  filePaths,
  onSortByName,
  onCollapse,
}: ParamPanelProps) {
  const meta = ALL_TOOLS.find((t) => t.id === tool) ?? ALL_TOOLS[0];
  let body: ReactNode = null;
  switch (tool) {
    case "merge":
      body = (
        <MergePanel
          value={params.merge}
          onChange={onChange.merge}
          fileCount={fileCount}
          fileNames={fileNames}
          onSortByName={onSortByName}
        />
      );
      break;
    case "split":
      body = <SplitPanel value={params.split} onChange={onChange.split} fileCount={fileCount} />;
      break;
    case "compress":
      body = (
        <CompressPanel value={params.compress} onChange={onChange.compress} fileCount={fileCount} />
      );
      break;
    case "convert":
      body = (
        <ConvertPanel value={params.convert} onChange={onChange.convert} fileCount={fileCount} />
      );
      break;
    case "ocr":
      body = <OcrPanel value={params.ocr} onChange={onChange.ocr} fileCount={fileCount} />;
      break;
    case "sign":
      body = <SignPanel value={params.sign} onChange={onChange.sign} fileCount={fileCount} />;
      break;
    case "batch":
      body = <BatchPanel value={params.batch} onChange={onChange.batch} fileCount={fileCount} />;
      break;
    case "edit":
      body = <EditPanel value={params.edit} onChange={onChange.edit} fileCount={fileCount} />;
      break;
    case "annotate":
      body = (
        <AnnotatePanel
          value={params.annotate}
          onChange={onChange.annotate}
          fileCount={fileCount}
        />
      );
      break;
    case "pages":
      body = <PagesPanel value={params.pages} onChange={onChange.pages} fileCount={fileCount} />;
      break;
    case "decorate":
      body = (
        <DecoratePanel
          value={params.decorate}
          onChange={onChange.decorate}
          fileCount={fileCount}
        />
      );
      break;
    case "export":
      body = <ExportPanel value={params.export} onChange={onChange.export} fileCount={fileCount} />;
      break;
    case "objects":
      body = (
        <ObjectsPanel
          value={params.objects}
          onChange={onChange.objects}
          fileCount={fileCount}
          filePaths={filePaths}
        />
      );
      break;
    case "compare":
      body = (
        <ComparePanel
          value={params.compare}
          onChange={onChange.compare}
          filePaths={filePaths}
        />
      );
      break;
    case "create":
      body = <CreatePanel value={params.create} onChange={onChange.create} fileCount={fileCount} />;
      break;
    case "share":
      body = <SharePanel value={params.share} onChange={onChange.share} fileCount={fileCount} />;
      break;
  }
  return (
    <aside className="params" aria-label="参数面板">
      <header className="params-header">
        <div className="params-heading">
          <h2>{meta.label}</h2>
          <p>{meta.hint}</p>
        </div>
        {onCollapse ? (
          <button
            type="button"
            className="panel-collapse"
            aria-label="收起右侧参数面板"
            title="收起参数面板，给文档腾地方（顶栏「参数」可再展开）"
            onClick={onCollapse}
          >›</button>
        ) : null}
      </header>
      <div className="params-body">{body}</div>
    </aside>
  );
}
