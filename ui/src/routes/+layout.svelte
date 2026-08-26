<script lang="ts">
  import "../app.css";
  import PwaStatus from "$lib/PwaStatus.svelte";

  let { children } = $props();
</script>

{@render children()}
<PwaStatus />
