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
2. **Important:** while the app is in *Testing*, Google expires its sign-in after **7 days**. To avoid re-signing in weekly, publish it:
   1. Open **Audience** and find **Publishing status** (it says *Testing*).
   2. Click **Publish app**, then **Confirm**. The status becomes *In production*.
   3. If you already connected an account while in Testing, remove it in Omalook and add it again so it gets a fresh sign-in.
   4. When you sign in, Google shows *"Google hasn't verified this app"*. This is expected for your own app: click the small **Advanced** link (bottom left), then **Go to Omalook (unsafe)**, and tick every permission box.

   Publishing doesn't list your app anywhere, but treat the client ID and secret as private: anyone who has them could sign in through your app (Google caps unverified apps at 100 users).

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
