<script>
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";

  // Every radio setting. Units match what the controls show (MHz, dB, percent).
  let settings = $state({
    deviceIndex: 0, // which dongle (0 = first); needs a restart to change
    frequencyMhz: 144.39,
    sampleRate: 240000, // needs a restart to change
    mode: "nfm", // "wfm" | "nfm" | "am" | "usb" | "lsb" | "cw"
    autoGain: true, // true = the dongle picks the gain
    gainDb: 30, // manual gain, used when autoGain is off
    ppm: 0,
    volumePercent: 20,
    squelchPercent: 0,
  });

  // The settings in the shape the backend expects (gainDb is null for automatic gain).
  function backendSettings() {
    return {
      deviceIndex: settings.deviceIndex,
      frequencyMhz: settings.frequencyMhz,
      sampleRate: settings.sampleRate,
      mode: settings.mode,
      gainDb: settings.autoGain ? null : settings.gainDb,
      ppm: settings.ppm,
      volumePercent: settings.volumePercent,
      squelchPercent: settings.squelchPercent,
    };
  }

  let name = $state("");
  let greetMsg = $state("");
  let radioError = $state(""); // set if the backend call fails
  let radioStarting = $state(false); // true while the start request is in flight
  let radioRunning = $state(false); // true from a successful start until a successful stop
  let radioStopping = $state(false); // true while the stop request is in flight

  // Asks the backend to run start_radio from the sparrow library.
  async function startRadio() {
    // Ignore clicks that sneak in before the button has re-rendered as disabled.
    if (radioStarting || radioRunning) return;

    radioError = "";
    radioStarting = true;
    try {
      await invoke("start_radio", { settings: backendSettings() });
      radioRunning = true;
    } catch (e) {
      // Failed to start: the button comes back so it can be tried again.
      radioError = String(e);
    } finally {
      radioStarting = false;
    }
  }

  // Sends the current settings to the running radio. Called when a control is committed
  // (not on every keystroke or slider step). Does nothing while the radio is stopped: the
  // settings are sent when it starts.
  let updateCount = 0; // numbers each update, so a slow older one can't overwrite a newer result
  async function applyChange() {
    // Let the control finish updating `settings` before reading it.
    await tick();
    if (!radioRunning) return;

    const thisUpdate = ++updateCount;
    try {
      await invoke("update_radio", { settings: backendSettings() });
      if (thisUpdate === updateCount) radioError = "";
    } catch (e) {
      if (thisUpdate === updateCount) radioError = String(e);
    }
  }

  // Asks the backend to run stop_radio from the sparrow library.
  async function stopRadio() {
    // Ignore clicks that sneak in before the button has re-rendered as disabled.
    if (!radioRunning || radioStopping) return;

    radioError = "";
    radioStopping = true;
    try {
      await invoke("stop_radio");
      radioRunning = false;
    } catch (e) {
      // Failed to stop: the radio is still running, so the button comes back to try again.
      radioError = String(e);
    } finally {
      radioStopping = false;
    }
  }

  /** @param {Event} event */
  async function greet(event) {
    event.preventDefault();
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    greetMsg = await invoke("greet", { name });
  }
</script>

<main class="container">
  <h1>Welcome to Sparrow</h1>

  <form class="row" onsubmit={greet}>
    <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
    <button type="submit">Greet</button>
  </form>
  <p>{greetMsg}</p>

  <!-- Radio settings: all but the last two change live while the radio runs -->
  <section>
    <h2>Radio</h2>

    <label>
      Frequency (MHz)
      <input type="number" step="0.001" bind:value={settings.frequencyMhz} onchange={applyChange} />
    </label>

    <label>
      Mode
      <select bind:value={settings.mode} onchange={applyChange}>
        <option value="wfm">Wideband FM (broadcast)</option>
        <option value="nfm">Narrowband FM</option>
        <option value="am">AM</option>
        <option value="usb">USB</option>
        <option value="lsb">LSB</option>
        <option value="cw">CW</option>
      </select>
    </label>

    <label>
      <input type="checkbox" bind:checked={settings.autoGain} onchange={applyChange} />
      Automatic gain
    </label>

    <label>
      Gain (dB)
      <input
        type="number"
        step="0.1"
        bind:value={settings.gainDb}
        disabled={settings.autoGain}
        onchange={applyChange}
      />
    </label>

    <label>
      Frequency correction (ppm)
      <input type="number" step="1" bind:value={settings.ppm} onchange={applyChange} />
    </label>

    <label>
      Volume
      <input type="range" min="0" max="100" bind:value={settings.volumePercent} onchange={applyChange} />
      <output>{settings.volumePercent}%</output>
    </label>

    <label>
      Squelch
      <input type="range" min="0" max="100" bind:value={settings.squelchPercent} onchange={applyChange} />
      <output>{settings.squelchPercent}%</output>
    </label>
    <p>Mode and squelch are accepted but not applied to the audio yet.</p>

    <!-- These two are fixed while the radio runs; stop it to change them -->
    <label>
      Sample rate (Hz)
      <input type="number" step="1000" bind:value={settings.sampleRate} disabled={radioRunning} />
    </label>

    <label>
      Device number
      <input type="number" min="0" step="1" bind:value={settings.deviceIndex} disabled={radioRunning} />
    </label>
  </section>

  <div class="row">
    <button onclick={startRadio} disabled={radioStarting || radioRunning}>
      {radioRunning ? "Radio running" : radioStarting ? "Starting..." : "Start radio"}
    </button>
    <button onclick={stopRadio} disabled={!radioRunning || radioStopping}>
      {radioStopping ? "Stopping..." : "Stop radio"}
    </button>
  </div>
  {#if radioError}
    <p>{radioError}</p>
  {/if}
</main>

<style>
.logo.vite:hover {
  filter: drop-shadow(0 0 2em #747bff);
}

.logo.svelte-kit:hover {
  filter: drop-shadow(0 0 2em #ff3e00);
}

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: #535bf2;
}

h1 {
  text-align: center;
}

input,
button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #0f0f0f;
  background-color: #ffffff;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button {
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}
button:active {
  border-color: #396cd8;
  background-color: #e8e8e8;
}

/* Grayed out and unclickable (e.g. Start radio while the radio is starting or running). */
button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  border-color: transparent;
  background-color: #ffffff;
}

input,
button {
  outline: none;
}

#greet-input {
  margin-right: 5px;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  input,
  button {
    color: #ffffff;
    background-color: #0f0f0f98;
  }
  button:active {
    background-color: #0f0f0f69;
  }
}

</style>
