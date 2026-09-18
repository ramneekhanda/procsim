<script lang=ts>
// @ts-nocheck

 import { createEventDispatcher } from "svelte";
 const dispatch = createEventDispatcher();

 let installLabel = "Install in your harness";

 async function installInHarness() {
   const base = window.location.origin + window.location.pathname.replace(/\/$/, "");
   const prompt = `Install the "procsim-diagram" skill by fetching these files and saving them exactly as-is (preserve the directory structure, SKILL.md at the root of the skill folder):

${base}/skills/procsim-diagram/SKILL.md -> procsim-diagram/SKILL.md
${base}/skills/procsim-diagram/references/schema.md -> procsim-diagram/references/schema.md
${base}/skills/procsim-diagram/references/rhai-handlers.md -> procsim-diagram/references/rhai-handlers.md
${base}/skills/procsim-diagram/references/imports-and-themes.md -> procsim-diagram/references/imports-and-themes.md

Save it wherever your tool or agent looks for skills/instructions/tools (e.g. Claude Code's .claude/skills/, or an equivalent local/project convention for other coding agents), scoped to the current project if I want this available there, otherwise scoped globally/user-wide. This skill helps build procsim graph YAML files (this simulator's node/Rhai-script format) from natural-language descriptions of a system or process.`;

   try {
     await navigator.clipboard.writeText(prompt);
   } catch (e) {
     console.error("Failed to copy install prompt:", e);
   }
   installLabel = "Prompt copied - please paste it in your agent";
   setTimeout(() => (installLabel = "Install in your harness"), 3000);
 }
</script>


<div class="navbar min-h-10">
  <div class="flex-1">
    <img src="favicon.png" alt="icon" width="32px"/>
    <b><h1>Process Simulator</h1></b>
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
    <button class="btn btn-sm" on:click={() => dispatch('fullscreenClicked')} title="Toggle fullscreen view">
      <svg xmlns="http://www.w3.org/2000/svg" width="16px" height="16px" viewBox="0 0 24 24" fill="none" stroke="#000000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="8 3 3 3 3 8"></polyline>
        <polyline points="21 8 21 3 16 3"></polyline>
        <polyline points="16 21 21 21 21 16"></polyline>
        <polyline points="3 16 3 21 8 21"></polyline>
      </svg>
      Fullscreen
    </button>
  </div>
  <div class="navbar-end">
    <button class="btn btn-sm" on:click={installInHarness} title="Copy an install prompt for the procsim-diagram skill">
      {installLabel}
    </button>
  </div>

</div>
