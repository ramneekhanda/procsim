<script lang="ts">
    import "../app.css";
    import type monaco from "monaco-editor";

    import { onMount } from "svelte";

    import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
    import yamlWorker from "monaco-yaml/yaml.worker?worker";

    let editorElement: HTMLDivElement;
    let editor: monaco.editor.IStandaloneCodeEditor;
    let Monaco;
    export let value: string = "";

    export function getCode(){
        return editor.getValue()
    }

 onMount(async () => {
     // @ts-ignore
     self.MonacoEnvironment = {
            getWorker: function (_moduleId: any, label: string) {
                if (label === "yaml") {
                    return new yamlWorker();
                }

                return new editorWorker();
            },
        };

        Monaco = await import("monaco-editor");
        editor = Monaco.editor.create(editorElement, {
            value: value,
            language: "yaml",
            minimap: { enabled: false },
            automaticLayout: true,
            scrollBeyondLastLine: false,
        });

        return () => {
            editor.dispose();
        };
    });
</script>

<div class="monaco-container flex z-0 h-fit" bind:this={editorElement} />
