CREATE TABLE draft_applications (
    id SERIAL PRIMARY KEY,
    bpp_id TEXT NOT NULL,
    bpp_uri TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now(),
    modified_at TIMESTAMP WITH TIME ZONE DEFAULT now(),
    metadata JSONB,
    user_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    CONSTRAINT unique_bpp_user_job UNIQUE (bpp_id, user_id, job_id)
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
BEFORE UPDATE ON draft_applications
FOR EACH ROW
EXECUTE PROCEDURE set_modified_at_timestamp();

CREATE INDEX idx_draft_applications_user_job ON draft_applications (user_id, job_id, bpp_id);

