<script>
  import { invoke } from "@tauri-apps/api/core";

  let callsign = $state("");
  let greetMsg = $state("");

  async function greet(event) {
    event.preventDefault();
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    greetMsg = await invoke("greet", { callsign });
  }
</script>

 <main class="container">
  <h1>Settings</h1>
  <p>Framework only. Radio interface config (Digirig audio device, PTT) will live here.</p>
  <form class="row" onsubmit={greet}>
    <input id="greet-input" placeholder="KC1RIZ" bind:value={callsign} />
    <button type="submit">Greet</button>
  </form>
</main>

<style>
  .container {
    padding-top: 4vh;
    text-align: center;
  }
</style>
