<script lang="ts">
    import { onMount } from "svelte";
    let parentDiv: HTMLDivElement;
    let canvasElement: HTMLCanvasElement;

    onMount(() => {
        canvasElement.id = "bevy-canvas";
    });
</script>

<canvas id="bevy-canvas" class="w-100 h-100" bind:this={canvasElement}/>
