<script lang="ts">
    import { onMount } from "svelte";
    import init from './dsa.js';

    let parentDiv: HTMLDivElement;
    let canvasElement: HTMLCanvasElement;

    onMount(() => {
        canvasElement.id = "bevy-canvas";

        init().catch((error) => {
            if (!error.message.startsWith("Using exceptions for control flow, don't mind me. This isn't actually an error!")) {
                throw error;
            }
        });
    });
</script>

<canvas id="bevy-canvas" class="d-block w-100 h-100" bind:this={canvasElement}/>
