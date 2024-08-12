<script>
  import Monaco from './Monaco.svelte';
  import Canvas from './Canvas.svelte';
  import {Pane, Splitpanes} from 'svelte-splitpanes';
  import Menubar from '$lib/components/menubar/menubar.svelte'; 
  import Editorbar from '$lib/components/menubar/editorbar.svelte'; 
</script>

<Menubar />
<Splitpanes horizontal="{true}">
    <Pane minSize={15}>
        <Canvas />
      </Pane>
    <Pane>
        <Editorbar />
        <Monaco />
      </Pane>
</Splitpanes>
