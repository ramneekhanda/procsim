<script lang="ts">
  import Monaco from "./Monaco.svelte";
  import Canvas from "./Canvas.svelte";
  import { Pane, Splitpanes } from "svelte-splitpanes";
  import Menubar from "$lib/components/menubar/menubar.svelte";
  import { createSwapy } from "swapy";
  import init, { compile_code, get_code_schema } from "./dsa.js";
  import { onMount } from "svelte";

  let codeEditor;
  let schema = "";

  onMount(async () => {
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
        console.log("got schema!");
      });
  });
  function compileCode() {
    let b = compile_code(codeEditor.getCode());
    console.log(b.error_log);
  }
</script>

<div class="flex flex-col h-full">
  <div class="flex-initial">
    <Menubar on:runClicked={() => compileCode()} />
  </div>

  <Splitpanes class="flex-auto" horizontal={false}>
    <Pane minSize={15}>
      <Canvas />
    </Pane>
    <Pane>
      <div class="flex flex-col h-full">
        <Monaco bind:this={codeEditor} {schema} />
      </div>
    </Pane>
  </Splitpanes>
</div>
