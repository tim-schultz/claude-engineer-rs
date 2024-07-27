use anyhow::Result;
use dotenv::dotenv;
use octocrab::{models::repos::RepoCommit, Octocrab};

pub async fn fetch_latest_commits(
    owner: &str,
    repo: &str,
    sha: &str,
) -> octocrab::Result<RepoCommit> {
    dotenv().ok();
    let token =
        std::env::var("GITHUB_ACCESS_TOKEN").expect("GITHUB_ACCESS_TOKEN env variable is required");
    let octocrab = Octocrab::builder().personal_token(token.clone()).build()?;
    let commit = octocrab.commits(owner, repo).get(sha).await?;

    Ok(commit)
}

pub fn process_commit_changes(commit: RepoCommit) -> Result<String> {
    let mut result = String::new();
    for file in commit.files.unwrap() {
        result.push_str(&format!(
            "File: {file}, Additions: {additions}, Deletions: {deletions}, Patch: {patch}\n",
            file = file.filename,
            additions = file.additions,
            deletions = file.deletions,
            patch = file.patch.unwrap_or_default()
        ));
    }
    Ok(result)
}
