<script>
  import Monaco from "./Monaco.svelte";
  import Canvas from "./Canvas.svelte";
  import { Pane, Splitpanes } from "svelte-splitpanes";
  import Menubar from "$lib/components/menubar/menubar.svelte";
  import Editorbar from "$lib/components/menubar/editorbar.svelte";
</script>

<div class="flex-col flex h-full">
<div class="flex-initial">
  <Menubar />
</div>

<Splitpanes class="flex-auto" horizontal={false}>
  <Pane minSize={15}>
    <Canvas />
  </Pane>
  <Pane>
    <div class="flex-col flex h-full">
      <Editorbar />
      <Monaco />
    </div>
  </Pane>
</Splitpanes>
</div>