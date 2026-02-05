use crate::db::match_score::{
    fetch_all_jobs, fetch_all_profiles, fetch_job_by_id, fetch_new_jobs, fetch_new_profiles,
    fetch_profile_by_id, fetch_stale_matches, upsert_match_score, JobLiteRow, JobRow,
    ProfileLiteRow, ProfileRow, StaleMatchRow,
};
use crate::services::match_score::compute_match_score;
use crate::state::AppState;
use crate::utils::batching::chunk_vec;
use crate::utils::logging::format_duration;
use sqlx::PgPool;
use std::time::Instant;
use tracing::info;

pub async fn calculate_match_score(app_state: &AppState) {
    let start = Instant::now();
    info!("🔄 match-score cron started");

    let new_jobs = match fetch_new_jobs(&app_state.db_pool).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to fetch new jobs: {:?}", e);
            return;
        }
    };

    let new_profiles = match fetch_new_profiles(&app_state.db_pool).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to fetch new profiles: {:?}", e);
            return;
        }
    };

    let stale_matches = match fetch_stale_matches(&app_state.db_pool).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to fetch stale matches: {:?}", e);
            return;
        }
    };

    info!(
        "📊 match-score summary → new_jobs: {}, new_profiles: {}, stale_pairs: {}",
        new_jobs.len(),
        new_profiles.len(),
        stale_matches.len()
    );

    if new_jobs.is_empty() && new_profiles.is_empty() && stale_matches.is_empty() {
        info!("match-score: nothing to process");
        return;
    }
    /* ---------------- stale matches (batched) ---------------- */

    let batch_size = app_state.config.cron.compute_match_scores.batch.max(1);

    if !stale_matches.is_empty() {
        let stale_batches = chunk_vec(stale_matches, batch_size);
        let stale_total = stale_batches.len();

        info!(
            "🔁 recomputing stale matches in {} batches (batch_size={})",
            stale_total, batch_size
        );

        for (idx, batch) in stale_batches.into_iter().enumerate() {
            info!(
                "➡️ processing stale batch {}/{} ({} pairs)",
                idx + 1,
                stale_total,
                batch.len()
            );

            recompute_stale_matches(&app_state, batch).await;
        }
    }

    /* ---------------- new jobs ---------------- */

    if !new_jobs.is_empty() {
        let job_batches = chunk_vec(new_jobs, batch_size);
        let job_total = job_batches.len();

        info!(
            "🆕 processing new jobs in {} batches (batch_size={})",
            job_total, batch_size
        );

        for (idx, batch) in job_batches.into_iter().enumerate() {
            info!(
                "➡️ processing job batch {}/{} ({} jobs)",
                idx + 1,
                job_total,
                batch.len()
            );

            process_new_jobs(&app_state, batch).await;
        }
    }

    /* ---------------- new profiles ---------------- */

    if !new_profiles.is_empty() {
        let profile_batches = chunk_vec(new_profiles, batch_size);
        let profile_total = profile_batches.len();

        info!(
            "🆕 processing new profiles in {} batches (batch_size={})",
            profile_total, batch_size
        );

        for (idx, batch) in profile_batches.into_iter().enumerate() {
            info!(
                "➡️ processing profile batch {}/{} ({} profiles)",
                idx + 1,
                profile_total,
                batch.len()
            );

            process_new_profiles(&app_state, batch).await;
        }
    }

    let elapsed = start.elapsed();

    info!(
        "✅ match-score cron finished in {}",
        format_duration(elapsed)
    );
}

pub async fn recompute_stale_matches(app_state: &AppState, stale_matches: Vec<StaleMatchRow>) {
    for pair in stale_matches {
        let job = match fetch_job_by_id(&app_state.db_pool, pair.job_id).await {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("failed to fetch job {}: {:?}", pair.job_id, e);
                continue;
            }
        };

        let profile = match fetch_profile_by_id(&app_state.db_pool, pair.profile_id).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("failed to fetch profile {}: {:?}", pair.profile_id, e);
                continue;
            }
        };

        compute_and_upsert(app_state, &job, &profile, "stale").await;
        info!(
            "✅ finished stale match job={} profile={}",
            job.id, profile.id
        );
    }
}

pub async fn process_new_jobs(app_state: &AppState, new_jobs: Vec<JobLiteRow>) {
    if new_jobs.is_empty() {
        return;
    }

    let profiles = match fetch_all_profiles(&app_state.db_pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("failed to fetch profiles: {:?}", e);
            return;
        }
    };

    info!(
        "🆕 processing {} new jobs against {} profiles",
        new_jobs.len(),
        profiles.len()
    );

    for lite_job in new_jobs {
        let job = match fetch_job_by_id(&app_state.db_pool, lite_job.id).await {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("failed to fetch job {}: {:?}", lite_job.id, e);
                continue;
            }
        };

        for profile in &profiles {
            compute_and_upsert(app_state, &job, profile, "new_job").await;
        }

        info!("✅ finished job {}", job.id);
    }
}

pub async fn process_new_profiles(app_state: &AppState, new_profiles: Vec<ProfileLiteRow>) {
    if new_profiles.is_empty() {
        return;
    }

    let jobs = match fetch_all_jobs(&app_state.db_pool).await {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("failed to fetch jobs: {:?}", e);
            return;
        }
    };

    info!(
        "🆕 processing {} new profiles against {} jobs",
        new_profiles.len(),
        jobs.len()
    );

    for lite_profile in new_profiles {
        let profile = match fetch_profile_by_id(&app_state.db_pool, lite_profile.id).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("failed to fetch profile {}: {:?}", lite_profile.id, e);
                continue;
            }
        };

        for job in &jobs {
            compute_and_upsert(app_state, job, &profile, "new_profile").await;
        }

        info!("✅ finished profile {}", profile.id);
    }
}

async fn compute_and_upsert(
    app_state: &AppState,
    job: &JobRow,
    profile: &ProfileRow,
    source: &'static str,
) {
    let (score, breakdown) = compute_match_score(app_state, job, profile);

    if let Err(e) = upsert_match_score(
        &app_state.db_pool,
        job.id,
        profile.id,
        &job.hash,
        &profile.hash,
        score,
        breakdown,
    )
    .await
    {
        tracing::error!(
            source = source,
            job_id = %job.id,
            profile_id = %profile.id,
            error = ?e,
            "failed to upsert match score"
        );
    }
}
