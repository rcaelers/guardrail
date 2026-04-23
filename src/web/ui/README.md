# Guardrail · Crashdumps (SvelteKit)

Svelte 5 + SvelteKit + Tailwind port of the `Crashdumps.html` prototype in the parent project.

## Stack

- **Svelte 5** (runes: `$state`, `$derived`, `$effect`, `$props`)
- **SvelteKit 2** (file-based routes, server `load`, form actions, cookie session)
- **Tailwind CSS 3** (design tokens mirror the prototype's `aTheme` in `tailwind.config.js`)
- **TypeScript**

## Route map

```
/                                 → redirect (first product, or /admin, or /no-access)
/login                            sign-in form (fake — accepts any seeded email)
/logout                           POST clears the session cookie
/no-access                        friendly landing when you have no memberships

/p/[product]/crashes              product-scoped list + split-pane detail
/p/[product]/crashes/[id]         deep-linkable single-group view
/p/[product]/symbols              symbol store (filter · sort · upload · delete)
/p/[product]/settings             redirects to members
/p/[product]/settings/members     grant / change / revoke access
/p/[product]/settings/danger      delete product (types-the-name confirmation)

/admin                            redirects to /admin/users
/admin/users                      create · delete · promote/demote
/admin/products                   create · delete
```

Admin console is gated on `user.isAdmin`. Every product-scoped route checks
membership (or admin override) before loading.

## Structure

```
src/
  app.html              shell, loads Inter + JetBrains Mono
  app.css               Tailwind entry + scrollbar polish
  app.d.ts              App.Locals typing (user: User | null)
  hooks.server.ts       resolves locals.user from the session cookie
  lib/
    server/
      session.ts        cookie read/write/clear helpers (httpOnly, 30 days)
    adapters/
      types.ts          GuardrailAdapter interface + shared types
      mock.ts           in-memory dataset: crashes, users, products,
                        memberships, symbols
      http.ts           fetch()-based stub that matches the same contract
      index.ts          picks adapter from env.GUARDRAIL_API_URL
    stores/
      pane.svelte.ts    detail pane width + open (persists to localStorage)
    components/
      ProductSwitcher, UserMenu, RoleBadge
      GroupRow, SignalChip, StatusPill, Sparkline, Select, ThemeToggle
      detail/
        DetailPanel        header + tab strip (takes readOnly, canMerge)
        StackTab, ThreadsTab, ModulesTab, EnvTab,
        BreadcrumbsTab, LogsTab, UserContextTab,
        RelatedTab, NotesTab
    utils/format.ts     fmtDate, fmtInt
  routes/
    +layout.svelte           dark-mode-only chrome
    +layout.server.ts        session gate; loads user + accessible products
    +page.server.ts          smart redirect (see above)
    login/                   auth form + form action
    logout/                  sign-out form action
    no-access/               empty-state page
    admin/                   admin console (layout + users/products tabs)
    p/[product]/             product-scoped shell
      +layout.server.ts      product lookup + role resolution (403 if none)
      +layout.svelte         top bar with ProductSwitcher, tabs, UserMenu
      crashes/               list + detail
      symbols/               symbol store
      settings/              side-nav + members + danger
```

## Auth

This is a **fake auth** for development. The login form accepts any email
belonging to a seeded user and sets an httpOnly cookie (`gr_uid`) with that
user's id. `hooks.server.ts` resolves it to `locals.user` on every request.

Replace `/login/+page.server.ts` and `hooks.server.ts` with real auth when you
wire up the backend — the rest of the app only reads `locals.user` and
`adapter.signIn / getUser`.

## Roles

Per-product roles (enforced on the **server** — form actions check before calling the adapter):

| Role         | Read crashes | Triage / notes | Merge groups | Upload symbols | Delete symbols | Manage members | Delete product |
| ------------ | :----------: | :------------: | :----------: | :------------: | :------------: | :------------: | :------------: |
| `readonly`   | ✓            |                |              |                |                |                |                |
| `readwrite`  | ✓            | ✓              |              | ✓              |                |                |                |
| `maintainer` | ✓            | ✓              | ✓            | ✓              | ✓              | ✓              | ✓              |

Platform-wide administrators (`user.isAdmin`) additionally get:
- Access to `/admin/users` and `/admin/products`
- Override for member management / product deletion on any product

## Swap the data source

`src/lib/adapters/index.ts` picks the adapter. Wire a real backend:

```
# .env
GUARDRAIL_API_URL=https://api.example.com/v1
```

Or implement your own — conform to `GuardrailAdapter` in `types.ts` and export
it from `index.ts`. Every route loader and form action goes through
`adapter.*`; there are no hard-coded data paths.

## Data shape

`CrashGroup` is modeled on minidump-stackwalk JSON output. Each group carries:

- **Summary fields** (`signal`, `title`, `topFrame`, `file`, `line`, `address`, `platform`, `version`, `build`, counts, status, timestamps, plus `productId`) — the flat projection the list row and detail header render directly.
- **`dump`** (optional) — the raw report.
- **`derived`** (optional) — normalized facts pulled out of the dump.
- **UI-projected views** (`stack`, `threads`, `modules`, `env`) — flattened, renderable shapes so tab components stay simple.
- Plus the usual `occurrences`, `breadcrumbs`, `logs`, `notes`, `related`, `userDescription`.

`User`, `Product`, `Membership`, and `Symbol` live alongside in the mock
adapter and are seeded with sensible cross-references (you're a maintainer of
Guardrail, a readwrite on Harpoon, and readonly on Rivet).

## Run

```
npm install
npm run dev
```

Sign in with any seeded email on the login screen — the list suggests them.
Try `you@studio.co` (admin + Guardrail maintainer) for the fullest experience,
or `sofia@guardrail.co` (readonly on Guardrail) to see the gated UI.
