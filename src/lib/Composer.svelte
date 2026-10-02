<script lang="ts">
  import { onMount } from "svelte";
  import { open as pickFiles } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, type Contact, type Outgoing } from "./api";
  import { app, fail, flash, accountColor } from "./state.svelte";
  import { baseName, fileSize, splitAddrs } from "./format";
  import RichEditor from "./RichEditor.svelte";
  import AddrInput from "./AddrInput.svelte";

  let editor = $state<RichEditor>();
  let sending = $state(false);
  let saving = $state(false);
  let dirty = $state(false);
  let showCc = $state(!!app.compose?.cc || !!app.compose?.bcc);
  let showBcc = $state(!!app.compose?.bcc);
  let drag = $state(false);
  let contacts = $state<Contact[]>([]);

  let c = $derived(app.compose!);

  $effect(() => {
    const id = c.accountId;
    contacts = [];
    api.contacts(id).then((r) => { if (app.compose?.accountId === id) contacts = r; }).catch(() => {});
  });

  onMount(() => {
    let off: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "over") drag = true;
        else if (p.type === "drop") { drag = false; addPaths(p.paths); }
        else drag = false;
      })
      .then((u) => (off = u));
    return () => off?.();
  });

  function addPaths(paths: string[]) {
    for (const p of paths) if (!c.attachments.some((a) => a.path === p)) c.attachments.push({ name: baseName(p), path: p });
    dirty = true;
  }

  async function attach() {
    const r = await pickFiles({ multiple: true });
    if (r) addPaths(Array.isArray(r) ? r : [r]);
  }

  const hasContent = () => !!(c.to.trim() || c.subject.trim() || c.attachments.length || c.existing.length || !editor?.isEmpty());

  function build(): Outgoing {
    return {
      to: splitAddrs(c.to), cc: splitAddrs(c.cc), bcc: splitAddrs(c.bcc),
      subject: c.subject, html: editor?.getHtml() ?? c.html,
      attachments: c.attachments, keepAttachments: c.existing.map((a) => a.id),
      replyToId: c.replyToId, replyAll: c.replyAll, forwardOfId: c.forwardOfId, draftId: c.draftId,
    };
  }

  async function persist(): Promise<boolean> {
    saving = true;
    try {
      const id = await api.saveDraft(c.accountId, build());
      c.draftId = id;
      // re-read so attachment ids are the server's and new files aren't uploaded twice
      const d = await api.openDraft(c.accountId, id);
      c.existing = d.attachments;
      c.attachments = [];
      dirty = false;
      app.mailRev++;
      return true;
    } catch (e) {
      fail(e);
      return false;
    } finally {
      saving = false;
    }
  }

  async function saveDraft() {
    if (await persist()) flash("Draft saved");
  }

  async function send() {
    const out = build();
    if (!out.to.length) { fail("Add at least one recipient."); return; }
    if (!out.subject.trim() && !confirm("Send without a subject?")) return;
    sending = true;
    try {
      await api.send(c.accountId, out);
      app.compose = null;
      app.mailRev++;
      flash("Message sent");
    } catch (e) {
      fail(e);
    } finally {
      sending = false;
    }
  }

  async function discard() {
    if (hasContent() && !confirm("Discard this message?")) return;
    const id = c.draftId, acct = c.accountId;
    app.compose = null;
    if (id) { try { await api.discardDraft(acct, id); app.mailRev++; } catch (e) { fail(e); } }
  }

  async function close() {
    if (!hasContent()) { return discard(); }
    if (dirty || !c.draftId) { if (!(await persist())) return; flash("Draft saved"); }
    app.compose = null;
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") close();
    else if ((e.ctrlKey || e.metaKey) && e.key === "Enter") { e.preventDefault(); send(); }
    else if ((e.ctrlKey || e.metaKey) && e.key === "s") { e.preventDefault(); saveDraft(); }
  }
</script>

<svelte:window onkeydown={key} />

<div class="bg" role="presentation">
  <div class="win" class:drag role="dialog" aria-label="New message" tabindex="-1" oninput={() => (dirty = true)}>
    <div class="mh">
      <b>{c.subject || "New message"}</b>
      <span class="sp"></span>
      <button class="icon" title="Save and close (Esc)" onclick={close}>✕</button>
    </div>

    <div class="fields">
      {#if app.accounts.length > 1 && !c.draftId && !c.replyToId && !c.forwardOfId}
        <div class="row">
          <span class="lbl">From</span>
          <select bind:value={c.accountId}>
            {#each app.accounts as a, i}<option value={a.id}>{a.name} &lt;{a.email}&gt;</option>{/each}
          </select>
        </div>
      {:else if app.accounts.length > 1}
        <div class="row"><span class="lbl">From</span><span class="from">{app.accounts.find((a) => a.id === c.accountId)?.email}</span></div>
      {/if}
      <div class="rowx">
        <div class="grow"><AddrInput label="To" bind:value={c.to} {contacts} /></div>
        {#if !showCc}<button class="link" onclick={() => (showCc = true)}>Cc</button>{/if}
        {#if !showBcc}<button class="link" onclick={() => { showCc = true; showBcc = true; }}>Bcc</button>{/if}
      </div>
      {#if showCc}<AddrInput label="Cc" bind:value={c.cc} {contacts} />{/if}
      {#if showBcc}<AddrInput label="Bcc" bind:value={c.bcc} {contacts} />{/if}
      <div class="row"><span class="lbl">Subj</span><input bind:value={c.subject} placeholder="Subject" /></div>
    </div>

    <RichEditor bind:this={editor} html={c.html} onchange={() => (dirty = true)} />

    {#if c.attachments.length || c.existing.length}
      <div class="atts">
        {#each c.existing as a}
          <span class="att">📎 {a.name} <small>{fileSize(a.size)}</small>
            <button class="icon" onclick={() => { c.existing = c.existing.filter((x) => x.id !== a.id); dirty = true; }}>✕</button></span>
        {/each}
        {#each c.attachments as a}
          <span class="att">📎 {a.name}
            <button class="icon" onclick={() => { c.attachments = c.attachments.filter((x) => x.path !== a.path); }}>✕</button></span>
        {/each}
      </div>
    {/if}

    <div class="actions">
      <button class="primary" onclick={send} disabled={sending}>{sending ? "Sending…" : "Send"}</button>
      <button onclick={attach}>📎 Attach</button>
      <button onclick={saveDraft} disabled={saving}>{saving ? "Saving…" : "Save draft"}</button>
      <span class="sp"></span>
      <small>Ctrl+Enter send · Ctrl+S save · drop files to attach</small>
      <button class="danger" onclick={discard}>🗑 Discard</button>
    </div>
    {#if drag}<div class="dropmsg">Drop files to attach</div>{/if}
  </div>
</div>

<style>
  .bg { position: fixed; inset: 0; background: #0009; display: grid; place-items: center; z-index: 50; }
  .win { position: relative; width: min(880px, 94vw); height: min(720px, 92vh); background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 12px 14px; display: flex; flex-direction: column; gap: 8px; box-shadow: 0 20px 60px #000a; }
  .win.drag { outline: 2px dashed var(--accent); }
  .dropmsg { position: absolute; inset: 0; display: grid; place-items: center; background: #0007; font-size: 18px; border-radius: 12px; pointer-events: none; }
  .mh { display: flex; align-items: center; gap: 8px; }
  .sp { flex: 1; }
  .fields { display: grid; gap: 6px; }
  .row { display: flex; align-items: center; gap: 8px; }
  .rowx { display: flex; align-items: center; gap: 8px; }
  .grow { flex: 1; }
  .lbl { width: 34px; color: var(--dim); font-size: 12.5px; }
  .from { color: var(--dim); }
  .link { background: none; border: none; color: var(--accent); padding: 2px 6px; }
  .atts { display: flex; gap: 8px; flex-wrap: wrap; }
  .att { background: var(--panel2); border: 1px solid var(--line); border-radius: 6px; padding: 3px 4px 3px 10px; display: inline-flex; gap: 6px; align-items: center; }
  .actions { display: flex; gap: 8px; align-items: center; }
  .danger:hover { border-color: var(--danger); color: var(--danger); }
</style>
