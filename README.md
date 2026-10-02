# Omalook

An Outlook-style email client for Linux, built for [Omarchy](https://omarchy.org). Mail, calendar and contacts for **Microsoft 365** and **Gmail** in one fast native-feeling app, with your Omarchy theme applied live.

Built with [Tauri 2](https://tauri.app) (Rust) and Svelte 5.

## Features

- **Multiple accounts** — Microsoft 365 / Outlook (Microsoft Graph) and Gmail / Google, side by side
- **Mail** — folder tree (nested folders, Gmail labels), search, unread counts, new-mail polling, flag / archive / junk / delete, keyboard-driven
- **Compose** — rich text (bold, lists, quotes, links), To/Cc/Bcc with contact autocomplete, attachments (picker or drag-and-drop), drafts, reply / reply all / forward (with attachments)
- **Reading** — remote images blocked until you allow them, inline `cid:` images, attachment download
- **Calendar** — day / week / month across all accounts, create / edit / delete events, invite attendees, join links
- **Contacts** — browse, search, create, edit, delete
- **Omarchy theme sync** — follows `~/.local/state/omarchy/current/theme/colors.toml` and updates when you switch themes
- **Private by design** — you register your own OAuth app, so there is no Omalook server; tokens live in your system keyring (Secret Service), with a private-file fallback

> **Status:** early. Microsoft 365 mail has been exercised against a live tenant. Gmail, calendar and contacts are implemented and unit-tested at the protocol level but have had less real-world use — please open issues.

## Install

Requirements (Arch / Omarchy): `rust`, `nodejs`, `npm`, `webkit2gtk-4.1`, and a Secret Service provider such as `gnome-keyring` (already present on Omarchy).

```bash
git clone git@github.com:nwohater/Omalook.git
cd Omalook
npm install
npm run tauri dev                       # run in development
npm run tauri build -- --no-bundle      # release binary: src-tauri/target/release/omalook
```

Then follow **Setting up your accounts** below (the same guide is built into the app under *Settings*).

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `j` / `k` | Next / previous message |
| `c` | New message |
| `r` / `a` / `Shift+F` | Reply / reply all / forward |
| `e` | Archive |
| `Del` or `#` | Delete |
| `!` | Mark as junk |
| `u` / `f` | Toggle read / flag |
| `/` | Search |
| `Enter` | Edit the selected draft |
| `Ctrl+Enter` / `Ctrl+S` / `Esc` | Composer: send / save draft / save and close |
| `Ctrl+1` `2` `3` | Mail / Calendar / Contacts |
| `Ctrl+,` | Settings |
| Calendar: `t` `d` `w` `m` `n` `←` `→` | Today, day, week, month, new event, previous, next |

## Where things are stored

| What | Where |
| --- | --- |
| OAuth client IDs you entered | `~/.config/omalook/config.json` (mode 600) |
| Account list | `~/.config/omalook/accounts.json` |
| Sign-in tokens | System keyring (service `omalook`); fallback `~/.local/share/omalook/tokens/` (mode 600) |

Nothing is sent anywhere except to Microsoft and Google's own APIs.

# Setting up your accounts

Each provider needs a one-time app registration so that *you* own the connection. Client IDs are not secrets, but they are yours — Omalook stores them locally and never ships any.

## Microsoft 365 / Outlook

Omalook talks to Microsoft 365 through the Microsoft Graph API. Microsoft requires every app to be registered once, which is free and takes about five minutes. You do this in your own tenant, so nothing goes through anyone else's servers.

> Works with work and school accounts (including resellers such as GoDaddy 365). If your organization blocks users from consenting to apps, ask your admin to grant consent in step 5.

### 1. Create the app registration

1. Sign in to **https://entra.microsoft.com** with an account that can register apps (an admin account, or any user if your tenant allows it).
2. Go to **Entra ID → App registrations → New registration**.
3. **Name:** `Omalook` (anything works).
4. **Supported account types:**
   - *Accounts in this organizational directory only* if you only need one organization's mail.
   - *Accounts in any organizational directory* if you want to add accounts from several organizations.
5. **Redirect URI:** choose **Public client/native (mobile & desktop)** and enter `http://localhost`.
6. Click **Register**.

### 2. Allow public client sign-in

1. In your new app, open **Authentication** (it may be labelled *Authentication (Preview)*).
2. Open the **Settings** tab (or scroll to *Advanced settings*).
3. Set **Allow public client flows** to **Yes** and **Save**.
4. On the **Redirect URI configuration** tab, check that `http://localhost` is listed under *Mobile and desktop applications*.

### 3. Add API permissions

1. Open **API permissions → Add a permission → Microsoft Graph → Delegated permissions**.
2. Tick all of these (use the filter box inside the panel):
   - `Mail.ReadWrite`
   - `Mail.Send`
   - `Calendars.ReadWrite`
   - `Contacts.ReadWrite`
   - `User.Read`
   - `offline_access`
3. Click **Add permissions**.

### 4. Copy your IDs

On the app's **Overview** page, copy:

- **Application (client) ID**
- **Directory (tenant) ID**

If you chose *any organizational directory* in step 1, enter `organizations` as the tenant ID instead (or `common` to also allow personal Microsoft accounts).

### 5. Grant consent (admins)

If you are an admin, click **Grant admin consent for _your organization_** on the API permissions page. Otherwise each user is asked to approve the permissions the first time they sign in, if your tenant allows it.

### 6. Connect

Paste the client ID and tenant ID into Omalook (**Settings → Accounts → Microsoft 365**), save, then click **Add Microsoft account**. Your browser opens the Microsoft sign-in page. When it says *Signed in*, return to Omalook.

### Troubleshooting

| Message | Fix |
| --- | --- |
| `AADSTS7000218` / *client_assertion or client_secret required* | **Allow public client flows** is not set to Yes (step 2). |
| `AADSTS50011` redirect URI mismatch | Add `http://localhost` as a *Mobile and desktop* redirect URI (step 1.5). |
| `AADSTS65001` consent required | An admin needs to grant consent (step 5). |
| `AADSTS700016` application not found | The tenant ID doesn't match the tenant that owns the app. |
| *Need admin approval* | Your organization restricts user consent. Ask an admin to grant it. |

## Gmail / Google

Omalook uses the Gmail, Google Calendar and People (Contacts) APIs. Google requires each app to have its own OAuth client, which is free. You create one in a Google Cloud project you own.

### 1. Create a project and enable the APIs

1. Go to **https://console.cloud.google.com** and create a new project (name it `Omalook`).
2. Open **APIs & Services → Library** and enable all three:
   - **Gmail API**
   - **Google Calendar API**
   - **People API**

### 2. Configure the consent screen

1. Open **Google Auth Platform** (or *APIs & Services → OAuth consent screen*) and click **Get started**.
2. **App name:** `Omalook`. **User support email:** your address.
3. **Audience:** choose **External** (choose **Internal** if you use Google Workspace and only need accounts from your own organization).
4. Add your email as the contact, then **Create**.

### 3. Add the permissions (scopes)

1. Open **Data Access → Add or remove scopes**.
2. Add these scopes, then **Save**:
   - `https://www.googleapis.com/auth/gmail.modify`
   - `https://www.googleapis.com/auth/calendar`
   - `https://www.googleapis.com/auth/contacts`
   - `openid`, `email`, `profile`

### 4. Add yourself as a test user, and publish

1. Open **Audience → Test users → Add users** and add every Gmail address you plan to connect.
2. **Important:** while the app is in *Testing*, Google expires its sign-in after **7 days**. To avoid re-signing in weekly, click **Publish app** (*In production*). The app stays private to you. Google shows an *unverified app* warning when you sign in, which is expected for your own app: click **Advanced → Go to Omalook (unsafe)**.

### 5. Create the OAuth client

1. Open **Clients → Create client**.
2. **Application type:** **Desktop app**. Name it `Omalook`.
3. Click **Create** and copy the **Client ID** and **Client secret**. (For desktop apps Google does not treat the secret as confidential.)

### 6. Connect

Paste the client ID and secret into Omalook (**Settings → Accounts → Google**), save, then click **Add Google account**. Your browser opens Google's sign-in page. Approve the requested access (tick every box) and return to Omalook.

### Notes

- Gmail labels appear as folders. **Archive** removes a message from the Inbox; it stays in *All Mail*.
- Omalook cannot permanently delete mail from Gmail (that needs a broader permission). Trashed mail is removed by Google after 30 days.

### Troubleshooting

| Message | Fix |
| --- | --- |
| `access_denied` / *app not verified* | Add your address as a test user (step 4) or publish the app, then choose *Advanced → Go to Omalook*. |
| `invalid_client` | Wrong client ID or secret, or the client type isn't *Desktop app*. |
| `Gmail API has not been used in project…` | Enable the Gmail, Calendar and People APIs (step 1). |
| *No refresh token* | Remove the account in Omalook, remove Omalook at https://myaccount.google.com/permissions, and add it again. |
| Signed out after a week | The app is still in *Testing* (step 4). Publish it. |

## Development

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # backend unit tests
npx svelte-check                                   # frontend type check
```

Layout:

- `src-tauri/src/ms.rs` — Microsoft Graph (mail, calendar, contacts)
- `src-tauri/src/google.rs` — Gmail, Google Calendar, People API
- `src-tauri/src/oauth.rs` — PKCE loopback sign-in shared by both
- `src-tauri/src/config.rs` — config, accounts, keyring token storage
- `src/lib/*.svelte` — UI (mail, calendar, contacts, composer, settings)
- `docs/` — the setup guides (rendered in-app and copied into this README)

See [ROADMAP.md](ROADMAP.md) for what's next.
