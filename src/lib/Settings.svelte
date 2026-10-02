<script lang="ts">
  import { marked } from "marked";
  import { api, type Kind } from "./api";
  import { app, fail, flash, reloadAccounts, accountColor } from "./state.svelte";
  import msDoc from "../../docs/setup-microsoft.md?raw";
  import googleDoc from "../../docs/setup-google.md?raw";

  const docs = { microsoft: marked.parse(msDoc) as string, google: marked.parse(googleDoc) as string };

  let ms = $state({ clientId: app.config.microsoft?.clientId ?? "", tenantId: app.config.microsoft?.tenantId ?? "" });
  let g = $state({ clientId: app.config.google?.clientId ?? "", clientSecret: app.config.google?.clientSecret ?? "" });
  let busy = $state<Kind | "">("");

  const msReady = $derived(!!app.config.microsoft?.clientId);
  const gReady = $derived(!!app.config.google?.clientId);

  async function saveCfg() {
    app.config = {
      microsoft: ms.clientId.trim() ? { clientId: ms.clientId.trim(), tenantId: ms.tenantId.trim() || "organizations" } : null,
      google: g.clientId.trim() ? { clientId: g.clientId.trim(), clientSecret: g.clientSecret.trim() } : null,
    };
    await api.saveConfig($state.snapshot(app.config));
  }

  async function connect(kind: Kind) {
    busy = kind;
    try {
      await saveCfg();
      const a = await api.addAccount(kind);
      await reloadAccounts();
      app.activeId = a.id;
      app.settings = false;
      app.mode = "mail";
      flash(`Connected ${a.email}`);
    } catch (e) {
      fail(e);
    } finally {
      busy = "";
    }
  }

  async function remove(id: string, email: string) {
    if (!confirm(`Remove ${email} from Omalook? Your mail stays on the server.`)) return;
    try {
      await api.removeAccount(id);
      await reloadAccounts();
      flash("Account removed");
    } catch (e) { fail(e); }
  }

  const tabs = [
    ["accounts", "Accounts"],
    ["microsoft", "Microsoft 365 setup"],
    ["google", "Google setup"],
    ["shortcuts", "Shortcuts"],
  ] as const;
</script>

<div class="page">
  <header>
    <h1>Settings</h1>
    <span class="sp"></span>
    {#if app.accounts.length}<button onclick={() => (app.settings = false)}>✕ Close</button>{/if}
  </header>

  <div class="tabs">
    {#each tabs as [id, label]}
      <button class:on={app.settingsTab === id} onclick={() => (app.settingsTab = id)}>{label}</button>
    {/each}
  </div>

  <div class="body">
    {#if app.settingsTab === "accounts"}
      {#if !app.accounts.length}
        <div class="welcome">
          <div class="logo">✉</div>
          <h2>Welcome to Omalook</h2>
          <p>Add your first account. Each provider needs a one-time setup (about five minutes) so you own the connection.</p>
        </div>
      {/if}
      <div class="cards">
        {#each app.accounts as a, i}
          <div class="card">
            <span class="dot" style:background={accountColor(i)}></span>
            <div class="grow"><b>{a.name}</b><br /><small>{a.email} · {a.kind === "microsoft" ? "Microsoft 365" : "Google"}</small></div>
            <button class="danger" onclick={() => remove(a.id, a.email)}>Remove</button>
          </div>
        {/each}
      </div>
      <h3>Add an account</h3>
      <div class="add">
        <div class="provider">
          <h4>🪟 Microsoft 365 / Outlook</h4>
          <p>Work, school and Outlook.com accounts.</p>
          {#if msReady}
            <button class="primary" disabled={!!busy} onclick={() => connect("microsoft")}>{busy === "microsoft" ? "Waiting for browser…" : "Add Microsoft account"}</button>
            <button class="link" onclick={() => (app.settingsTab = "microsoft")}>Edit credentials</button>
          {:else}
            <button class="primary" onclick={() => (app.settingsTab = "microsoft")}>Set up Microsoft 365</button>
          {/if}
        </div>
        <div class="provider">
          <h4>📧 Gmail / Google</h4>
          <p>Gmail, Google Calendar and Contacts.</p>
          {#if gReady}
            <button class="primary" disabled={!!busy} onclick={() => connect("google")}>{busy === "google" ? "Waiting for browser…" : "Add Google account"}</button>
            <button class="link" onclick={() => (app.settingsTab = "google")}>Edit credentials</button>
          {:else}
            <button class="primary" onclick={() => (app.settingsTab = "google")}>Set up Google</button>
          {/if}
        </div>
      </div>
      <p class="note">Sign-in tokens are stored in your system keyring (Secret Service), or in a private file in <code>~/.local/share/omalook</code> if no keyring is running. Credentials you enter below are saved in <code>~/.config/omalook/config.json</code>.</p>

    {:else if app.settingsTab === "microsoft"}
      <div class="form">
        <h3>Your app registration</h3>
        <label>Application (client) ID<input bind:value={ms.clientId} placeholder="00000000-0000-0000-0000-000000000000" /></label>
        <label>Directory (tenant) ID<input bind:value={ms.tenantId} placeholder="tenant GUID, or “organizations” / “common”" /></label>
        <div class="actions">
          <button class="primary" disabled={!ms.clientId.trim() || !!busy} onclick={() => connect("microsoft")}>{busy === "microsoft" ? "Waiting for browser…" : "Save & add Microsoft account"}</button>
          <button disabled={!!busy} onclick={() => saveCfg().then(() => flash("Saved")).catch(fail)}>Save only</button>
        </div>
      </div>
      <article class="doc">{@html docs.microsoft}</article>

    {:else if app.settingsTab === "google"}
      <div class="form">
        <h3>Your OAuth client</h3>
        <label>Client ID<input bind:value={g.clientId} placeholder="123456-abc.apps.googleusercontent.com" /></label>
        <label>Client secret<input bind:value={g.clientSecret} placeholder="GOCSPX-…" /></label>
        <div class="actions">
          <button class="primary" disabled={!g.clientId.trim() || !!busy} onclick={() => connect("google")}>{busy === "google" ? "Waiting for browser…" : "Save & add Google account"}</button>
          <button disabled={!!busy} onclick={() => saveCfg().then(() => flash("Saved")).catch(fail)}>Save only</button>
        </div>
      </div>
      <article class="doc">{@html docs.google}</article>

    {:else}
      <table class="keys">
        <tbody>
          <tr><th colspan="2">Mail</th></tr>
          <tr><td><kbd>j</kbd> <kbd>k</kbd></td><td>Next / previous message</td></tr>
          <tr><td><kbd>c</kbd></td><td>New message</td></tr>
          <tr><td><kbd>r</kbd> <kbd>a</kbd> <kbd>Shift+F</kbd></td><td>Reply / reply all / forward</td></tr>
          <tr><td><kbd>e</kbd></td><td>Archive</td></tr>
          <tr><td><kbd>Del</kbd> <kbd>#</kbd></td><td>Delete</td></tr>
          <tr><td><kbd>!</kbd></td><td>Mark as junk</td></tr>
          <tr><td><kbd>u</kbd> <kbd>f</kbd></td><td>Toggle read / flag</td></tr>
          <tr><td><kbd>/</kbd></td><td>Search</td></tr>
          <tr><td><kbd>F5</kbd> <kbd>F9</kbd></td><td>Check for new mail</td></tr>
          <tr><td><kbd>Enter</kbd></td><td>Edit the selected draft (in Drafts)</td></tr>
          <tr><th colspan="2">Composer</th></tr>
          <tr><td><kbd>Ctrl+Enter</kbd></td><td>Send</td></tr>
          <tr><td><kbd>Ctrl+S</kbd></td><td>Save draft</td></tr>
          <tr><td><kbd>Esc</kbd></td><td>Save draft and close</td></tr>
          <tr><th colspan="2">Everywhere</th></tr>
          <tr><td><kbd>Ctrl+1</kbd> <kbd>2</kbd> <kbd>3</kbd></td><td>Mail / Calendar / Contacts</td></tr>
          <tr><td><kbd>Ctrl+,</kbd></td><td>Settings</td></tr>
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .page { position: absolute; inset: 0; background: var(--bg); display: flex; flex-direction: column; z-index: 40; }
  header { display: flex; align-items: center; padding: 14px 24px; border-bottom: 1px solid var(--line); }
  h1 { margin: 0; font-size: 20px; }
  .sp, .grow { flex: 1; }
  .tabs { display: flex; gap: 4px; padding: 8px 24px 0; border-bottom: 1px solid var(--line); }
  .tabs button { background: none; border: none; border-bottom: 2px solid transparent; border-radius: 0; padding: 8px 14px; color: var(--dim); }
  .tabs button.on { color: var(--fg); border-bottom-color: var(--accent); }
  .body { overflow-y: auto; padding: 20px 24px 40px; max-width: 900px; width: 100%; margin: 0 auto; }
  .welcome { text-align: center; margin-bottom: 18px; }
  .welcome h2 { margin: 4px 0; }
  .welcome p { color: var(--dim); }
  .logo { font-size: 44px; color: var(--accent); }
  .cards { display: grid; gap: 8px; margin-bottom: 8px; }
  .card { display: flex; align-items: center; gap: 12px; background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 10px 14px; }
  .dot { width: 12px; height: 12px; border-radius: 50%; flex: none; }
  .add { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .provider { background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 14px; display: flex; flex-direction: column; gap: 6px; align-items: flex-start; }
  .provider h4 { margin: 0; }
  .provider p { margin: 0 0 6px; color: var(--dim); }
  .link { background: none; border: none; color: var(--accent); padding: 2px 0; }
  .note { color: var(--dim); font-size: 12.5px; margin-top: 18px; }
  .form { background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 14px 16px; display: grid; gap: 10px; margin-bottom: 22px; }
  .form h3 { margin: 0; }
  label { display: grid; gap: 4px; color: var(--dim); font-size: 12.5px; }
  .actions { display: flex; gap: 8px; }
  .danger:hover { border-color: var(--danger); color: var(--danger); }
  .keys { border-collapse: collapse; width: 100%; }
  .keys th { text-align: left; padding: 14px 0 6px; color: var(--accent); }
  .keys td { padding: 5px 0; border-bottom: 1px solid var(--line); }
  .keys td:first-child { width: 280px; }
  kbd { background: var(--panel2); border: 1px solid var(--line); border-radius: 4px; padding: 1px 6px; font-size: 12px; }

  .doc :global(h2) { margin-top: 0; }
  .doc :global(h3) { margin: 22px 0 6px; }
  .doc :global(a) { color: var(--accent); }
  .doc :global(code) { background: var(--panel2); padding: 1px 5px; border-radius: 4px; font-size: 12.5px; }
  .doc :global(blockquote) { margin: 10px 0; padding: 4px 14px; border-left: 3px solid var(--accent); color: var(--dim); }
  .doc :global(table) { border-collapse: collapse; width: 100%; font-size: 13px; }
  .doc :global(th), .doc :global(td) { border: 1px solid var(--line); padding: 6px 10px; text-align: left; vertical-align: top; }
  .doc :global(li) { margin: 3px 0; }
</style>
