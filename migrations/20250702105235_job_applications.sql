-- Up

CREATE TABLE job_applications (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
     order_id TEXT NOT NULL UNIQUE,
    transaction_id TEXT NOT NULL,
    bpp_id TEXT NOT NULL,
    bpp_uri TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now(),
    modified_at TIMESTAMP WITH TIME ZONE DEFAULT now(),
    metadata JSONB
);

-- Automatically update modified_at on row update
CREATE OR REPLACE FUNCTION set_modified_at_timestamp()
RETURNS TRIGGER AS $$
BEGIN
   NEW.modified_at = now();
   RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_set_modified_at
BEFORE UPDATE ON job_applications
FOR EACH ROW
EXECUTE PROCEDURE set_modified_at_timestamp();

-- Indexes for performance
CREATE INDEX idx_job_applications_user_id ON job_applications(user_id);
CREATE INDEX idx_job_applications_job_id ON job_applications(job_id);
