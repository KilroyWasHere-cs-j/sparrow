<script>
  import { page } from "$app/state";

  let { children } = $props();

  const links = [
    { href: "/", label: "Home" },
    { href: "/ft8", label: "FT8" },
    { href: "/aprs", label: "APRS" },
    { href: "/settings", label: "Settings" },
  ];
</script>

<nav>
  {#each links as link}
    <a href={link.href} aria-current={page.url.pathname === link.href ? "page" : undefined}>
      {link.label}
    </a>
  {/each}
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

  nav a[aria-current="page"] {
    color: #396cd8;
    text-decoration: underline;
  }

  @media (prefers-color-scheme: dark) {
    nav {
      border-bottom-color: #444;
    }

    nav a[aria-current="page"] {
      color: #24c8db;
    }
  }
</style>
