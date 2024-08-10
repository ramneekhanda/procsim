<script lang="ts">
    import { onMount } from "svelte";
    let canvasElement: HTMLCanvasElement
    onMount(() => {
    });
</script>

<canvas class='flex' bind:this={canvasElement}/>
