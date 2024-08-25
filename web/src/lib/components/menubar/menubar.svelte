<script>
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
    <div class="drawer drawer-end navbar-end">
      <input id="my-drawer" type="checkbox" class="drawer-toggle" />
      <div class="drawer-content">
        <!-- Page content here -->
        <label for="my-drawer" class="btn btn-primary btn-sm drawer-button">help</label>
      </div>
      <div class="drawer-side z-50 ">
        <label for="my-drawer" aria-label="close sidebar" class="drawer-overlay"></label>
        <div class="menu bg-base-200 text-base-content min-h-full w-80 p-4">
          <!-- Sidebar content here -->
            <div class="collapse collapse-arrow bg-base-200">
              <input type="radio" name="my-accordion-2" checked="checked" />
              <div class="collapse-title text-xl font-medium">Click to open this one and close others</div>
              <div class="collapse-content">
                <p>hello</p>
              </div>
            </div>
            <div class="collapse collapse-arrow bg-base-200">
              <input type="radio" name="my-accordion-2" />
              <div class="collapse-title text-xl font-medium">Click to open this one and close others</div>
              <div class="collapse-content">
                <p>hello</p>
              </div>
            </div>
            <div class="collapse collapse-arrow bg-base-200">
              <input type="radio" name="my-accordion-2" />
              <div class="collapse-title text-xl font-medium">Click to open this one and close others</div>
              <div class="collapse-content">
                <p>hello</p>
              </div>
            </div>
          </div>
      </div>
    </div>
  </div>

</div>
