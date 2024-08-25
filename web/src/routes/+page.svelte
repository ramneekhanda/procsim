<script lang="ts">
    import Monaco from "./Monaco.svelte";
    import Canvas from "./Canvas.svelte";
    import { Pane, Splitpanes } from "svelte-splitpanes";
    import Menubar from "$lib/components/menubar/menubar.svelte";
    import { compile_code } from './dsa.js';

    let codeEditor;

    function compileCode() {
        let b = compile_code(codeEditor.getCode());
        console.log(b.errorLog);
    }
</script>

<div class="flex flex-col h-full">
    <div class="flex-initial">
        <Menubar on:runClicked={() => compileCode()}/>
    </div>

    <Splitpanes class="flex-auto" horizontal={false}>
        <Pane minSize={15}>
            <Canvas />
        </Pane>
        <Pane>
            <div class="flex flex-col h-full">
                <Monaco bind:this={codeEditor}/>
            </div>
        </Pane>
    </Splitpanes>
</div>
