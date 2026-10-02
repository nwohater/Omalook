<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app, fail, reloadAccounts } from "$lib/state.svelte";
  import MailView from "$lib/MailView.svelte";
  import CalendarView from "$lib/CalendarView.svelte";
  import ContactsView from "$lib/ContactsView.svelte";
  import Composer from "$lib/Composer.svelte";
  import Settings from "$lib/Settings.svelte";

  // ── Omarchy theme sync ───────────────────────────────────────────
  let lastTheme = "";
  async function syncTheme() {
    const c = await api.themeColors().catch(() => null);
    const key = JSON.stringify(c);
    if (key === lastTheme) return;
    lastTheme = key;
    const root = document.documentElement;
    const map: Record<string, string | undefined> = {
      "--bg": c?.background,
      "--panel": c?.lighter_background ?? c?.dark_background,
      "--fg": c?.foreground,
      "--accent": c?.accent,
      "--sel": c?.selection,
      "--danger": c?.red,
      "--ok": c?.green,
      "--accent-fg": c ? (c.mode === "light" ? "#ffffff" : c.background) : undefined,
    };
    for (const [k, v] of Object.entries(map)) v ? root.style.setProperty(k, v) : root.style.removeProperty(k);
    root.classList.toggle("light", c?.mode === "light");
  }

  onMount(() => {
    (async () => {
      try {
        app.config = await api.getConfig();
        await reloadAccounts();
        if (!app.accounts.length) app.settings = true;
      } catch (e) { fail(e); }
      app.ready = true;
    })();
    syncTheme();
    const t = setInterval(syncTheme, 3000);
    return () => clearInterval(t);
  });

  function onKey(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    if (e.key === "1") app.mode = "mail";
    else if (e.key === "2") app.mode = "calendar";
    else if (e.key === "3") app.mode = "contacts";
    else if (e.key === ",") app.settings = !app.settings || !app.accounts.length;
    else return;
    e.preventDefault();
  }

  const modes = [
    ["mail", "✉", "Mail (Ctrl+1)"],
    ["calendar", "📅", "Calendar (Ctrl+2)"],
    ["contacts", "👥", "Contacts (Ctrl+3)"],
  ] as const;
</script>

<svelte:window onkeydown={onKey} />

{#if !app.ready}
  <div class="center"><div class="spinner"></div></div>
{:else}
  <div class="shell">
    <nav class="rail">
      {#each modes as [id, icon, tip]}
        <button class:on={app.mode === id && !app.settings} title={tip} onclick={() => { app.mode = id; app.settings = false; }}>{icon}</button>
      {/each}
      <span class="grow"></span>
      <button class:on={app.settings} title="Settings (Ctrl+,)" onclick={() => (app.settings = !app.settings)}>⚙</button>
    </nav>

    <div class="view">
      {#if app.accounts.length}
        {#if app.mode === "mail"}<MailView />
        {:else if app.mode === "calendar"}<CalendarView />
        {:else}<ContactsView />{/if}
      {/if}
      {#if app.settings || !app.accounts.length}<Settings />{/if}
    </div>
  </div>

  {#if app.compose}<Composer />{/if}

  {#if app.error}
    <div class="toast bad" role="alert">
      <span>{app.error}</span>
      <button class="icon" onclick={() => (app.error = "")}>✕</button>
    </div>
  {:else if app.toast}
    <div class="toast">
      {app.toast}
      {#if app.toastAction}<button onclick={() => { app.toastAction?.run(); app.toast = ""; }}>{app.toastAction.label}</button>{/if}
    </div>
  {/if}
{/if}

<style>
  .center { height: 100vh; display: grid; place-items: center; }
  .shell { display: grid; grid-template-columns: 52px 1fr; height: 100vh; }
  .rail { background: var(--panel); border-right: 1px solid var(--line); display: flex; flex-direction: column; align-items: center; padding: 10px 0; gap: 6px; }
  .rail button { width: 38px; height: 38px; font-size: 18px; padding: 0; background: none; border: none; border-radius: 10px; }
  .rail button:hover { background: var(--panel2); }
  .rail button.on { background: var(--sel); box-shadow: inset 3px 0 0 var(--accent); }
  .grow { flex: 1; }
  .view { position: relative; min-width: 0; min-height: 0; }
  .toast { position: fixed; bottom: 18px; left: 50%; transform: translateX(-50%); background: var(--panel2); border: 1px solid var(--line); padding: 8px 16px; border-radius: 8px; box-shadow: 0 6px 24px #0008; max-width: 70vw; display: flex; gap: 12px; align-items: center; z-index: 100; }
  .toast.bad { border-color: var(--danger); color: var(--danger); }
</style>
