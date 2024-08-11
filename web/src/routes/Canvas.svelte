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
    let w, h;

</script>

<div id="canvas-parent" class='flex' bind:clientWidth={w} bind:clientHeight={h}>
    <canvas height={h} width={w} id="bevy-canvas" bind:this={canvasElement}/>
</div>

<!-- <style>
    #bevy-canvas {
        height: 100%;
        width: 1;
    }
</style> -->