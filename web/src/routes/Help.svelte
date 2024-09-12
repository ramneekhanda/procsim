<script>
  import { Carta, Markdown } from "carta-md";
  import { code } from '@cartamd/plugin-code';

  const carta = new Carta({
  	extensions: [code()]
  });

  export let value;
  let x = fetch(`markdown/API.md`);
  x.then((response) => response.text()).then((data) => {
    value = data;
    console.log(value);
  }).catch((error) => {
    console.error("Error:", error);
  });
</script>

<div class="mkdown">
{#key value}
  <Markdown {carta} {value} />
{/key}
</div>
<!-- Convert the markdown to HTML and display it -->

<!-- Make it look (slightly) nicer ;) -->
<style>

  :global(body) {
    padding: 0;
  }
</style>
