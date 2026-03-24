ALTER TABLE jobs
ADD COLUMN embedding FLOAT4[];

CREATE INDEX idx_jobs_embedding_null
ON jobs (id)
WHERE embedding IS NULL;