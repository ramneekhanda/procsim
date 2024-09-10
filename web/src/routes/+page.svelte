<script lang="ts">
  import Monaco from "./Monaco.svelte";
  import Menubar from "$lib/components/menubar/menubar.svelte";
  import init, { compile_code, get_code_schema } from "./dsa";
  import { onMount } from "svelte";
  import "dockview-core/dist/styles/dockview.css";
  import "tabulator-tables/dist/css/tabulator.min.css";
  import * as Panels from "./panels";

  let id = 0;
  let code: string;
  let codeEditor: Monaco;
  let schema = "";
  let dockView: HTMLElement;
  let log_event_listener: HTMLDivElement;
  let data: Array<Panels.LogMessageType> = [];
  $: {
    console.log("code changed");
    code = code;
    schema = schema;
    if (codeEditor) {
      codeEditor.$set({ schema });
      codeEditor.setCode(code);
      codeEditor.setFocus();
    }
    code = '';
  }

  function onLogEvent(e: Event) {
    var customEvent = e as CustomEvent;
    let more_data = {} as Panels.LogMessageType;
    more_data.time = new Date().toLocaleTimeString();
    more_data.message = customEvent.detail;
    more_data.severity = "info";
    more_data.node = "dsa";
    data.unshift(more_data);
  }

  onMount(async () => {
    let returnVal = {} as Panels.DockviewReturn;
    Panels.createDockviewInternal(dockView, schema, data, returnVal);
    codeEditor = returnVal.codeEditor;
    log_event_listener.addEventListener("dsa-log-event", onLogEvent);
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

  async function getExampleFiles(filename: string) {
    return await fetch(`examples/${filename}`);
  }
  function exampleClicked(i: Object) {
    if (i.detail.filename) 
      getExampleFiles(i.detail.filename)
        .then((response) => response.text())
        .then((data) => {
          code = data;
        })
        .catch((error) => {
          console.error("Error:", error);
        });
  }
</script>

<div class="flex">
  <Menubar
    on:runClicked={() => compileCode()}
    on:exampleClicked={(i) => exampleClicked(i)}
  />
  <div class="flex" bind:this={dockView}></div>
</div>
<div id="dsa-log-event-listener" bind:this={log_event_listener} />
