-- Migration: 001_create_user_providers
-- Description: Create the user_providers table to map external auth provider
--              identities (NHost UUIDs, atproto DIDs) to Hasura user UUIDs.
--              This is the foundation for dual-auth support during the
--              NHost → atproto migration.

CREATE TABLE public.user_providers (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    provider    text NOT NULL CHECK (provider IN ('nhost', 'atproto')),
    provider_id text NOT NULL,
    handle      text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (provider, provider_id)
);

COMMENT ON TABLE public.user_providers IS
    'Maps external auth provider identities to Hasura user UUIDs. '
    'Supports dual-auth during NHost → atproto migration.';

COMMENT ON COLUMN public.user_providers.provider IS
    'Auth provider name: ''nhost'' or ''atproto''';

COMMENT ON COLUMN public.user_providers.provider_id IS
    'Provider-specific user identifier: NHost UUID or atproto DID';

COMMENT ON COLUMN public.user_providers.handle IS
    'Optional human-readable handle (e.g. Bluesky handle for atproto)';

-- Index for fast lookups by the auth webhook (provider + provider_id)
CREATE INDEX idx_user_providers_lookup
    ON public.user_providers (provider, provider_id);

-- Index for finding all providers linked to a user
CREATE INDEX idx_user_providers_user_id
    ON public.user_providers (user_id);

-- Backfill existing NHost users so they have a provider entry.
-- This ensures the dual-auth webhook can resolve NHost users through
-- the same user_providers lookup path if needed.
INSERT INTO public.user_providers (user_id, provider, provider_id)
SELECT id, 'nhost', id::text
FROM auth.users
ON CONFLICT (provider, provider_id) DO NOTHING;
