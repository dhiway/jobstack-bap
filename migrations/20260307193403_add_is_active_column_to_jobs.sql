ALTER TABLE jobs
ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;

CREATE INDEX idx_jobs_is_active
ON jobs (is_active);

CREATE INDEX idx_jobs_active_only
ON jobs (id)
WHERE is_active = true;