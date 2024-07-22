use octocrab::Octocrab;

pub async fn fetch_latest_commits(octocrab: &Octocrab, owner: &str, repo: &str, limit: usize) -> octocrab::Result<Vec<octocrab::models::commits::Commit>> {
    let commits = octocrab
        .repos(owner, repo)
        .list_commits()
        .per_page(limit as u8)
        .send()
        .await?;

    Ok(commits.items)
}

pub fn process_commit_changes(commit: &octocrab::models::commits::Commit) {
    if let Some(files) = &commit.files {
        for file in files {
            println!(
                "File: {file}, Additions: {additions}, Deletions: {deletions}",
                file = file.filename,
                additions = file.additions,
                deletions = file.deletions,
            );
        }
    }
}