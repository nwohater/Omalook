<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Folder, type MsgDetail, type MsgSummary } from "./api";
  import { app, fail, flash, openCompose, accountColor } from "./state.svelte";
  import { addrName, cleanQuote, esc, fileSize, when } from "./format";

  const ICONS: Record<string, string> = {
    inbox: "📥", drafts: "📝", sent: "📤", archive: "🗄️", all: "🗄️", trash: "🗑️", junk: "🚫", starred: "⭐",
  };

  let folders = $state<Folder[]>([]);
  let folderId = $state("");
  let msgs = $state<MsgSummary[]>([]);
  let next = $state<string | null>(null);
  let selected = $state<MsgDetail | null>(null);
  let selectedId = $state("");
  let loading = $state(false);
  let search = $state("");
  let loadImages = $state(false);
  let acctMenu = $state(false);
  let searchEl = $state<HTMLInputElement>();
  let frameEl = $state<HTMLIFrameElement>();
  let epoch = 0;
  let expanded = $state<string[]>([]);
  let loadingFolders = $state(false);
  let refreshing = $state(false);
  let lastSync = 0;

  // last-seen state per account, so switching accounts is instant and refreshes in the background
  type Snap = { folders: Folder[]; folderId: string; msgs: MsgSummary[]; next: string | null };
  const cache = new Map<string, Snap>();
  function snapshot() {
    if (app.activeId && folders.length) cache.set(app.activeId, { folders: $state.snapshot(folders), folderId, msgs: $state.snapshot(msgs), next });
  }

  const storeKey = () => `omalook.expanded.${app.activeId}`;
  function loadExpanded() {
    try { expanded = JSON.parse(localStorage.getItem(storeKey()) ?? "[]"); } catch { expanded = []; }
  }
  function toggleFolder(id: string) {
    expanded = expanded.includes(id) ? expanded.filter((x) => x !== id) : [...expanded, id];
    try { localStorage.setItem(storeKey(), JSON.stringify(expanded)); } catch { /* storage unavailable */ }
  }

  // folders arrive depth-first; hide everything under a collapsed parent
  let tree = $derived.by(() => {
    const out: { f: Folder; parent: boolean; open: boolean }[] = [];
    let hideBelow: number | null = null;
    folders.forEach((f, i) => {
      if (hideBelow !== null) {
        if (f.depth > hideBelow) return;
        hideBelow = null;
      }
      const parent = (folders[i + 1]?.depth ?? 0) > f.depth;
      const open = expanded.includes(f.id);
      out.push({ f, parent, open });
      if (parent && !open) hideBelow = f.depth;
    });
    return out;
  });

  let account = $derived(app.accounts.find((a) => a.id === app.activeId));
  let folder = $derived(folders.find((f) => f.id === folderId));
  let index = $derived(msgs.findIndex((m) => m.id === selectedId));
  let inDrafts = $derived(folder?.role === "drafts");

  // ── loading ──────────────────────────────────────────────────────

  async function boot() {
    const id = app.activeId;
    epoch++;
    selected = null; selectedId = "";
    loadExpanded();
    const c = cache.get(id);
    if (c) { folders = c.folders; folderId = c.folderId; msgs = c.msgs; next = c.next; }
    else { folders = []; msgs = []; next = null; folderId = ""; }
    if (!id) return;
    if (c) refreshing = true; else loadingFolders = true;
    try {
      const f = await api.folders(id);
      if (id !== app.activeId) return;
      folders = f;
      lastSync = Date.now();
      if (f.some((x) => x.id === folderId)) await load(false); // revalidate what's on screen
      else {
        const inbox = f.find((x) => x.role === "inbox") ?? f[0];
        if (inbox) await openFolder(inbox.id);
      }
    } catch (e) { fail(e); }
    finally { if (id === app.activeId) { refreshing = false; loadingFolders = false; } }
  }

  async function openFolder(id: string) {
    folderId = id;
    search = "";
    selected = null; selectedId = "";
    msgs = []; next = null;
    await load(false);
  }

  async function load(more: boolean) {
    const my = ++epoch, acct = app.activeId, fid = folderId;
    loading = true;
    try {
      const r = await api.messages(acct, fid, more ? next : null, search);
      if (my !== epoch) return;
      msgs = more ? [...msgs, ...r.items.filter((n) => !msgs.some((m) => m.id === n.id))] : r.items;
      next = r.next;
    } catch (e) { if (my === epoch) fail(e); }
    finally { if (my === epoch) { loading = false; snapshot(); } }
  }

  async function refreshFolders() {
    const id = app.activeId;
    try {
      const f = await api.folders(id);
      if (id !== app.activeId) return;
      for (const nf of f) {
        const old = folders.find((x) => x.id === nf.id);
        if (old) { old.unread = nf.unread; old.total = nf.total; }
      }
      if (f.length !== folders.length) folders = f;
    } catch { /* background refresh; ignore */ }
  }

  /** Pull in new mail without disturbing selection or scroll position. */
  async function poll() {
    if (!folderId || search || loading || app.compose) return;
    const acct = app.activeId, fid = folderId;
    try {
      const r = await api.messages(acct, fid, null, null);
      if (acct !== app.activeId || fid !== folderId || search) return;
      const fresh = r.items.filter((n) => !msgs.some((m) => m.id === n.id));
      if (fresh.length) {
        msgs = [...fresh, ...msgs];
        if (folder?.role === "inbox" && fresh.some((m) => !m.isRead)) flash(`${fresh.filter((m) => !m.isRead).length} new message(s)`);
      }
      refreshFolders();
    } catch { /* offline etc. */ }
  }

  $effect(() => {
    app.activeId;
    untrack(() => boot());
  });

  let lastRev = 0;
  $effect(() => {
    const r = app.mailRev;
    if (r === lastRev) return;
    lastRev = r;
    untrack(() => { refreshFolders(); if (inDrafts || folder?.role === "sent") load(false); });
  });

  /** Manual "check for mail": refresh folder counts and re-read the current folder. */
  async function refreshNow() {
    if (!folderId || refreshing) return;
    const before = new Set(msgs.map((m) => m.id));
    refreshing = true;
    lastSync = Date.now();
    try {
      await Promise.all([refreshFolders(), load(false)]);
      const fresh = msgs.filter((m) => !before.has(m.id)).length;
      flash(fresh ? `${fresh} new message${fresh > 1 ? "s" : ""}` : "Up to date");
    } finally { refreshing = false; }
  }

  // coming back from Calendar/Contacts/Settings: pull new mail if it's been a while
  $effect(() => {
    if (app.mode !== "mail" || app.settings) return;
    untrack(() => {
      if (!folderId || Date.now() - lastSync < 20_000) return;
      lastSync = Date.now();
      refreshing = true;
      poll().finally(() => (refreshing = false));
    });
  });

  onMount(() => {
    const t = setInterval(() => { lastSync = Date.now(); poll(); }, 60_000);
    return () => clearInterval(t);
  });

  // ── actions ──────────────────────────────────────────────────────

  async function select(m: MsgSummary) {
    loadImages = false;
    selectedId = m.id;
    const acct = app.activeId;
    try {
      const d = await api.message(acct, m.id);
      if (selectedId !== m.id || acct !== app.activeId) return;
      selected = d;
    } catch (e) { fail(e); }
    if (!m.isRead) {
      m.isRead = true;
      if (folder && folder.unread > 0) folder.unread--;
      api.setRead(acct, m.id, true).catch(fail);
    }
  }

  function dropFromList(id: string) {
    const i = msgs.findIndex((x) => x.id === id);
    msgs = msgs.filter((x) => x.id !== id);
    selected = null; selectedId = "";
    const nxt = msgs[Math.min(i, msgs.length - 1)];
    if (nxt) select(nxt);
  }

  async function moveTo(m: MsgSummary, target: string, label: string) {
    dropFromList(m.id);
    try {
      await api.move(app.activeId, m.id, target, folderId);
      flash(label);
      refreshFolders();
    } catch (e) { fail(e); load(false); }
  }

  const archive = (m: MsgSummary) => moveTo(m, "archive", "Archived");
  const junk = (m: MsgSummary) => moveTo(m, "junk", "Moved to junk");

  async function del(m: MsgSummary) {
    if (folder?.role === "trash") {
      if (!confirm("Delete this message permanently?")) return;
      dropFromList(m.id);
      try { await api.deleteForever(app.activeId, m.id); flash("Deleted"); refreshFolders(); } catch (e) { fail(e); load(false); }
    } else moveTo(m, "trash", "Moved to Deleted Items");
  }

  async function toggleRead(m: MsgSummary) {
    m.isRead = !m.isRead;
    if (selected?.id === m.id) selected.isRead = m.isRead;
    try { await api.setRead(app.activeId, m.id, m.isRead); refreshFolders(); } catch (e) { fail(e); }
  }

  async function toggleFlag(m: MsgSummary) {
    m.flagged = !m.flagged;
    if (selected?.id === m.id) selected.flagged = m.flagged;
    try { await api.setFlag(app.activeId, m.id, m.flagged); } catch (e) { fail(e); }
  }

  async function download(a: { id: string; name: string }) {
    if (!selected) return;
    try {
      const path = await api.download(app.activeId, selected.id, a.id, a.name);
      flash(`Saved ${a.name} to Downloads`, { label: "Open", run: () => api.openDownload(path).catch(fail) });
    } catch (e) { fail(e); }
  }

  // ── compose helpers ──────────────────────────────────────────────

  function quote(d: MsgDetail, head: string) {
    return `<div><br></div><div><br></div><div>${head}</div><blockquote>${cleanQuote(d.html)}</blockquote>`;
  }

  function prefixed(subject: string, p: string) {
    return new RegExp(`^\\s*${p}:`, "i").test(subject) ? subject : `${p}: ${subject}`;
  }

  function reply(all: boolean) {
    const d = selected;
    if (!d || !account) return;
    const me = account.email.toLowerCase();
    const others = (list: { address: string }[]) => list.map((a) => a.address).filter((a) => a && a.toLowerCase() !== me);
    const to = all ? [d.from.address, ...others(d.to)] : [d.from.address];
    const cc = all ? others(d.cc) : [];
    openCompose({
      to: [...new Set(to)].join(", "), cc: [...new Set(cc)].join(", "),
      subject: prefixed(d.subject, "Re"), replyToId: d.id, replyAll: all,
      html: quote(d, `On ${new Date(d.received).toLocaleString()}, ${esc(addrName(d.from))} wrote:`),
    });
  }

  function forward() {
    const d = selected;
    if (!d) return;
    const head = `---------- Forwarded message ----------<br>From: ${esc(addrName(d.from))} &lt;${esc(d.from.address)}&gt;<br>Date: ${esc(new Date(d.received).toLocaleString())}<br>Subject: ${esc(d.subject)}<br>To: ${esc(d.to.map(addrName).join(", "))}`;
    openCompose({ subject: prefixed(d.subject, "Fwd"), forwardOfId: d.id, html: quote(d, head) });
  }

  async function editDraft() {
    const m = selected;
    if (!m) return;
    try {
      const d = await api.openDraft(app.activeId, m.id);
      openCompose({
        to: d.to.join(", "), cc: d.cc.join(", "), bcc: d.bcc.join(", "), subject: d.subject, html: d.html,
        existing: d.attachments, draftId: d.draftId, replyToId: null,
      });
    } catch (e) { fail(e); }
  }

  // ── reader frame ─────────────────────────────────────────────────

  let hasRemote = $derived(!!selected && /<img[^>]+src=["']?https?:/i.test(selected.html));

  let frameDoc = $derived.by(() => {
    if (!selected) return "";
    const img = loadImages ? "img-src data: https: http:" : "img-src data:";
    const csp = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; ${img}">`;
    const base = `<style>html{background:#fff;color:#1b1b1f}body{margin:16px;font:14px/1.5 system-ui,sans-serif;word-wrap:break-word}img{max-width:100%;height:auto}pre{white-space:pre-wrap}</style>`;
    return `<!doctype html><html><head><meta charset="utf-8">${csp}<base target="_blank">${base}</head><body>${selected.html}</body></html>`;
  });

  function hookLinks() {
    frameEl?.contentDocument?.addEventListener("click", (e) => {
      const a = (e.target as HTMLElement).closest?.("a") as HTMLAnchorElement | null;
      if (!a) return;
      e.preventDefault();
      if (/^(https?|mailto):/i.test(a.href)) openUrl(a.href).catch(fail);
    });
  }

  // ── keyboard ─────────────────────────────────────────────────────

  function onKey(e: KeyboardEvent) {
    if (app.mode !== "mail" || app.compose || app.settings) return;
    if (e.key === "F5" || e.key === "F9") { e.preventDefault(); refreshNow(); return; }
    const t = e.target as HTMLElement;
    if (/INPUT|TEXTAREA|SELECT/.test(t.tagName) || e.ctrlKey || e.metaKey || e.altKey) return;
    const m = msgs[index];
    switch (e.key) {
      case "j": case "ArrowDown": e.preventDefault(); if (index < msgs.length - 1) select(msgs[index + 1]); break;
      case "k": case "ArrowUp": e.preventDefault(); if (index > 0) select(msgs[index - 1]); break;
      case "c": e.preventDefault(); openCompose(); break;
      case "/": e.preventDefault(); searchEl?.focus(); break;
      case "r": if (selected) { e.preventDefault(); reply(false); } break;
      case "a": if (selected) { e.preventDefault(); reply(true); } break;
      case "F": if (selected) { e.preventDefault(); forward(); } break;
      case "Enter": if (selected && inDrafts) editDraft(); break;
      case "e": if (m) archive(m); break;
      case "u": if (m) toggleRead(m); break;
      case "f": if (m) toggleFlag(m); break;
      case "#": case "Delete": if (m) del(m); break;
      case "!": if (m) junk(m); break;
    }
  }

  function onScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (next && !loading && el.scrollTop + el.clientHeight > el.scrollHeight - 300) load(true);
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="mail">
  <aside>
    <div class="acct">
      <button class="acctbtn" onclick={() => (acctMenu = !acctMenu)}>
        <span class="avatar" style:background={accountColor(app.accounts.findIndex((a) => a.id === app.activeId))}>{(account?.name ?? "?")[0]}</span>
        <span class="who"><b>{account?.name}</b><small>{account?.email}</small></span>
        <span>▾</span>
      </button>
      {#if acctMenu}
        <div class="menu">
          {#each app.accounts as a, i}
            <button class:on={a.id === app.activeId} onclick={() => { app.activeId = a.id; acctMenu = false; }}>
              <span class="dot" style:background={accountColor(i)}></span>{a.email}
            </button>
          {/each}
          <button onclick={() => { acctMenu = false; app.settings = true; app.settingsTab = "accounts"; }}>＋ Add account…</button>
        </div>
      {/if}
    </div>
    <button class="primary compose" onclick={() => openCompose()}>✏️ New message</button>
    <nav>
      {#if loadingFolders}<div class="hint"><span class="spinner small"></span> Loading folders…</div>{/if}
      {#each tree as { f, parent, open } (f.id)}
        <div class="frow" class:active={f.id === folderId} style:padding-left="{4 + f.depth * 14}px">
          {#if parent}
            <button class="chev" title={open ? "Collapse" : "Expand"} onclick={() => toggleFolder(f.id)}>{open ? "▾" : "▸"}</button>
          {:else}
            <span class="chev"></span>
          {/if}
          <button class="folder" onclick={() => openFolder(f.id)}>
            <span class="ico">{ICONS[f.role] ?? "📁"}</span>
            <span class="fname">{f.name}</span>
            {#if f.unread > 0 && f.role !== "trash"}<span class="badge">{f.unread}</span>{/if}
          </button>
        </div>
      {/each}
    </nav>
  </aside>

  <section class="list">
    <header>
      <div class="titlebar">
        <h2>{folder?.name ?? ""}{#if loading || refreshing || loadingFolders}<span class="spinner small" title="Refreshing…"></span>{/if}</h2>
        <button class="icon refresh" title="Check for new mail (F5)" disabled={!folderId || refreshing} onclick={refreshNow}>⟳</button>
      </div>
      {#if loading || refreshing || loadingFolders}<div class="progress"></div>{/if}
      <input bind:this={searchEl} bind:value={search} placeholder="Search  ( / )" onkeydown={(e) => e.key === "Enter" && load(false)} />
    </header>
    <div class="rows" onscroll={onScroll}>
      {#each msgs as m (m.id)}
        <button class="row" class:unread={!m.isRead} class:sel={selectedId === m.id} onclick={() => select(m)} ondblclick={() => inDrafts && editDraft()}>
          <div class="top">
            <span class="from">{inDrafts || folder?.role === "sent" ? "To: " + (m.to.map(addrName).join(", ") || "(no recipient)") : addrName(m.from)}</span>
            <span class="time">{when(m.received)}</span>
          </div>
          <div class="subj">
            {#if m.flagged}<span class="flag">⚑</span>{/if}
            {m.subject || "(no subject)"}
            {#if m.hasAttachments}<span class="clip">📎</span>{/if}
          </div>
          <div class="prev">{m.preview}</div>
        </button>
      {/each}
      {#if (loading || loadingFolders) && !msgs.length}<div class="hint"><span class="spinner small"></span> Loading messages…</div>{/if}
      {#if !loading && msgs.length === 0 && folderId}<div class="hint">Nothing here.</div>{/if}
    </div>
  </section>

  <main>
    {#if selected}
      <div class="toolbar">
        {#if inDrafts}<button class="primary" onclick={editDraft}>✎ Edit draft</button>{/if}
        <button onclick={() => reply(false)}>↩ Reply</button>
        <button onclick={() => reply(true)}>↩↩ Reply all</button>
        <button onclick={forward} title="Forward (Shift+F)">↪ Forward</button>
        <span class="sp"></span>
        <button onclick={() => archive(selected!)} title="Archive (e)">🗄️</button>
        <button onclick={() => toggleRead(selected!)} title="Mark read/unread (u)">{selected.isRead ? "○" : "●"}</button>
        <button onclick={() => toggleFlag(selected!)} title="Flag (f)" class:on={selected.flagged}>⚑</button>
        <button onclick={() => junk(selected!)} title="Junk (!)">🚫</button>
        <button onclick={() => del(selected!)} title="Delete (Del)">🗑️</button>
      </div>
      <div class="head">
        <h1>{selected.subject || "(no subject)"}</h1>
        <div class="meta">
          <div class="avatar big">{addrName(selected.from)[0]}</div>
          <div class="who2">
            <div><b>{addrName(selected.from)}</b> <small>&lt;{selected.from.address}&gt;</small></div>
            <div class="to">To: {selected.to.map(addrName).join(", ")}{#if selected.cc.length} · Cc: {selected.cc.map(addrName).join(", ")}{/if}</div>
          </div>
          <span class="sp"></span>
          <small>{new Date(selected.received).toLocaleString()}</small>
        </div>
        {#if selected.attachments.some((a) => !a.inline)}
          <div class="atts">
            {#each selected.attachments.filter((a) => !a.inline) as a}
              <button class="att" title="Download" onclick={() => download(a)}>📎 {a.name} <small>{fileSize(a.size)}</small></button>
            {/each}
          </div>
        {/if}
        {#if hasRemote && !loadImages}
          <div class="banner">Remote images are blocked to protect your privacy. <button onclick={() => (loadImages = true)}>Load images</button></div>
        {/if}
      </div>
      <iframe bind:this={frameEl} title="message" sandbox="allow-same-origin" srcdoc={frameDoc} onload={hookLinks}></iframe>
    {:else}
      <div class="empty">
        <div class="logo">✉</div>
        <p>Select a message to read</p>
        <small>j/k move · r reply · a reply all · Shift+F forward · e archive · Del delete · u read/unread · f flag · c compose · / search</small>
      </div>
    {/if}
  </main>
</div>

<style>
  .mail { display: grid; grid-template-columns: 230px 350px 1fr; height: 100%; min-height: 0; }
  aside { background: var(--panel); border-right: 1px solid var(--line); display: flex; flex-direction: column; padding: 10px 8px; gap: 10px; min-height: 0; }
  .acct { position: relative; }
  .acctbtn { display: flex; align-items: center; gap: 8px; width: 100%; text-align: left; background: var(--panel2); }
  .who, .who2 { display: grid; min-width: 0; flex: 1; }
  .who b, .who small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .menu { position: absolute; z-index: 30; left: 0; right: 0; top: 100%; margin-top: 4px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; overflow: hidden; box-shadow: 0 8px 24px #0008; }
  .menu button { display: flex; align-items: center; gap: 8px; width: 100%; border: none; border-radius: 0; background: none; text-align: left; }
  .menu button:hover, .menu button.on { background: var(--sel); }
  .dot { width: 9px; height: 9px; border-radius: 50%; flex: none; }
  .compose { padding: 9px; }
  nav { flex: 1; overflow-y: auto; display: grid; gap: 1px; align-content: start; }
  .frow { display: flex; align-items: center; border-radius: 6px; }
  .frow:hover { background: var(--panel2); }
  .frow.active { background: var(--sel); font-weight: 600; }
  .chev { width: 18px; flex: none; background: none; border: none; padding: 0; color: var(--dim); font-size: 11px; text-align: center; }
  .chev:hover { color: var(--fg); }
  .folder { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; background: none; border: none; text-align: left; padding: 6px 8px 6px 2px; color: inherit; font-weight: inherit; }
  .fname { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge { background: var(--accent); color: var(--accent-fg); border-radius: 10px; padding: 0 7px; font-size: 11px; font-weight: 700; }
  .avatar { width: 30px; height: 30px; border-radius: 50%; background: var(--accent); color: var(--accent-fg); display: grid; place-items: center; font-weight: 700; flex: none; text-transform: uppercase; }
  .avatar.big { width: 38px; height: 38px; }

  .list { border-right: 1px solid var(--line); display: flex; flex-direction: column; min-height: 0; background: var(--bg); }
  .list header { padding: 12px; display: grid; gap: 8px; border-bottom: 1px solid var(--line); }
  .list h2 { margin: 0; font-size: 17px; display: flex; align-items: center; gap: 10px; }
  .list header { position: relative; }
  .titlebar { display: flex; align-items: center; justify-content: space-between; }
  .refresh { font-size: 18px; line-height: 1; padding: 2px 8px; }
  .refresh:disabled { opacity: 0.4; }
  .progress { position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; overflow: hidden; background: transparent; }
  .progress::after { content: ""; position: absolute; top: 0; bottom: 0; width: 35%; background: var(--accent); animation: slide 1.1s ease-in-out infinite; }
  @keyframes slide { from { left: -35%; } to { left: 100%; } }
  .rows { overflow-y: auto; flex: 1; }
  .row { display: block; width: 100%; text-align: left; border: none; border-bottom: 1px solid var(--line); border-radius: 0; background: none; padding: 9px 14px; border-left: 3px solid transparent; }
  .row:hover { background: var(--panel); }
  .row.sel { background: var(--sel); border-left-color: var(--accent); }
  .row .top { display: flex; justify-content: space-between; gap: 8px; }
  .from { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .time { color: var(--dim); font-size: 12px; flex: none; }
  .subj, .prev { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .subj { margin: 1px 0; }
  .prev { color: var(--dim); font-size: 12.5px; }
  .row.unread .from, .row.unread .subj { font-weight: 700; color: var(--fg); }
  .row.unread .time { color: var(--accent); }
  .flag { color: var(--danger); }
  .hint { color: var(--dim); text-align: center; padding: 24px; display: flex; gap: 10px; align-items: center; justify-content: center; }

  main { display: flex; flex-direction: column; min-width: 0; min-height: 0; background: var(--panel); }
  .toolbar { display: flex; gap: 6px; padding: 8px 14px; border-bottom: 1px solid var(--line); }
  .toolbar button.on { color: var(--danger); border-color: var(--danger); }
  .sp { flex: 1; }
  .head { padding: 14px 20px 10px; }
  .head h1 { margin: 0 0 10px; font-size: 20px; }
  .meta { display: flex; align-items: center; gap: 10px; }
  .to { color: var(--dim); font-size: 12.5px; }
  .atts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 10px; }
  .att { background: var(--panel2); border: 1px solid var(--line); border-radius: 6px; padding: 4px 10px; }
  .banner { margin-top: 10px; background: var(--panel2); border: 1px solid var(--line); border-radius: 6px; padding: 6px 10px; color: var(--dim); display: flex; gap: 10px; align-items: center; }
  iframe { flex: 1; border: none; margin: 0 20px 14px; border-radius: 8px; background: #fff; min-height: 120px; }
  .empty { margin: auto; text-align: center; color: var(--dim); display: grid; gap: 8px; max-width: 460px; }
  .logo { font-size: 44px; color: var(--accent); }
</style>
