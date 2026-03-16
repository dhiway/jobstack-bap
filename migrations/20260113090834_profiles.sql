CREATE TABLE profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  profile_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  type TEXT NOT NULL,
  metadata JSONB,
  beckn_structure JSONB,
  hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_synced_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_profiles_profile_id
  ON profiles(profile_id);

CREATE INDEX idx_profiles_user_id
  ON profiles(user_id);

CREATE INDEX idx_profiles_type
  ON profiles(type);

CREATE INDEX idx_profiles_metadata_gin
  ON profiles USING GIN (metadata);

CREATE INDEX idx_profiles_beckn_structure_gin
  ON profiles USING GIN (beckn_structure);

CREATE UNIQUE INDEX idx_profiles_hash_unique
  ON profiles(hash);

CREATE INDEX idx_profiles_last_synced_at_not_null
ON profiles (last_synced_at)
WHERE last_synced_at IS NOT NULL;

