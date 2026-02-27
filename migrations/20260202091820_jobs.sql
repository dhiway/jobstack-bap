CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    job_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,

    transaction_id TEXT NOT NULL,
    bpp_id TEXT NOT NULL,
    bpp_uri TEXT NOT NULL,

    metadata JSONB,
    beckn_structure JSONB,

    hash TEXT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_jobs_job_provider_unique
  ON jobs(job_id, provider_id);

CREATE UNIQUE INDEX idx_jobs_hash_unique
  ON jobs(hash);

CREATE INDEX idx_jobs_last_synced_at_not_null
  ON jobs (last_synced_at)
  WHERE last_synced_at IS NOT NULL;

CREATE INDEX idx_jobs_bpp_txn
  ON jobs (bpp_id, transaction_id);
