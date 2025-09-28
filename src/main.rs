use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use octocrab::models::pulls::PullRequest;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Deserialize)]
struct Event {
    number: i64,
    pull_request: PullRequest,
}

const DEFAULT_SPAM_LABEL: &str = "Spam";
const SIX_MONTHS_AGO_MONTHS: i64 = 6;

#[tokio::main]
async fn main() -> Result<()> {
    let spam_label =
        env::var("INPUT_SPAM_LABEL").unwrap_or_else(|_| DEFAULT_SPAM_LABEL.to_string());
    let close_spam_prs = env::var("INPUT_CLOSE_SPAM_PRS").unwrap_or_else(|_| "yes".to_string());
    let access_token = env::var("INPUT_ACCESS_TOKEN").context("missing access_token")?;

    let octocrab = octocrab::Octocrab::builder()
        .personal_token(access_token)
        .build()
        .context("failed to create Octocrab client")?;

    let event_path = env::var("GITHUB_EVENT_PATH").context("missing GITHUB_EVENT_PATH")?;

    let file = File::open(&event_path)
        .with_context(|| format!("failed to open event file: {}", event_path))?;

    let reader = BufReader::new(file);
    let event: Event = serde_json::from_reader(reader).context("failed to parse event JSON")?;

    let repo_full = env::var("GITHUB_REPOSITORY").context("missing GITHUB_REPOSITORY")?;

    let (owner, repo) = repo_full
        .split_once('/')
        .context("invalid GITHUB_REPOSITORY format")?;

    let pr_num = event.number;
    let pr = &event.pull_request;

    if !pr
        .head
        .repo
        .as_ref()
        .is_some_and(|r| r.fork.unwrap_or(false))
    {
        println!("The pull request is not from a forked repository");
        return Ok(());
    }

    let pr = octocrab
        .pulls(owner, repo)
        .get(pr_num as u64)
        .await
        .with_context(|| format!("could not fetch PR {} in {}/{}", pr_num, owner, repo))?;

    let user_login = if let Some(user) = &pr.user {
        user.login.clone()
    } else {
        return Err(anyhow::format_err!("user has no login"));
    };

    // Get user info
    let user_url = format!("/users/{}", user_login);
    let user_info: serde_json::Value = octocrab
        .get(user_url, None::<&()>)
        .await
        .with_context(|| format!("could not fetch user {}", user_login))?;

    // Get user repos
    let repos_url = format!("/users/{}/repos", user_login);
    let user_repos: octocrab::Page<serde_json::Value> = octocrab
        .get(repos_url, Some(&serde_json::json!({"per_page": 100})))
        .await
        .with_context(|| format!("could not fetch {}'s repositories", user_login))?;

    let total_repos = user_repos.total_count.unwrap_or(0) as i64;
    let forks_count = user_repos
        .items
        .iter()
        .filter(|r| r.get("fork").and_then(|f| f.as_bool()).unwrap_or_default())
        .count() as i64;

    let pr_files_url = format!("/repos/{}/{}/pulls/{}/files", owner, repo, pr_num);
    let pr_files: Vec<serde_json::Value> = octocrab
        .get(pr_files_url, None::<&()>)
        .await
        .with_context(|| format!("could not fetch PR files in {}/#{}", owner, repo))?;

    let docs_only = pr_files.iter().all(|f| {
        if let Some(filename) = f.get("filename").and_then(|fn_| fn_.as_str()) {
            filename.ends_with(".md") || filename.ends_with(".txt") || filename.ends_with(".rst")
        } else {
            false
        }
    });

    let additions = pr.additions.unwrap_or(0) as i64;
    let deletions = pr.deletions.unwrap_or(0) as i64;

    // Check criteria 1: User registered in the last 6 months and has only forked repositories
    let six_months_ago = Utc::now() - Duration::days(30 * SIX_MONTHS_AGO_MONTHS);
    let user_created_at = user_info
        .get("created_at")
        .and_then(|t| t.as_str())
        .unwrap_or("1970-01-01T00:00:00Z");
    let user_registered_after_six_months =
        if let Ok(created) = user_created_at.parse::<chrono::DateTime<Utc>>() {
            created > six_months_ago
        } else {
            false
        };
    let has_only_forks = total_repos > 0 && total_repos == forks_count;

    if user_registered_after_six_months && has_only_forks {
        println!("::error {}/{}#{}: user registered in the last 6 months and has only forked repositories", owner, repo, pr_num);
        mark_as_spam(
            &octocrab,
            owner,
            repo,
            pr_num as u64,
            &spam_label,
            &close_spam_prs,
        )
        .await?;
        return Ok(());
    }

    // Check criteria 2: PR is changing documentation insignificantly
    if docs_only && (additions + deletions) < 10 {
        println!(
            "::error {}/{}#{}: pull request is changing documentation insignificantly",
            owner, repo, pr_num
        );
        mark_as_spam(
            &octocrab,
            owner,
            repo,
            pr_num as u64,
            &spam_label,
            &close_spam_prs,
        )
        .await?;
        return Ok(());
    }

    // Check criteria 3: PR consists of additions and deletions in a single file only
    if pr_files.len() == 1 && additions > 0 && deletions > 0 {
        println!(
            "::error {}/{}#{}: PR consists of additions and deletions in a single file only",
            owner, repo, pr_num
        );
        mark_as_spam(
            &octocrab,
            owner,
            repo,
            pr_num as u64,
            &spam_label,
            &close_spam_prs,
        )
        .await?;
        return Ok(());
    }

    Ok(())
}

async fn mark_as_spam(
    octocrab: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pr_num: u64,
    spam_label: &str,
    close_spam_prs: &str,
) -> Result<()> {
    // Try to add label (may exist already)
    let labels_url = format!("/repos/{}/{}/issues/{}/labels", owner, repo, pr_num);
    let _: Vec<serde_json::Value> = octocrab
        .post(labels_url, Some(&serde_json::json!([spam_label])))
        .await?;

    println!("marked {}/{}#{} as spam", owner, repo, pr_num);

    if close_spam_prs == "yes" {
        // Use raw HTTP request for closing PR to avoid API complexity
        let pr_url = format!("/repos/{}/{}/pulls/{}", owner, repo, pr_num);
        let _: serde_json::Value = octocrab
            .patch(pr_url, Some(&serde_json::json!({"state": "closed"})))
            .await?;

        println!("closed {}/{}#{}", owner, repo, pr_num);
    }

    Ok(())
}
