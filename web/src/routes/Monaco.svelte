<script lang="ts">
  import "../app.css";
  import type monaco from "monaco-editor";

  import { configureMonacoYaml } from "monaco-yaml";

  import { onMount } from "svelte";

  import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
  import YamlWorker from "./monaco_yaml.worker.js?worker";

  let Monaco: typeof monaco;
  export let schema = "";
  let editorElement: HTMLDivElement;
  let editor: monaco.editor.IStandaloneCodeEditor;

  function configureMonaco() {
    if (typeof Monaco !== "undefined" && schema !== "") {
      configureMonacoYaml(Monaco, {
        enableSchemaRequest: true,
        completion: true,
        schemas: [
          {
            fileMatch: ["*.yaml"],
            schema: JSON.parse(schema),
            uri: "http://google.com/",
          },
        ],
      });
    }
  }
  $: if (schema !== "") {
    configureMonaco();
  }

  export let value: string = "";

  export function getCode() {
    return editor.getValue();
  }

  onMount(async () => {
    // @ts-ignore
    window.MonacoEnvironment = {
      getWorker: function (_moduleId: any, label: string) {
        switch (label) {
          case "editorWorkerService":
            return new EditorWorker();
          case "yaml":
            let worker = new YamlWorker();
            return worker;
          default:
            throw new Error(`Unknown label ${label}`);
        }
      },
    };

    Monaco = await import("monaco-editor");

    editor = Monaco.editor.create(editorElement, {
      minimap: { enabled: false },
      automaticLayout: true,
      scrollBeyondLastLine: false,
      quickSuggestions: {
        other: true,
        comments: false,
        strings: true,
      },
    });
    configureMonaco();
    let yamlModel = Monaco.editor.createModel(
      value,
      "yaml",
      Monaco.Uri.parse("inmemory://mymodel.yaml"),
    );
    editor.setModel(yamlModel);

    return () => {
      editor.dispose();
    };
  });
</script>

<div class="monaco-container flex h-fit" bind:this={editorElement} />
