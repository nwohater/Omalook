<script lang="ts">
  import type { Contact } from "./api";

  let {
    value = $bindable(""),
    label,
    contacts = [],
  }: { value: string; label: string; contacts: { name: string; address: string }[] | Contact[]; } = $props();

  type Sug = { name: string; address: string };
  let focused = $state(false);
  let active = $state(0);

  let pool = $derived<Sug[]>(
    (contacts as any[]).flatMap((c) => (c.address ? [{ name: c.name, address: c.address }] : (c.emails ?? []).map((e: string) => ({ name: c.name, address: e })))),
  );
  let token = $derived(value.split(/[,;]/).pop()!.trim().toLowerCase());
  let have = $derived(new Set(value.split(/[,;]/).map((t) => t.trim().toLowerCase())));
  let matches = $derived(
    token.length < 1
      ? []
      : pool.filter((s) => !have.has(s.address.toLowerCase()) && (s.address.toLowerCase().includes(token) || s.name.toLowerCase().includes(token))).slice(0, 6),
  );

  function pick(s: Sug) {
    const head = value.includes(",") || value.includes(";") ? value.replace(/[^,;]*$/, "").trimEnd() + " " : "";
    value = `${head}${s.address}, `;
    active = 0;
  }

  function onkey(e: KeyboardEvent) {
    if (!matches.length) return;
    if (e.key === "ArrowDown") { e.preventDefault(); active = (active + 1) % matches.length; }
    else if (e.key === "ArrowUp") { e.preventDefault(); active = (active - 1 + matches.length) % matches.length; }
    else if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); pick(matches[active]); }
    else if (e.key === "Escape") { e.stopPropagation(); focused = false; }
  }
</script>

<div class="row">
  <span class="lbl">{label}</span>
  <div class="box">
    <input bind:value onfocus={() => (focused = true)} onblur={() => setTimeout(() => (focused = false), 120)} onkeydown={onkey} oninput={() => (active = 0)} />
    {#if focused && matches.length}
      <div class="menu">
        {#each matches as m, i}
          <button class:on={i === active} onmousedown={(e) => { e.preventDefault(); pick(m); }}>
            <b>{m.name || m.address}</b>{#if m.name}<small>{m.address}</small>{/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .row { display: flex; align-items: center; gap: 8px; }
  .lbl { width: 34px; color: var(--dim); font-size: 12.5px; }
  .box { position: relative; flex: 1; }
  .menu { position: absolute; z-index: 20; top: 100%; left: 0; right: 0; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; margin-top: 2px; box-shadow: 0 8px 24px #0008; overflow: hidden; }
  .menu button { display: flex; gap: 10px; align-items: baseline; width: 100%; border: none; border-radius: 0; background: none; text-align: left; }
  .menu button.on, .menu button:hover { background: var(--sel); }
  small { color: var(--dim); }
</style>
