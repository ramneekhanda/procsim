<script lang=ts>
// @ts-nocheck

 import { createEventDispatcher } from "svelte";
 const dispatch = createEventDispatcher();
</script>


<div class="navbar min-h-10">
  <div class="flex-1">
    <img src="favicon.png" alt="icon" width="32px"/>
  </div>
  <div class="navbar-center">
    <button class="btn btn-sm" on:click={() => dispatch('runClicked')}>
      <svg version="1.1" id="Layer_1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
	         width="16px" height="16px" viewBox="0 0 64 64" enable-background="new 0 0 64 64" xml:space="preserve">
        <g>
	        <polygon fill="none" stroke="#000000" stroke-width="2" stroke-linejoin="bevel" stroke-miterlimit="10" points="27,21 41,32
		                     27,43 	"/>
	        <path fill="none" stroke="#000000" stroke-width="2" stroke-miterlimit="10" d="M53.92,10.081
		                  c12.107,12.105,12.107,31.732,0,43.838c-12.106,12.108-31.734,12.108-43.839,0c-12.107-12.105-12.107-31.732,0-43.838
		                  C22.186-2.027,41.813-2.027,53.92,10.081z"/>
        </g>
      </svg>
      Run
    </button>
  </div>
  <div class="navbar-end">
    
    <div class="dropdown dropdown-end">
      <div tabindex="0" role="button" class="btn btn-sm m-1">Examples</div>
    <!-- svelte-ignore a11y-no-noninteractive-tabindex -->
    <ul tabindex="0" class="dropdown-content menu bg-base-100 rounded-box z-[1] w-80 p-2 shadow">
      <li>
        <details open>
          <summary>Basics</summary>
          <ul>
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <!-- svelte-ignore a11y-missing-attribute -->
            <li><a on:click={()=>dispatch('exampleClicked', {id: 1, filename: "lifecycle.yml"})}>Node Lifecycle</a></li>
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <!-- svelte-ignore a11y-missing-attribute -->
            <li><a on:click={()=>dispatch('exampleClicked', {id: 1, filename: "pingpong.yml"})}>Ping Pong</a></li>
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <!-- svelte-ignore a11y-missing-attribute -->
            <li><a on:click={()=>dispatch('exampleClicked', {id: 1, filename: "multiclient_pingpong.yml"})}>Multiclient Ping Pong</a></li>
          </ul>
        </details>
      </li>
    </ul>
    </div>
  </div>

</div>

      