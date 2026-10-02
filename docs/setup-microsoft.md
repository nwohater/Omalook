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
