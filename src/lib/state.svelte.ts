import { api, type Account, type AppConfig, type Attachment } from "./api";

export type Mode = "mail" | "calendar" | "contacts";

export interface ComposeState {
  accountId: string;
  to: string; cc: string; bcc: string; subject: string;
  html: string;
  attachments: { name: string; path: string }[];
  existing: Attachment[];
  replyToId: string | null;
  replyAll: boolean;
  forwardOfId: string | null;
  draftId: string | null;
}

export const app = $state({
  ready: false,
  accounts: [] as Account[],
  activeId: "",
  mode: "mail" as Mode,
  settings: false,
  settingsTab: "accounts" as "accounts" | "microsoft" | "google" | "shortcuts",
  config: {} as AppConfig,
  compose: null as ComposeState | null,
  toast: "",
  toastAction: null as null | { label: string; run: () => void },
  error: "",
  /** bumped when something outside the mail view changed mail (e.g. a draft was saved) */
  mailRev: 0,
});

let toastTimer: ReturnType<typeof setTimeout>;

export function flash(msg: string, action?: { label: string; run: () => void }) {
  app.toast = msg;
  app.toastAction = action ?? null;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (app.toast = ""), action ? 6000 : 2500);
}

export function fail(e: unknown) {
  app.error = String(e);
}

export async function reloadAccounts() {
  app.accounts = await api.listAccounts();
  if (!app.accounts.find((a) => a.id === app.activeId)) app.activeId = app.accounts[0]?.id ?? "";
}

export function openCompose(init: Partial<ComposeState> = {}) {
  app.compose = {
    accountId: app.activeId,
    to: "", cc: "", bcc: "", subject: "", html: "",
    attachments: [], existing: [],
    replyToId: null, replyAll: false, forwardOfId: null, draftId: null,
    ...init,
  };
}

export const accountColor = (i: number) => ["#7aa2f7", "#9ece6a", "#e0af68", "#bb9af7", "#f7768e", "#73daca"][i % 6];
