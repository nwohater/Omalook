<script lang="ts">
  let { html = "", onchange }: { html?: string; onchange?: () => void } = $props();

  let frame = $state<HTMLIFrameElement>();
  let linkOpen = $state(false);
  let linkUrl = $state("https://");
  let savedRange: Range | null = null;

  // The editor lives in a script-less sandboxed frame: quoted third-party HTML can't execute here
  // and can't reach the app's own (privileged) page.
  const srcdoc = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:">
<style>
  html{background:#fff}
  body{margin:0;padding:14px 16px;font:14px/1.5 system-ui,sans-serif;color:#1b1b1f;min-height:calc(100vh - 28px);outline:none;word-wrap:break-word}
  blockquote{margin:0 0 0 .8ex;border-left:2px solid #c8c8d0;padding-left:1ex;color:#555}
  img{max-width:100%;height:auto} a{color:#1a56c7}
</style></head><body></body></html>`;

  const doc = () => frame?.contentDocument ?? null;

  function init() {
    const d = doc();
    if (!d) return;
    d.designMode = "on";
    d.body.innerHTML = html;
    d.addEventListener("input", () => onchange?.());
    try { d.execCommand("styleWithCSS", false, "false"); } catch { /* ignore */ }
    // start with the cursor above any quoted text
    d.defaultView?.focus();
    const sel = d.getSelection();
    if (sel) { sel.collapse(d.body, 0); }
  }

  export function getHtml() { return doc()?.body.innerHTML ?? ""; }
  export function isEmpty() { const b = doc()?.body; return !b || (!b.textContent?.trim() && !b.querySelector("img")); }
  export function focus() { frame?.contentWindow?.focus(); }

  function cmd(name: string, arg?: string) {
    doc()?.execCommand(name, false, arg);
    onchange?.();
    focus();
  }

  function openLink() {
    const sel = doc()?.getSelection();
    savedRange = sel && sel.rangeCount ? sel.getRangeAt(0).cloneRange() : null;
    linkUrl = "https://";
    linkOpen = true;
  }

  function applyLink() {
    const d = doc();
    linkOpen = false;
    if (!d || !/^(https?:\/\/|mailto:)/i.test(linkUrl.trim())) return;
    focus();
    const sel = d.getSelection();
    if (savedRange && sel) { sel.removeAllRanges(); sel.addRange(savedRange); }
    if (sel && sel.isCollapsed) d.execCommand("insertHTML", false, `<a href="${linkUrl.replace(/"/g, "&quot;")}">${linkUrl.replace(/</g, "&lt;")}</a>`);
    else d.execCommand("createLink", false, linkUrl.trim());
    onchange?.();
  }

  const keep = (e: MouseEvent) => e.preventDefault();
</script>

<div class="editor">
  <div class="bar" role="toolbar" tabindex="-1" onmousedown={keep}>
    <button title="Bold (Ctrl+B)" onclick={() => cmd("bold")}><b>B</b></button>
    <button title="Italic (Ctrl+I)" onclick={() => cmd("italic")}><i>I</i></button>
    <button title="Underline (Ctrl+U)" onclick={() => cmd("underline")}><u>U</u></button>
    <button title="Strikethrough" onclick={() => cmd("strikeThrough")}><s>S</s></button>
    <span class="sep"></span>
    <button title="Bulleted list" onclick={() => cmd("insertUnorderedList")}>• List</button>
    <button title="Numbered list" onclick={() => cmd("insertOrderedList")}>1. List</button>
    <button title="Quote" onclick={() => cmd("formatBlock", "blockquote")}>❝</button>
    <button title="Indent" onclick={() => cmd("indent")}>⇥</button>
    <button title="Outdent" onclick={() => cmd("outdent")}>⇤</button>
    <span class="sep"></span>
    <button title="Insert link" onclick={openLink}>🔗</button>
    <button title="Remove link" onclick={() => cmd("unlink")}>⛓‍💥</button>
    <button title="Clear formatting" onclick={() => cmd("removeFormat")}>Tx</button>
    <span class="sep"></span>
    <button title="Undo (Ctrl+Z)" onclick={() => cmd("undo")}>↶</button>
    <button title="Redo (Ctrl+Shift+Z)" onclick={() => cmd("redo")}>↷</button>
  </div>
  {#if linkOpen}
    <form class="link" onsubmit={(e) => { e.preventDefault(); applyLink(); }}>
      <input bind:value={linkUrl} placeholder="https://example.com" />
      <button class="primary" type="submit">Add link</button>
      <button type="button" onclick={() => { linkOpen = false; focus(); }}>Cancel</button>
    </form>
  {/if}
  <iframe bind:this={frame} title="Message body" sandbox="allow-same-origin" {srcdoc} onload={init}></iframe>
</div>

<style>
  .editor { display: flex; flex-direction: column; flex: 1; min-height: 0; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .bar { display: flex; gap: 2px; padding: 4px 6px; background: var(--panel2); border-bottom: 1px solid var(--line); flex-wrap: wrap; align-items: center; }
  .bar button { background: none; border: 1px solid transparent; padding: 3px 8px; min-width: 28px; }
  .bar button:hover { border-color: var(--line); background: var(--panel); }
  .sep { width: 1px; height: 18px; background: var(--line); margin: 0 4px; }
  .link { display: flex; gap: 6px; padding: 6px; background: var(--panel2); border-bottom: 1px solid var(--line); }
  iframe { flex: 1; border: none; background: #fff; min-height: 160px; }
</style>
