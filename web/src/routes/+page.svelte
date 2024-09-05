<script lang="ts">
  import Monaco from "./Monaco.svelte";
  import Canvas from "./Canvas.svelte";
  import Menubar from "$lib/components/menubar/menubar.svelte";
  import init, { compile_code, get_code_schema } from "./dsa";
  import { onMount, tick } from "svelte";
  import "dockview-core/dist/styles/dockview.css";
  import { createDockview } from "dockview-core";
  import "dockview-core/dist/styles/dockview.css";
  import {TabulatorFull as Tabulator} from 'tabulator-tables';
  import  "tabulator-tables/dist/css/tabulator.min.css";

  interface LogMessageType {
    time: string;
    message: string;
    severity: string;
    node: string;
  }

  import type {
    GroupPanelPartInitParameters,
    IContentRenderer,
    IDockviewPanel,
    ITabRenderer,
  } from "dockview-core";
  import type Tabular from "gridjs/dist/src/tabular.js";

  const columns = [
	 	{title:"time", field:"time"},
	 	{title:"message", field:"message"},
	 	{title:"severity", field:"severity"},
    {title: "node", field: "node"}
  ];
  let id = 0;
  let codeEditor: Monaco;
  let schema = "";
  let grid: Tabular;
  let dockView: HTMLElement;
  let log_event_listener: HTMLDivElement;

  let data: Array<LogMessageType> = [];

  $: codeEditor && codeEditor.$set({ schema });

  class MonacoPanel implements IContentRenderer {
    private readonly _element: HTMLElement;

    get element(): HTMLElement {
      return this._element;
    }

    constructor() {
      const MonacoDiv = document.createElement("div");
      MonacoDiv.style.width = "100%";
      MonacoDiv.style.height = "100%";
      let monaco = new Monaco({ target: MonacoDiv, props: { schema } });
      codeEditor = monaco;
      this._element = MonacoDiv;
    }

    init(parameters: GroupPanelPartInitParameters): void {}
  }

  class LogPanel implements IContentRenderer {
    private readonly _element: HTMLElement;

    get element(): HTMLElement {
      return this._element;
    }

    constructor() {
      const logsDiv = document.createElement("div");
      const mygrid = document.getElementById("myrevo");
      grid = new Tabulator(
        logsDiv,
        {
          columns: columns,
          data: data,
          reactiveData:true,
          layout: "fitColumns",
          height: "100%",
          width: "100%",
        }
      );
      this._element = logsDiv;
    }

    init(parameters: GroupPanelPartInitParameters): void {}
  }

  class Tab implements ITabRenderer {
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

  class ViewPanel implements IContentRenderer {
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

    init(parameters: GroupPanelPartInitParameters): void {}
  }

  onMount(async () => {
    const api = createDockview(dockView, {
      className: "dockview-theme-light",
      createComponent: (options) => {
        switch (options.name) {
          case "MonacoPanel":
            return new MonacoPanel();
          case "ViewPanel":
            return new ViewPanel();
          case "LogPanel":
            return new LogPanel();
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
    api.panels[1].focus();
    log_event_listener.addEventListener("dsa-log-event", (e: CustomEvent) => {
      let more_data = {} as LogMessageType;
      more_data.time = new Date().toLocaleTimeString();
      more_data.message = e.detail;
      more_data.severity = "info";
      more_data.node = "dsa";
      data.push(more_data);
    });
    init()
      .catch((error) => {
        if (
          !error.message.startsWith(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
          )
        ) {
          console.log("actual error location is here");
          throw error;
        }
      })
      .then(() => {
        schema = get_code_schema();
      });
  });

  function compileCode() {
    let b = compile_code(codeEditor.getCode());
    console.log(b.error_log);
  }
</script>

<div class="flex">
  <Menubar on:runClicked={() => compileCode()} />
  <div class="flex" bind:this={dockView}></div>
</div>
<div id="dsa-log-event-listener" bind:this={log_event_listener} />
