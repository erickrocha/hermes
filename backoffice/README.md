# Hermes Backoffice

React administration application for Hermes tenants, subscriptions, business plans, and users.

## Development

1. Copy `.env.example` to `.env.local` and adjust `VITE_API_URL` when the API is not running on `http://localhost:8080`.
2. Install dependencies with `npm install`.
3. Start the application with `npm run dev`.

The Rust API creates the initial SysAdmin from `SYSADMIN_EMAIL` and `SYSADMIN_PASSWORD`. TenantOwner accounts created in the backoffice must replace their temporary password on first access.

## Verification

- `npm run lint`
- `npm test`
- `npm run build`
