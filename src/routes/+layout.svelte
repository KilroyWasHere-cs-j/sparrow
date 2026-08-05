<script>
  let { children } = $props();
</script>

<nav>
  <a href="/">Home</a>
  <a href="/ft8">FT8</a>
  <a href="/aprs">APRS</a>
  <a href="/settings">Settings</a>
</nav>

{@render children()}

<style>
  nav {
    display: flex;
    gap: 1.5em;
    justify-content: center;
    padding: 1em;
    border-bottom: 1px solid #ddd;
  }

  nav a {
    font-weight: 500;
  }

  @media (prefers-color-scheme: dark) {
    nav {
      border-bottom-color: #444;
    }
  }
</style>
