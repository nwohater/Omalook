<script lang="ts">
  import { untrack } from "svelte";
  import { api, type Contact } from "./api";
  import { app, fail, flash, openCompose, accountColor } from "./state.svelte";

  const blank = (): Contact => ({ id: "", etag: null, name: "", emails: [""], phones: [""], company: "", title: "" });

  let contacts = $state<Contact[]>([]);
  let q = $state("");
  let loading = $state(false);
  let sel = $state<Contact | null>(null);
  let saving = $state(false);
  let epoch = 0;

  let shown = $derived(
    contacts.filter((c) => {
      const t = q.trim().toLowerCase();
      return !t || c.name.toLowerCase().includes(t) || c.emails.some((e) => e.toLowerCase().includes(t)) || c.company.toLowerCase().includes(t);
    }),
  );

  async function load() {
    const my = ++epoch, id = app.activeId;
    sel = null;
    if (!id) return;
    loading = true;
    try {
      const r = await api.contacts(id);
      if (my === epoch) contacts = r.sort((a, b) => (a.name || a.emails[0] || "").localeCompare(b.name || b.emails[0] || ""));
    } catch (e) { if (my === epoch) fail(e); }
    finally { if (my === epoch) loading = false; }
  }

  $effect(() => {
    app.activeId;
    untrack(() => load());
  });

  function pick(c: Contact) {
    sel = { ...c, emails: c.emails.length ? [...c.emails] : [""], phones: c.phones.length ? [...c.phones] : [""] };
  }

  async function save() {
    const c = sel;
    if (!c) return;
    if (!c.name.trim() && !c.emails.some((e) => e.trim())) return fail("Add a name or an email address.");
    saving = true;
    try {
      await api.saveContact(app.activeId, $state.snapshot(c));
      flash("Contact saved");
      const wasNew = !c.id;
      await load();
      if (wasNew) sel = null;
      else { const again = contacts.find((x) => x.id === c.id); if (again) pick(again); }
    } catch (e) { fail(e); } finally { saving = false; }
  }

  async function remove() {
    const c = sel;
    if (!c?.id || !confirm(`Delete ${c.name || "this contact"}?`)) return;
    try {
      await api.deleteContact(app.activeId, c.id);
      flash("Contact deleted");
      await load();
    } catch (e) { fail(e); }
  }

  const initials = (c: Contact) => (c.name || c.emails[0] || "?")[0];
</script>

<div class="people">
  <section class="list">
    <header>
      <div class="top">
        <h2>Contacts</h2>
        <span class="sp"></span>
        <button class="primary" onclick={() => (sel = blank())}>＋ New</button>
      </div>
      {#if app.accounts.length > 1}
        <select bind:value={app.activeId}>{#each app.accounts as a}<option value={a.id}>{a.email}</option>{/each}</select>
      {/if}
      <input placeholder="Search contacts" bind:value={q} />
    </header>
    <div class="rows">
      {#each shown as c (c.id)}
        <button class="row" class:sel={sel?.id === c.id} onclick={() => pick(c)}>
          <span class="avatar" style:background={accountColor(app.accounts.findIndex((a) => a.id === app.activeId))}>{initials(c)}</span>
          <span class="txt"><b>{c.name || c.emails[0]}</b><small>{c.name ? c.emails[0] ?? c.company : c.company}</small></span>
        </button>
      {/each}
      {#if loading}<div class="hint"><div class="spinner small"></div></div>{/if}
      {#if !loading && !shown.length}<div class="hint">{contacts.length ? "No matches." : "No contacts yet."}</div>{/if}
    </div>
  </section>

  <main>
    {#if sel}
      {@const c = sel}
      <div class="form">
        <div class="hd">
          <span class="avatar big">{initials(c)}</span>
          <input class="name" placeholder="Full name" bind:value={c.name} />
        </div>
        <div class="grp">
          <h4>Email</h4>
          {#each c.emails as _, i}<input type="email" placeholder="name@example.com" bind:value={c.emails[i]} />{/each}
          <button class="link" onclick={() => c.emails.push("")}>＋ Add email</button>
        </div>
        <div class="grp">
          <h4>Phone</h4>
          {#each c.phones as _, i}<input type="tel" placeholder="+1 555 0100" bind:value={c.phones[i]} />{/each}
          <button class="link" onclick={() => c.phones.push("")}>＋ Add phone</button>
        </div>
        <div class="grp two">
          <label>Company<input bind:value={c.company} /></label>
          <label>Job title<input bind:value={c.title} /></label>
        </div>
        <div class="actions">
          <button class="primary" disabled={saving} onclick={save}>{saving ? "Saving…" : "Save"}</button>
          {#if c.emails.find((e) => e.trim())}
            <button onclick={() => openCompose({ to: c.emails.find((e) => e.trim())! })}>✉ Email</button>
          {/if}
          <button onclick={() => (sel = null)}>Cancel</button>
          <span class="sp"></span>
          {#if c.id}<button class="danger" onclick={remove}>🗑 Delete</button>{/if}
        </div>
      </div>
    {:else}
      <div class="empty"><div class="logo">👥</div><p>Select a contact, or add a new one</p></div>
    {/if}
  </main>
</div>

<style>
  .people { display: grid; grid-template-columns: 340px 1fr; height: 100%; min-height: 0; }
  .list { border-right: 1px solid var(--line); display: flex; flex-direction: column; min-height: 0; background: var(--bg); }
  header { padding: 12px; display: grid; gap: 8px; border-bottom: 1px solid var(--line); }
  .top { display: flex; align-items: center; }
  h2 { margin: 0; font-size: 17px; }
  .sp { flex: 1; }
  .rows { overflow-y: auto; flex: 1; }
  .row { display: flex; align-items: center; gap: 10px; width: 100%; text-align: left; border: none; border-bottom: 1px solid var(--line); border-radius: 0; background: none; padding: 8px 14px; border-left: 3px solid transparent; }
  .row:hover { background: var(--panel); }
  .row.sel { background: var(--sel); border-left-color: var(--accent); }
  .txt { display: grid; min-width: 0; }
  .txt b, .txt small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .avatar { width: 32px; height: 32px; border-radius: 50%; color: var(--accent-fg); display: grid; place-items: center; font-weight: 700; flex: none; text-transform: uppercase; }
  .avatar.big { width: 56px; height: 56px; font-size: 22px; }
  .hint { color: var(--dim); text-align: center; padding: 24px; }
  main { background: var(--panel); overflow-y: auto; }
  .form { max-width: 560px; margin: 28px auto; padding: 0 20px; display: grid; gap: 18px; }
  .hd { display: flex; align-items: center; gap: 14px; }
  .name { font-size: 20px; font-weight: 600; }
  .grp { display: grid; gap: 6px; }
  .grp h4 { margin: 0; color: var(--dim); font-size: 12px; text-transform: uppercase; letter-spacing: 0.05em; }
  .two { grid-template-columns: 1fr 1fr; gap: 10px; }
  label { display: grid; gap: 4px; color: var(--dim); font-size: 12.5px; }
  .link { background: none; border: none; color: var(--accent); justify-self: start; padding: 2px 0; }
  .actions { display: flex; gap: 8px; }
  .danger:hover { border-color: var(--danger); color: var(--danger); }
  .empty { height: 100%; display: grid; place-content: center; text-align: center; color: var(--dim); }
  .logo { font-size: 44px; }
</style>
