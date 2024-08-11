<script lang="ts">
    import { onMount } from "svelte";
    import init from './dsa.js';

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

<canvas class='flex' id="bevy-canvas" bind:this={canvasElement}/>
