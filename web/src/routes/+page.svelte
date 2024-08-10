<script>
  import Monaco from './Monaco.svelte';
  import Canvas from './Canvas.svelte';
  import {Pane, Splitpanes} from 'svelte-splitpanes';
</script>
<Splitpanes class="default-theme flex" horizontal="{true}">
    <Pane minSize={15}><Canvas /></Pane>
    <Pane><Monaco /></Pane>
</Splitpanes>

