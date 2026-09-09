import Monaco from "./Monaco.svelte";
import Canvas from "./Canvas.svelte";
import Help from "./Help.svelte";

import type {
  GroupPanelPartInitParameters,
  IContentRenderer,
  IDockviewPanel,
  ITabRenderer,
} from "dockview-core";

import { ReactiveDataModule, TabulatorFull as Tabulator } from 'tabulator-tables';
import { createDockview } from "dockview-core";

export interface LogMessageType {
  time: string;
  message: string;
  severity: string;
  node: string;
}

export const columns = [
  { title: "time", field: "time" },
  { title: "message", field: "message" },
  { title: "severity", field: "severity" },
  { title: "node", field: "node" }
];

export class Tab implements ITabRenderer {
  private _element: HTMLElement;
  private _title: string;

  get element(): HTMLElement {
    return this._element;
  }

  constructor() {
    this._element = document.createElement("div");
    this._title = "new tab";
  }

  init(parameters: GroupPanelPartInitParameters): void {
    var text = document.createTextNode(parameters.title);
    this._element.appendChild(text);
  }
}

export class HelpPanel implements IContentRenderer {
  private readonly _element: HTMLElement;

  get element(): HTMLElement {
    return this._element;
  }

  constructor() {
    const MDDiv = document.createElement("div");
    MDDiv.style.width = "100%";
    MDDiv.style.height = "100%";
    let canvas = new Help({ target: MDDiv });
    this._element = MDDiv;
  }

  init(_: GroupPanelPartInitParameters): void {}
}

export class ViewPanel implements IContentRenderer {
  private readonly _element: HTMLElement;

  get element(): HTMLElement {
    return this._element;
  }

  constructor() {
    const CanvasDiv = document.createElement("div");
    CanvasDiv.style.width = "100%";
    CanvasDiv.style.height = "100%";
    let canvas = new Canvas({ target: CanvasDiv });
    this._element = CanvasDiv;
  }

  init(_: GroupPanelPartInitParameters): void {}
}

export class MonacoPanel implements IContentRenderer {
  private readonly _element: HTMLElement;
  monaco: Monaco;

  get element(): HTMLElement {
    return this._element;
  }

  constructor(schema: string) {
    const MonacoDiv = document.createElement("div");
    MonacoDiv.style.width = "100%";
    MonacoDiv.style.height = "100%";
    this.monaco = new Monaco({ target: MonacoDiv, props: { schema } });
    this._element = MonacoDiv;
  }

  init(_: GroupPanelPartInitParameters): void {}
}

export class LogPanel implements IContentRenderer {
  private readonly _element: HTMLElement;

  get element(): HTMLElement {
    return this._element;
  }

  constructor(data: &Array<LogMessageType>) {
    const LogDiv = document.createElement("div");
    LogDiv.style.width = "100%";
    LogDiv.style.height = "100%";
    this._element = LogDiv;
    new Tabulator(LogDiv, {
      columns,
      data,
      reactiveData:true,
      layout: "fitColumns",
      height: "100%",
    });
  }

  init(_: GroupPanelPartInitParameters): void {}
}

export interface DockviewReturn {
  codeEditor: Monaco;
  viewPanel: IDockviewPanel;
};

export function createDockviewInternal(dockView: HTMLElement, schema: string, data: &Array<LogMessageType>, returnVal: DockviewReturn) {
  const api = createDockview(dockView, {
    className: "dockview-theme-light",
    createComponent: (options) => {
      switch (options.name) {
        case "MonacoPanel":
          let monacoPanel = new MonacoPanel(schema);
          returnVal.codeEditor = monacoPanel.monaco;
          return monacoPanel;
        case "ViewPanel":
          return new ViewPanel();
        case "LogPanel":
          return new LogPanel(data);
        case "HelpPanel":
            return new HelpPanel();
        default:
          throw new Error(`Unknown component ${options.name}`);
      }
    },
    createTabComponent: (options) => {
      switch (options.name) {
        case "Tab":
          return new Tab();
        default:
          throw new Error(`Unknown tab component ${options.name}`);
      }
    },
  });

  let viewPanel = api.addPanel({
    id: "view_panel",
    component: "ViewPanel",
    tabComponent: "Tab",
    title: "View",
  });
  returnVal.viewPanel = viewPanel;

  const monacoPanel: IDockviewPanel = api.addPanel({
    id: "monaco_panel",
    component: "MonacoPanel",
    title: "Editor",
    tabComponent: "Tab",
    position: {
      referencePanel: viewPanel,
      direction: "right",
    },
  });

  api.addPanel({
    id: "log_panel",
    component: "LogPanel",
    title: "Logs",
    tabComponent: "Tab",
    position: {
      referencePanel: monacoPanel,
    },
  });

  api.addPanel({
    id: "md_panel",
    component: "HelpPanel",
    title: "Learn",
    tabComponent: "Tab",
    position: {
      referencePanel: monacoPanel,
    },
  });
  api.panels[1].focus();
}


