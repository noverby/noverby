# Router Migration Complete ✅

The wiki project has been successfully migrated from Next.js to react-router-dom.

## What Changed

### Routing Library
- **Before**: Next.js (file-based routing + `next/router`)
- **After**: react-router-dom (explicit route definitions)

### Key Files Modified
- ✅ Core hooks: `usePath`, `usePathList`, `useLink`, `useApps`, `useScreen`
- ✅ Main app structure: `src/App.tsx` now handles all routing
- ✅ All components using `useRouter` updated to use react-router-dom hooks
- ✅ Nhost integration switched from `@nhost/nextjs` to `@nhost/react`
- ✅ Image component uses standard `<img>` instead of `next/image`
- ✅ Link component uses react-router-dom `Link`

### Files Removed
- ❌ `next.config.ts`
- ❌ `next-env.d.ts`
- ❌ `src/pages/_app.tsx`
- ❌ `src/pages/_document.tsx`
- ❌ `src/pages/_error.tsx`
- ❌ `src/pages/404.tsx`

### Package Changes
```json
Removed:
- "next": "^15.3.5"
- "@nhost/nextjs": "^2.1.17"
- "@mui/material-nextjs": "^7.0.0"

Kept (already installed):
- "react-router-dom": "^7.9.5"
- "@nhost/react": "^3.11.2"
```

## Development

The project now uses **rsbuild** for building and development:

```bash
# Install dependencies (if needed)
npm install

# Development
npm run dev

# Build
npm run build
```

## Router API Changes

### Navigation
```typescript
// Before (Next.js)
import { useRouter } from 'next/router';
const router = useRouter();
router.push('/path');
router.back();

// After (react-router-dom)
import { useNavigate } from 'react-router-dom';
const navigate = useNavigate();
navigate('/path');
navigate(-1);
```

### Query Parameters
```typescript
// Before
router.query.app

// After
import { useSearchParams } from 'react-router-dom';
const [searchParams] = useSearchParams();
searchParams.get('app')
```

### Path Access
```typescript
// Before
router.asPath

// After
import { useLocation } from 'react-router-dom';
const location = useLocation();
location.pathname
```

## Notes

- All functionality should work the same as before
- Client-side routing is configured with `historyApiFallback` in rsbuild
- Deep linking and URL sharing continues to work
- Browser back/forward navigation is handled by react-router-dom

See `MIGRATION_NOTES.md` for detailed technical changes.
