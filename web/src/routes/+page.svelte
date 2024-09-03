<script lang="ts">
  import Monaco from "./Monaco.svelte";
  import Canvas from "./Canvas.svelte";
  import { Pane, Splitpanes } from "svelte-splitpanes";
  import Menubar from "$lib/components/menubar/menubar.svelte";
  import init, { compile_code, get_code_schema } from "./dsa";
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

<div class="flex">
  <Menubar on:runClicked={() => compileCode()} />
  <Splitpanes horizontal={false}>
    <Pane >
      <Canvas/>
    </Pane>
    <Pane>
      <Monaco bind:this={codeEditor} {schema} />
    </Pane>
  </Splitpanes>
</div>
