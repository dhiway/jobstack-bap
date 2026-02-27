CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE job_profile_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    job_id UUID NOT NULL,
    profile_id UUID NOT NULL,

    job_hash TEXT NOT NULL,
    profile_hash TEXT NOT NULL,

    match_score SMALLINT NOT NULL CHECK (match_score BETWEEN 0 AND 100),

    score_breakdown JSONB,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT fk_job
        FOREIGN KEY (job_id)
        REFERENCES jobs(id)
        ON DELETE CASCADE,

    CONSTRAINT fk_profile
        FOREIGN KEY (profile_id)
        REFERENCES profiles(id)
        ON DELETE CASCADE,

    CONSTRAINT uniq_job_profile UNIQUE (job_id, profile_id)
);

CREATE INDEX idx_jpm_profile_score_desc
ON job_profile_matches (profile_id, match_score DESC);

CREATE INDEX idx_jpm_job_score_desc
ON job_profile_matches (job_id, match_score DESC);

CREATE INDEX idx_jpm_profile_hash
ON job_profile_matches (profile_id, profile_hash);

CREATE INDEX idx_jpm_job_hash
ON job_profile_matches (job_id, job_hash);

CREATE INDEX idx_jpm_computed_at
ON job_profile_matches (computed_at);

CREATE UNIQUE INDEX IF NOT EXISTS idx_profiles_profile_id
ON profiles (profile_id);


CREATE INDEX idx_jobs_role_trgm
ON jobs
USING GIN ((beckn_structure #>> '{tags,role}') gin_trgm_ops);

CREATE INDEX idx_jobs_name_trgm
ON jobs
USING GIN ((beckn_structure #>> '{descriptor,name}') gin_trgm_ops);

CREATE INDEX idx_jobs_industry_trgm
ON jobs
USING GIN ((beckn_structure #>> '{tags,industry}') gin_trgm_ops);
