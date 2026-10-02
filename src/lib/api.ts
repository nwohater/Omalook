import { invoke } from "@tauri-apps/api/core";

export type Kind = "microsoft" | "google";
export interface Account { id: string; kind: Kind; email: string; name: string }
export interface Addr { name: string; address: string }
export interface Folder { id: string; name: string; role: string; unread: number; total: number; depth: number }
export interface Attachment { id: string; name: string; size: number; contentType: string; inline: boolean }
export interface MsgSummary {
  id: string; subject: string; from: Addr; to: Addr[]; received: string;
  isRead: boolean; preview: string; hasAttachments: boolean; flagged: boolean;
}
export interface MsgDetail extends MsgSummary { cc: Addr[]; html: string; attachments: Attachment[] }
export interface MessagePage { items: MsgSummary[]; next: string | null }
export interface OutAttachment { name: string; path: string }
export interface Outgoing {
  to: string[]; cc: string[]; bcc: string[]; subject: string; html: string;
  attachments: OutAttachment[]; keepAttachments: string[];
  replyToId?: string | null; replyAll: boolean; forwardOfId?: string | null; draftId?: string | null;
}
export interface DraftData {
  draftId: string; to: string[]; cc: string[]; bcc: string[]; subject: string; html: string;
  attachments: Attachment[]; replyToId?: string | null;
}
export interface CalEvent {
  id: string; calendarId: string; calendarName: string; color: string; title: string;
  start: string; end: string; allDay: boolean; location: string; description: string;
  attendees: string[]; organizer: string; webLink?: string | null; joinUrl?: string | null;
}
export interface EventInput {
  id?: string | null; calendarId?: string | null; title: string; start: string; end: string;
  allDay: boolean; location: string; description: string | null; attendees: string[];
}
export interface CalendarInfo { id: string; name: string; color: string }
export interface Contact { id: string; etag?: string | null; name: string; emails: string[]; phones: string[]; company: string; title: string }
export interface AppConfig {
  microsoft?: { clientId: string; tenantId: string } | null;
  google?: { clientId: string; clientSecret: string } | null;
}

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (config: AppConfig) => invoke<void>("save_config", { config }),
  listAccounts: () => invoke<Account[]>("list_accounts"),
  addAccount: (kind: Kind) => invoke<Account>("add_account", { kind }),
  removeAccount: (id: string) => invoke<void>("remove_account", { id }),

  folders: (account: string) => invoke<Folder[]>("list_folders", { account }),
  messages: (account: string, folder: string, page?: string | null, search?: string | null) =>
    invoke<MessagePage>("list_messages", { account, folder, page: page ?? null, search: search || null }),
  message: (account: string, id: string) => invoke<MsgDetail>("get_message", { account, id }),
  setRead: (account: string, id: string, read: boolean) => invoke<void>("set_read", { account, id, read }),
  setFlag: (account: string, id: string, flagged: boolean) => invoke<void>("set_flag", { account, id, flagged }),
  move: (account: string, id: string, target: string, from?: string) =>
    invoke<void>("move_message", { account, id, target, from: from ?? null }),
  deleteForever: (account: string, id: string) => invoke<void>("delete_message", { account, id }),
  saveDraft: (account: string, out: Outgoing) => invoke<string>("save_draft", { account, out }),
  send: (account: string, out: Outgoing) => invoke<void>("send_mail", { account, out }),
  openDraft: (account: string, id: string) => invoke<DraftData>("open_draft", { account, id }),
  discardDraft: (account: string, draftId: string) => invoke<void>("discard_draft", { account, draftId }),
  download: (account: string, message: string, attachment: string, name: string) =>
    invoke<string>("download_attachment", { account, message, attachment, name }),
  openDownload: (path: string) => invoke<void>("open_download", { path }),

  calendars: (account: string) => invoke<CalendarInfo[]>("list_calendars", { account }),
  events: (account: string, from: string, to: string) => invoke<CalEvent[]>("list_events", { account, from, to }),
  saveEvent: (account: string, event: EventInput) => invoke<void>("save_event", { account, event }),
  deleteEvent: (account: string, calendarId: string, id: string) => invoke<void>("delete_event", { account, calendarId, id }),

  contacts: (account: string) => invoke<Contact[]>("list_contacts", { account }),
  saveContact: (account: string, contact: Contact) => invoke<void>("save_contact", { account, contact }),
  deleteContact: (account: string, id: string) => invoke<void>("delete_contact", { account, id }),

  themeColors: () => invoke<Record<string, string> | null>("theme_colors"),
};
