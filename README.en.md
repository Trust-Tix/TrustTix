# TrustTix

*Lee esto en [español](README.md).*

Event tickets that can't be counterfeited or scalped. Every ticket belongs to its owner in a Stellar smart contract: payment and ticket happen in the same transaction, each ticket gets in only once, and resale has a price cap. The money rules are in [docs/reglas-de-negocio.md](docs/reglas-de-negocio.md) and the manual test cases in [docs/casos-de-prueba.md](docs/casos-de-prueba.md) (both in Spanish).

**Live:** <https://trusttix.xyz> (landing), <https://app.trusttix.xyz> (buyers), <https://organizer.trusttix.xyz> (organizers) and <https://staff.trusttix.xyz> (door staff). Everything runs on Stellar **Testnet**: no real money is used.

**Contract v2 on Testnet:** [`CDKVPZI5HC25H6ILACGF24PNTTNYMUDQBSAJGMSUKEQEFUYVSAAKKRD4`](https://stellar.expert/explorer/testnet/contract/CDKVPZI5HC25H6ILACGF24PNTTNYMUDQBSAJGMSUKEQEFUYVSAAKKRD4)

## What the contract enforces

| Rule | How |
| --- | --- |
| No overselling | An event never sells more tickets than its capacity. |
| One owner per ticket | Only the current owner can transfer or resell it. |
| Pay and receive at once | Payment to the organizer, the TrustTix fee (3 %) and the ticket are a single transaction. If anything fails, nothing is charged. |
| Fair resale | Owners resell inside TrustTix up to a cap (110 % of the original price by default; each organizer can set 100 % to 150 %). Payment to the seller, the TrustTix fee (2 %) and the change of owner are one transaction. |
| One entry per ticket | Only the organizer or staff they authorized can check a ticket in, and only once. |
| Cancellation and refunds | A cancelled event can't sell, resell or check in tickets. The organizer refunds the original price to each ticket's current owner, once. |

Fees are stored in basis points, can only be changed by the TrustTix admin account and can never exceed 10 %.

## Requirements

**Frontend:**

- [Node.js](https://nodejs.org/) 20.19 or later (includes npm).
- Git.
- A Stellar wallet is optional: the app creates a TrustTix wallet on Testnet for each account (see [Wallets](#wallets)). If you prefer your own, Varial (formerly Freighter), xBull, Albedo, LOBSTR and others work, set to **Testnet**.

**Contract (optional; it is already deployed on Testnet):**

- [Rust](https://www.rust-lang.org/tools/install) 1.84 or later and the `wasm32v1-none` target.
- [Stellar CLI](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup).
- On Windows, the Visual Studio C++ Build Tools.

## Install

```bash
git clone https://github.com/Trust-Tix/TrustTix.git
cd TrustTix/frontend
npm install
```

For the contract:

```bash
rustup target add wasm32v1-none
```

## Run it

**Frontend:**

```bash
cd frontend
npm run dev
```

Before the first run, copy `frontend/.env.example` to `frontend/.env.local` and set your Google client ID in `VITE_GOOGLE_CLIENT_ID` (see [Google sign-in](#google-sign-in)).

Open <http://localhost:5173/?portal=app>, browse events, sign in with Google and buy. The frontend already points to the v2 contract on Testnet.

**Portals:** the same app is published at four addresses.

| Portal | Production | Local |
| --- | --- | --- |
| Landing: who we are and what we offer | trusttix.xyz | <http://localhost:5173> |
| Buyers: browse events and venues without an account; buy, keep and resell tickets with one | app.trusttix.xyz | <http://localhost:5173/?portal=app> |
| Organizers: create events and assign staff | organizer.trusttix.xyz | <http://localhost:5173/?portal=organizer> |
| Door staff: scan tickets in line | staff.trusttix.xyz | <http://localhost:5173/?portal=staff> |

Locally, the chosen portal is remembered while the tab stays open; to switch, open another address with `?portal=`.

Staff open their portal, connect a wallet and show their QR code; the organizer scans it from the event panel and signs to authorize it in the contract. At the door, the scanner camera stays open and reads one ticket after another.

**Contract tests:**

```bash
cd contracts
cargo test
```

**Build the contract:**

```bash
cd contracts
stellar contract build
```

**Deploy your own instance on Testnet (optional, PowerShell):**

```powershell
.\contracts\scripts\desplegar-v2.ps1 -Fuente my-account
.\contracts\scripts\sembrar.ps1
```

The first script builds, deploys and initializes the contract (XLM token, admin account and 3 % / 2 % fees) and saves its ID in `contracts/scripts/contrato-id.txt`. The second creates the sample events on that contract, with purchases, a check-in and tickets for resale, and logs the transaction links in `contracts/scripts/transacciones.md`. To use that instance in the frontend, add `VITE_CONTRATO_ID=<contract id>` to `frontend/.env.local` and run `npm run dev` again.

## Google sign-in

1. In [Google Cloud Console](https://console.cloud.google.com/), create a project and open **Google Auth Platform**.
2. Under **Branding**, fill in the app name and support email. Under **Audience**, choose **External** and publish the app.
3. Under **Clients**, create a **Web application** client and add these authorized JavaScript origins:
   `http://localhost`, `http://localhost:5173`, `https://trust-tix.vercel.app`, `https://trusttix.xyz`, `https://www.trusttix.xyz`, `https://app.trusttix.xyz`, `https://organizer.trusttix.xyz` and `https://staff.trusttix.xyz`.
4. Copy the client ID into `VITE_GOOGLE_CLIENT_ID` (in `frontend/.env.local` and in Vercel's environment variables).

The Google account only identifies the session; permissions inside TrustTix come from the wallet signature.

## Wallets

The wallet module ([frontend/src/stellar/billetera.js](frontend/src/stellar/billetera.js)) offers two ways to sign, and the rest of the app signs through the same function without knowing which one is active:

| Type | How it works |
| --- | --- |
| **TrustTix wallet** (recommended) | Created instantly for the Google account and funded with test XLM by Friendbot. It signs in the browser, so it works on phones without installing anything. It is stored with the account, so it is the same on any device or subdomain. It can be backed up or imported with its secret key. |
| **Other wallet** | The [Stellar Wallets Kit](https://github.com/Creit-Tech/Stellar-Wallets-Kit) picker: Varial (formerly Freighter), xBull, Albedo, LOBSTR, Hana, Rabet and more. It must be on Testnet. |

**Sponsored fees.** The Vercel function [frontend/api/patrocinar.js](frontend/api/patrocinar.js) wraps each contract call in a *fee bump* signed by a TrustTix account: people pay only for the ticket and TrustTix covers the network fee. It only sponsors calls to the TrustTix contract, capped at 1 XLM. It is enabled with the `TRUSTTIX_PATROCINADOR_SECRETA` environment variable (the secret key of the paying account, marked *Sensitive* in Vercel). Without it, or locally, the app sends the transaction directly and the fee comes from the wallet.

The **Wallet** page (`/billetera`, in all three portals) shows the balance, receives with a QR code, sends XLM with an optional memo (creating the destination account if needed) and lists movements, including ticket purchases. Account operations live in [frontend/src/stellar/cuenta.js](frontend/src/stellar/cuenta.js) and use Testnet Horizon.

> The TrustTix wallet stores its key in Supabase's `billeteras` table, readable only by its owner, and in the browser. Without a Supabase session it lives only in the browser. This is fine for Testnet but not for real money: whoever administers the Supabase project could read the keys. On Mainnet it would be replaced by a passkey or custodial wallet, and the app would use external wallets only.

## Database (Supabase)

The contract stores what defines a ticket. Each event's cover image, category, address, coordinates and description live in Supabase: an `eventos` table and a public `eventos` storage bucket.

1. Create a project in [Supabase](https://supabase.com/dashboard).
2. In the **SQL Editor**, run the files in [supabase/migrations](supabase/migrations) in order: `20261103000000_eventos.sql` creates the events table, its access rules and the bucket; `20261108000000_billeteras.sql` creates the table where each account keeps its TrustTix wallet; `20261108010000_total_billeteras.sql` creates the function that counts wallets for the landing page.
3. In **Authentication > Sign In / Providers > Google**, enable Google and paste the same client ID from `VITE_GOOGLE_CLIENT_ID` into **Client IDs**. That way the Google session also opens the Supabase one.
4. In **Project Settings > API**, copy the URL and the public key into `VITE_SUPABASE_URL` and `VITE_SUPABASE_ANON_KEY` (in `frontend/.env.local` and in Vercel).
5. Load the sample catalog. Take the `service_role` key from the same screen and use it only in the terminal:

   ```powershell
   cd frontend
   $env:SUPABASE_SERVICE_ROLE_KEY = "<service_role key>"
   npm run sembrar
   ```

   It uploads the images in [supabase/semilla/imagenes](supabase/semilla/imagenes) and the data in [supabase/semilla/catalogo.json](supabase/semilla/catalogo.json). Each entry is linked to the contract event with the same name, so run `sembrar.ps1` first. It can be run again; it updates what already exists.

Anyone can read the data. Registering a new event's data requires a Google session, and only whoever registered it can change it. New covers must be at least 1600 px wide; the app stores them as WebP up to 2400 px.

## Location

The app asks for location only when the person taps **Use my location**. With it, events are sorted by distance and the venue map is shown. Addresses are geocoded with [Nominatim](https://nominatim.org/) from OpenStreetMap, without a key and limited to one request per second.

## License

[MIT](LICENSE)
