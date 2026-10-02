import type { Addr } from "./api";

export const addrName = (a?: Addr) => a?.name || a?.address || "(unknown)";

export function when(iso: string) {
  const d = new Date(iso), n = new Date();
  if (d.toDateString() === n.toDateString()) return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (d.getFullYear() === n.getFullYear()) return d.toLocaleDateString([], { month: "short", day: "numeric" });
  return d.toLocaleDateString([], { year: "2-digit", month: "numeric", day: "numeric" });
}

export function fileSize(b: number) {
  return b > 1048576 ? (b / 1048576).toFixed(1) + " MB" : Math.max(1, Math.round(b / 1024)) + " KB";
}

export const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Pull plain addresses out of "a@b, Name <c@d>; e@f". */
export function splitAddrs(s: string): string[] {
  return s
    .split(/[,;]/)
    .map((t) => t.trim())
    .map((t) => /<([^>]+)>/.exec(t)?.[1] ?? t)
    .filter(Boolean);
}

export const esc = (t: string) => t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

/** Strip anything executable from third-party HTML before quoting it in a reply. */
export function cleanQuote(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("script,iframe,object,embed,meta,link,base,form,style,title").forEach((n) => n.remove());
  doc.body.querySelectorAll("*").forEach((el) => {
    for (const a of [...el.attributes]) {
      if (/^on/i.test(a.name) || (/^(href|src|action|formaction)$/i.test(a.name) && /^\s*javascript:/i.test(a.value))) el.removeAttribute(a.name);
    }
  });
  return doc.body.innerHTML;
}

const pad = (n: number) => String(n).padStart(2, "0");
export const ymd = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const localInput = (d: Date) => `${ymd(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
export const isoUtc = (d: Date) => d.toISOString().replace(/\.\d{3}Z$/, "Z");
export const addDays = (d: Date, n: number) => { const x = new Date(d); x.setDate(x.getDate() + n); return x; };
export const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
export const parseYmd = (s: string) => { const [y, m, d] = s.split("-").map(Number); return new Date(y, m - 1, d); };
