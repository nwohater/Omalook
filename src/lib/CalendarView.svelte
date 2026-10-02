<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type CalEvent, type CalendarInfo } from "./api";
  import { app, fail, flash, accountColor } from "./state.svelte";
  import { addDays, isoUtc, localInput, parseYmd, startOfDay, ymd } from "./format";

  type Ev = CalEvent & { accountId: string };
  type View = "day" | "week" | "month";
  interface Edit {
    id?: string; accountId: string; calendarId: string;
    title: string; allDay: boolean;
    startDate: string; startTime: string; endDate: string; endTime: string;
    location: string; description: string; origDescription: string; attendees: string;
    joinUrl?: string | null; webLink?: string | null; organizer?: string;
  }

  const HOUR = 48;
  const WEEK_START = 0; // Sunday

  let view = $state<View>("month");
  let cursor = $state(startOfDay(new Date()));
  let events = $state<Ev[]>([]);
  let hidden = $state<string[]>([]);
  let loading = $state(false);
  let edit = $state<Edit | null>(null);
  let saving = $state(false);
  let calendars = $state<CalendarInfo[]>([]);
  let scroller = $state<HTMLDivElement>();
  let now = $state(new Date());
  let epoch = 0;

  const startOfWeek = (d: Date) => addDays(startOfDay(d), -((d.getDay() - WEEK_START + 7) % 7));

  let range = $derived.by(() => {
    if (view === "day") return { from: cursor, to: addDays(cursor, 1) };
    if (view === "week") { const s = startOfWeek(cursor); return { from: s, to: addDays(s, 7) }; }
    const first = startOfWeek(new Date(cursor.getFullYear(), cursor.getMonth(), 1));
    return { from: first, to: addDays(first, 42) };
  });

  let days = $derived.by(() => {
    const n = view === "day" ? 1 : view === "week" ? 7 : 42;
    return Array.from({ length: n }, (_, i) => addDays(range.from, i));
  });

  let title = $derived(
    view === "month"
      ? cursor.toLocaleDateString([], { month: "long", year: "numeric" })
      : view === "week"
        ? `${range.from.toLocaleDateString([], { month: "short", day: "numeric" })} – ${addDays(range.to, -1).toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" })}`
        : cursor.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric", year: "numeric" }),
  );

  async function load() {
    const my = ++epoch;
    loading = true;
    const from = isoUtc(range.from), to = isoUtc(range.to);
    const results = await Promise.allSettled(app.accounts.map((a) => api.events(a.id, from, to)));
    if (my !== epoch) return;
    const all: Ev[] = [];
    results.forEach((r, i) => {
      if (r.status === "fulfilled") all.push(...r.value.map((e) => ({ ...e, accountId: app.accounts[i].id })));
      else fail(`${app.accounts[i].email}: ${r.reason}`);
    });
    events = all;
    loading = false;
  }

  $effect(() => {
    range.from; range.to; app.accounts;
    untrack(() => load());
  });

  onMount(() => {
    const t = setInterval(() => (now = new Date()), 60_000);
    scroller?.scrollTo({ top: 7.5 * HOUR });
    return () => clearInterval(t);
  });

  $effect(() => {
    view;
    untrack(() => queueMicrotask(() => scroller?.scrollTo({ top: 7.5 * HOUR })));
  });

  // ── geometry ─────────────────────────────────────────────────────

  const span = (e: Ev) => (e.allDay ? { s: parseYmd(e.start), e: parseYmd(e.end) } : { s: new Date(e.start), e: new Date(e.end) });
  let visible = $derived(events.filter((e) => !hidden.includes(e.accountId)));

  function onDay(day: Date, e: Ev) {
    const { s, e: en } = span(e);
    const ds = startOfDay(day), de = addDays(ds, 1);
    return s < de && (en > ds || (+en === +s && s >= ds));
  }

  const allDayOn = (d: Date) => visible.filter((e) => e.allDay && onDay(d, e));
  const timedOn = (d: Date) => visible.filter((e) => !e.allDay && onDay(d, e));

  function layout(d: Date) {
    const ds = startOfDay(d), de = addDays(ds, 1);
    const items = timedOn(d)
      .map((e) => {
        const { s, e: en } = span(e);
        const s0 = Math.max(+s, +ds), e0 = Math.min(Math.max(+en, +s + 15 * 60000), +de);
        return { e, s0, e0, col: 0, cols: 1 };
      })
      .sort((a, b) => a.s0 - b.s0 || b.e0 - a.e0);
    let cluster: typeof items = [], end = 0;
    const flush = () => { const n = Math.max(0, ...cluster.map((c) => c.col)) + 1; cluster.forEach((c) => (c.cols = n)); cluster = []; };
    for (const it of items) {
      if (cluster.length && it.s0 >= end) { flush(); end = 0; }
      const used = new Set(cluster.filter((c) => c.e0 > it.s0).map((c) => c.col));
      while (used.has(it.col)) it.col++;
      cluster.push(it);
      end = Math.max(end, it.e0);
    }
    flush();
    return items.map((it) => ({
      ...it,
      top: ((it.s0 - +ds) / 3600000) * HOUR,
      height: Math.max(18, ((it.e0 - it.s0) / 3600000) * HOUR - 2),
    }));
  }

  const hm = (iso: string) => new Date(iso).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const isToday = (d: Date) => d.toDateString() === now.toDateString();
  const idx = (id: string) => app.accounts.findIndex((a) => a.id === id);

  // ── navigation ───────────────────────────────────────────────────

  function step(dir: number) {
    if (view === "month") cursor = new Date(cursor.getFullYear(), cursor.getMonth() + dir, 1);
    else cursor = addDays(cursor, dir * (view === "week" ? 7 : 1));
  }

  function toggle(id: string) {
    hidden = hidden.includes(id) ? hidden.filter((h) => h !== id) : [...hidden, id];
  }

  // ── editing ──────────────────────────────────────────────────────

  async function loadCalendars(accountId: string) {
    calendars = [];
    try { calendars = await api.calendars(accountId); } catch { /* optional */ }
  }

  function newEvent(day: Date, hour = 9, allDay = false) {
    const s = new Date(day.getFullYear(), day.getMonth(), day.getDate(), Math.floor(hour), (hour % 1) * 60);
    const e = new Date(+s + 3600000);
    const accountId = app.activeId || app.accounts[0]?.id;
    edit = {
      accountId, calendarId: "", title: "", allDay,
      startDate: ymd(s), startTime: localInput(s).slice(11), endDate: ymd(e), endTime: localInput(e).slice(11),
      location: "", description: "", origDescription: "", attendees: "",
    };
    loadCalendars(accountId);
  }

  function openEvent(e: Ev) {
    const s = span(e);
    const endShown = e.allDay ? addDays(s.e, -1) : s.e;
    edit = {
      id: e.id, accountId: e.accountId, calendarId: e.calendarId, title: e.title, allDay: e.allDay,
      startDate: ymd(s.s), startTime: e.allDay ? "09:00" : localInput(s.s).slice(11),
      endDate: ymd(endShown), endTime: e.allDay ? "10:00" : localInput(s.e).slice(11),
      location: e.location, description: e.description, origDescription: e.description,
      attendees: e.attendees.join(", "), joinUrl: e.joinUrl, webLink: e.webLink, organizer: e.organizer,
    };
  }

  function slotClick(ev: MouseEvent, day: Date) {
    const el = ev.currentTarget as HTMLElement;
    const y = ev.clientY - el.getBoundingClientRect().top;
    newEvent(day, Math.floor((y / HOUR) * 2) / 2);
  }

  async function save() {
    const x = edit;
    if (!x) return;
    let start: string, end: string;
    if (x.allDay) {
      start = x.startDate;
      end = ymd(addDays(parseYmd(x.endDate < x.startDate ? x.startDate : x.endDate), 1));
    } else {
      const s = new Date(`${x.startDate}T${x.startTime}`), e = new Date(`${x.endDate}T${x.endTime}`);
      if (isNaN(+s) || isNaN(+e)) return fail("Enter a valid start and end.");
      if (e <= s) return fail("The event must end after it starts.");
      start = isoUtc(s); end = isoUtc(e);
    }
    saving = true;
    try {
      await api.saveEvent(x.accountId, {
        id: x.id ?? null, calendarId: x.calendarId || null, title: x.title || "(No title)", start, end, allDay: x.allDay,
        location: x.location,
        description: x.id && x.description === x.origDescription ? null : x.description,
        attendees: x.attendees.split(/[,;]/).map((a) => a.trim()).filter(Boolean),
      });
      edit = null;
      flash("Event saved");
      load();
    } catch (e) { fail(e); } finally { saving = false; }
  }

  async function remove() {
    const x = edit;
    if (!x?.id || !confirm("Delete this event?")) return;
    try {
      await api.deleteEvent(x.accountId, x.calendarId, x.id);
      edit = null;
      flash("Event deleted");
      load();
    } catch (e) { fail(e); }
  }

  function onKey(e: KeyboardEvent) {
    if (app.mode !== "calendar" || app.compose || app.settings) return;
    if (edit) { if (e.key === "Escape") edit = null; return; }
    if (/INPUT|TEXTAREA|SELECT/.test((e.target as HTMLElement).tagName) || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === "t") cursor = startOfDay(new Date());
    else if (e.key === "ArrowLeft") step(-1);
    else if (e.key === "ArrowRight") step(1);
    else if (e.key === "d") view = "day";
    else if (e.key === "w") view = "week";
    else if (e.key === "m") view = "month";
    else if (e.key === "n" || e.key === "c") { e.preventDefault(); newEvent(cursor); }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="cal">
  <header>
    <button onclick={() => (cursor = startOfDay(new Date()))}>Today</button>
    <button class="icon" onclick={() => step(-1)}>‹</button>
    <button class="icon" onclick={() => step(1)}>›</button>
    <h2>{title}</h2>
    {#if loading}<div class="spinner small"></div>{/if}
    <span class="sp"></span>
    {#each app.accounts as a, i}
      <button class="chip" class:off={hidden.includes(a.id)} onclick={() => toggle(a.id)} title="Show/hide {a.email}">
        <span class="dot" style:background={accountColor(i)}></span>{a.email}
      </button>
    {/each}
    <div class="seg">
      {#each ["day", "week", "month"] as const as v}
        <button class:on={view === v} onclick={() => (view = v)}>{v[0].toUpperCase() + v.slice(1)}</button>
      {/each}
    </div>
    <button class="primary" onclick={() => newEvent(cursor)}>＋ New event</button>
  </header>

  {#if view === "month"}
    <div class="month">
      <div class="dow">{#each days.slice(0, 7) as d}<div>{d.toLocaleDateString([], { weekday: "short" })}</div>{/each}</div>
      <div class="grid">
        {#each days as d}
          {@const evs = [...allDayOn(d), ...timedOn(d)]}
          <div class="cell" class:other={d.getMonth() !== cursor.getMonth()} class:today={isToday(d)} role="presentation" onclick={() => newEvent(d, 9)}>
            <button class="num" onclick={(e) => { e.stopPropagation(); cursor = d; view = "day"; }}>{d.getDate()}</button>
            {#each evs.slice(0, 3) as e}
              <button class="pill" class:allday={e.allDay} style:--c={e.color} onclick={(ev) => { ev.stopPropagation(); openEvent(e); }}>
                {#if !e.allDay}<small>{hm(e.start)}</small>{/if} {e.title}
              </button>
            {/each}
            {#if evs.length > 3}
              <button class="more" onclick={(e) => { e.stopPropagation(); cursor = d; view = "day"; }}>+{evs.length - 3} more</button>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {:else}
    <div class="tg-head" style:--n={days.length}>
      <div class="gutter"></div>
      {#each days as d}
        <div class="dh" class:today={isToday(d)}>
          <span>{d.toLocaleDateString([], { weekday: "short" })}</span>
          <b>{d.getDate()}</b>
          {#each allDayOn(d) as e}
            <button class="pill allday" style:--c={e.color} onclick={() => openEvent(e)}>{e.title}</button>
          {/each}
        </div>
      {/each}
    </div>
    <div class="tg-scroll" bind:this={scroller}>
      <div class="tg" style:--n={days.length} style:height="{24 * HOUR}px">
        <div class="gutter">
          {#each Array(24) as _, h}<div class="hour" style:top="{h * HOUR}px">{h === 0 ? "" : new Date(2000, 0, 1, h).toLocaleTimeString([], { hour: "numeric" })}</div>{/each}
        </div>
        {#each days as d}
          <div class="col" role="presentation" onclick={(e) => slotClick(e, d)}>
            {#each Array(24) as _, h}<div class="line" style:top="{h * HOUR}px"></div>{/each}
            {#each layout(d) as it}
              <button
                class="ev" style:--c={it.e.color}
                style:top="{it.top}px" style:height="{it.height}px"
                style:left="calc({(it.col / it.cols) * 100}% + 1px)" style:width="calc({100 / it.cols}% - 3px)"
                onclick={(ev) => { ev.stopPropagation(); openEvent(it.e); }}
              >
                <b>{it.e.title}</b>
                <small>{hm(it.e.start)}{it.e.location ? " · " + it.e.location : ""}</small>
              </button>
            {/each}
            {#if isToday(d)}<div class="now" style:top="{((now.getHours() * 60 + now.getMinutes()) / 60) * HOUR}px"></div>{/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

{#if edit}
  {@const x = edit}
  <div class="modal-bg" role="presentation" onclick={() => (edit = null)}>
    <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="mh"><b>{x.id ? "Edit event" : "New event"}</b><button class="icon" onclick={() => (edit = null)}>✕</button></div>
      <input class="t" placeholder="Add a title" bind:value={x.title} />
      <label class="chk"><input type="checkbox" bind:checked={x.allDay} /> All day</label>
      <div class="when">
        <input type="date" bind:value={x.startDate} />
        {#if !x.allDay}<input type="time" bind:value={x.startTime} />{/if}
        <span>→</span>
        {#if !x.allDay}<input type="time" bind:value={x.endTime} />{/if}
        <input type="date" bind:value={x.endDate} />
      </div>
      {#if !x.id}
        <div class="when">
          <select bind:value={x.accountId} onchange={() => loadCalendars(x.accountId)}>
            {#each app.accounts as a}<option value={a.id}>{a.email}</option>{/each}
          </select>
          {#if calendars.length > 1}
            <select bind:value={x.calendarId}>
              <option value="">Default calendar</option>
              {#each calendars as c}<option value={c.id}>{c.name}</option>{/each}
            </select>
          {/if}
        </div>
      {:else}
        <small class="dim">{app.accounts.find((a) => a.id === x.accountId)?.email}{x.organizer ? ` · organizer: ${x.organizer}` : ""}</small>
      {/if}
      <input placeholder="Location" bind:value={x.location} />
      <input placeholder="Invite people (emails, comma separated)" bind:value={x.attendees} />
      <textarea placeholder="Description" bind:value={x.description}></textarea>
      <div class="actions">
        <button class="primary" disabled={saving} onclick={save}>{saving ? "Saving…" : "Save"}</button>
        {#if x.joinUrl}<button onclick={() => openUrl(x.joinUrl!).catch(fail)}>🎥 Join</button>{/if}
        {#if x.webLink}<button onclick={() => openUrl(x.webLink!).catch(fail)}>Open in browser</button>{/if}
        <span class="sp"></span>
        {#if x.id}<button class="danger" onclick={remove}>🗑 Delete</button>{/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .cal { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--bg); }
  header { display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  header h2 { margin: 0 8px; font-size: 18px; min-width: 200px; }
  .sp { flex: 1; }
  .seg { display: flex; }
  .seg button { border-radius: 0; }
  .seg button:first-child { border-radius: 6px 0 0 6px; }
  .seg button:last-child { border-radius: 0 6px 6px 0; }
  .seg button.on { background: var(--sel); border-color: var(--accent); }
  .chip { display: inline-flex; align-items: center; gap: 6px; background: none; font-size: 12px; }
  .chip.off { opacity: 0.4; }
  .dot { width: 9px; height: 9px; border-radius: 50%; }

  .month { flex: 1; display: flex; flex-direction: column; min-height: 0; }
  .dow { display: grid; grid-template-columns: repeat(7, 1fr); text-align: center; color: var(--dim); padding: 6px 0; border-bottom: 1px solid var(--line); font-size: 12px; }
  .grid { flex: 1; display: grid; grid-template-columns: repeat(7, 1fr); grid-template-rows: repeat(6, minmax(0, 1fr)); min-height: 0; }
  .cell { border-right: 1px solid var(--line); border-bottom: 1px solid var(--line); padding: 2px 4px; overflow: hidden; display: flex; flex-direction: column; gap: 1px; cursor: default; }
  .cell.other { background: color-mix(in srgb, var(--panel) 55%, var(--bg)); color: var(--dim); }
  .cell.today { background: color-mix(in srgb, var(--accent) 10%, var(--bg)); }
  .num { align-self: flex-start; background: none; border: none; padding: 1px 6px; border-radius: 10px; font-size: 12px; }
  .cell.today .num { background: var(--accent); color: var(--accent-fg); font-weight: 700; }
  .pill { display: block; width: 100%; text-align: left; border: none; border-left: 3px solid var(--c); background: color-mix(in srgb, var(--c) 20%, var(--panel)); border-radius: 4px; padding: 1px 5px; font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pill.allday { background: var(--c); color: #111; }
  .pill small { color: var(--dim); }
  .more { background: none; border: none; color: var(--dim); font-size: 11px; text-align: left; padding: 0 4px; }

  .tg-head, .tg { display: grid; grid-template-columns: 56px repeat(var(--n), 1fr); }
  .tg-head { border-bottom: 1px solid var(--line); }
  .dh { padding: 6px 6px 4px; border-left: 1px solid var(--line); display: grid; gap: 2px; text-align: center; }
  .dh span { color: var(--dim); font-size: 11.5px; text-transform: uppercase; }
  .dh b { font-size: 20px; }
  .dh.today b { color: var(--accent); }
  .tg-scroll { flex: 1; overflow-y: auto; min-height: 0; }
  .tg { position: relative; }
  .gutter { position: relative; }
  .hour { position: absolute; right: 8px; transform: translateY(-50%); color: var(--dim); font-size: 11px; }
  .col { position: relative; border-left: 1px solid var(--line); }
  .line { position: absolute; left: 0; right: 0; border-top: 1px solid var(--line); opacity: 0.6; pointer-events: none; }
  .ev { position: absolute; text-align: left; overflow: hidden; border: none; border-left: 3px solid var(--c); background: color-mix(in srgb, var(--c) 28%, var(--panel)); border-radius: 5px; padding: 2px 5px; display: grid; align-content: start; font-size: 12px; }
  .ev b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ev small { color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .now { position: absolute; left: 0; right: 0; border-top: 2px solid var(--danger); pointer-events: none; }
  .now::before { content: ""; position: absolute; left: -4px; top: -5px; width: 8px; height: 8px; border-radius: 50%; background: var(--danger); }

  .modal-bg { position: fixed; inset: 0; background: #0009; display: grid; place-items: center; z-index: 50; }
  .modal { width: min(560px, 92vw); max-height: 92vh; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 14px; display: flex; flex-direction: column; gap: 8px; overflow-y: auto; }
  .mh { display: flex; justify-content: space-between; align-items: center; }
  .t { font-size: 17px; }
  .when { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .when input, .when select { width: auto; flex: 1; min-width: 110px; }
  .chk { display: flex; align-items: center; gap: 6px; }
  .chk input { width: auto; }
  textarea { min-height: 90px; resize: vertical; }
  .actions { display: flex; gap: 8px; align-items: center; }
  .danger:hover { border-color: var(--danger); color: var(--danger); }
  .dim { color: var(--dim); }
</style>
