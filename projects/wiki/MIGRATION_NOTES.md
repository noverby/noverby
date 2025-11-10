# Next.js to react-router-dom Migration

## Summary
Successfully migrated the RadikalWiki project from Next.js to react-router-dom.

## Key Changes

### 1. Dependencies
- **Removed**: `next`, `@nhost/nextjs`, `@mui/material-nextjs`
- **Kept**: `@nhost/react`, `react-router-dom` (already installed)

### 2. Routing
- Replaced Next.js file-based routing with react-router-dom
- Updated `App.tsx` to use `<BrowserRouter>` and define routes
- All routes defined in `src/App.tsx`

### 3. Hooks Migration
- `useRouter()` → `useNavigate()`, `useLocation()`, `useSearchParams()`
- `router.push()` → `navigate()`
- `router.back()` → `navigate(-1)`
- `router.reload()` → `window.location.reload()`
- `router.query` → `searchParams.get()`
- `router.asPath` → `location.pathname`

### 4. Core Hooks Updated
- `usePath.ts`: Uses `useLocation()` instead of `useRouter()`
- `usePathList.ts`: Uses `useParams()` to extract wildcard path
- `useLink.ts`: Uses `useNavigate()` for navigation
- `useApps.ts`: Uses `useSearchParams()` for query params
- `useScreen.ts`: Uses `useSearchParams()` to check app param

### 5. Components Updated
- `Image.tsx`: Replaced `next/image` with standard `<img>` tag
- `Link.tsx`: Replaced `next/link` with react-router-dom `Link`
- All authentication flows in `AuthForm.tsx`
- Layout and navigation components
- `BreadCrumbs.tsx`, `UserMenu.tsx`, `UnknownApp.tsx`, etc.

### 6. Provider Changes
- `NhostProvider` now from `@nhost/react` instead of `@nhost/nextjs`
- All nhost hooks (`useAuthenticationStatus`, `useUserId`, etc.) now from `@nhost/react`

### 7. Configuration Files
- **Deleted**: `next.config.ts`, `next-env.d.ts`
- **Updated**: `tsconfig.json` (removed Next.js plugin, changed jsx to react-jsx)
- **Updated**: `rsbuild.config.ts` (added entry point, historyApiFallback)
- **Created**: `public/index.html` (HTML template for rsbuild)

### 8. Deleted Files
- `src/pages/_app.tsx` (logic moved to `src/App.tsx`)
- `src/pages/_document.tsx` (replaced by `public/index.html`)
- `src/pages/_error.tsx`
- `src/pages/404.tsx`

### 9. Router Setup
Routes are now explicitly defined in `src/App.tsx`:
- `/` - Home page
- `/user/login` - Login page
- `/user/register` - Registration page
- `/user/reset-password` - Password reset
- `/user/set-password` - Set new password
- `/user/unverified` - Email verification page
- `/*` - Catch-all for dynamic paths (wiki content)

## Build System
The project now uses rsbuild as the build tool (was already configured).
- Entry point: `src/index.tsx`
- HTML template: `public/index.html`
- History API fallback enabled for client-side routing

## Testing Recommendations
1. Test all authentication flows (login, register, password reset)
2. Verify navigation throughout the app
3. Check that breadcrumb navigation works
4. Test deep linking and URL sharing
5. Verify search params are preserved
6. Test browser back/forward navigation
