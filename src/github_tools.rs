use octocrab::{models::repos::RepoCommit, Octocrab};

pub async fn fetch_latest_commits(
    octocrab: &Octocrab,
    owner: &str,
    repo: &str,
    limit: usize,
) -> octocrab::Result<Vec<RepoCommit>> {
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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use mockall::mock;
//     use mockall::predicate::*;
//     use octocrab::models::commits::{Commit, CommitFile};

//     mock! {
//         Octocrab {}

//         #[async_trait::async_trait]
//         impl octocrab::Octocrab for Octocrab {
//             async fn repos(&self, owner: &str, repo: &str) -> octocrab::repos::RepoHandler;
//         }
//     }

//     mock! {
//         RepoHandler {}

//         #[async_trait::async_trait]
//         impl octocrab::repos::RepoHandler for RepoHandler {
//             fn list_commits(&self) -> octocrab::repos::ListCommitsBuilder;
//         }
//     }

//     mock! {
//         ListCommitsBuilder {}

//         #[async_trait::async_trait]
//         impl octocrab::repos::ListCommitsBuilder for ListCommitsBuilder {
//             fn per_page(self, per_page: u8) -> Self;
//             async fn send(self) -> octocrab::Result<octocrab::Page<RepoCommit>>;
//         }
//     }

//     #[tokio::test]
//     async fn test_fetch_latest_commits() {
//         let mut mock_octocrab = MockOctocrab::new();
//         let mut mock_repo_handler = MockRepoHandler::new();
//         let mut mock_list_commits_builder = MockListCommitsBuilder::new();

//         mock_octocrab
//             .expect_repos()
//             .with(eq("owner"), eq("repo"))
//             .return_once(|_, _| mock_repo_handler);

//         mock_repo_handler
//             .expect_list_commits()
//             .return_once(|| mock_list_commits_builder);

//         mock_list_commits_builder
//             .expect_per_page()
//             .with(eq(5u8))
//             .return_once(|_| mock_list_commits_builder);

//         mock_list_commits_builder.expect_send().return_once(|| {
//             Ok(octocrab::Page {
//                 items: vec![RepoCommit::default(); 5],
//                 next: None,
//                 prev: None,
//                 last: None,
//                 first: None,
//                 incomplete_results: Some(false),
//                 total_count: Some(5),
//             })
//         });

//         let result = fetch_latest_commits(&mock_octocrab, "owner", "repo", 5).await;
//         assert!(result.is_ok());
//         assert_eq!(result.unwrap().len(), 5);
//     }

//     #[test]
//     fn test_process_commit_changes() {
//         let commit = Commit {
//             files: Some(vec![
//                 CommitFile {
//                     filename: "file1.rs".to_string(),
//                     additions: 10,
//                     deletions: 5,
//                     ..Default::default()
//                 },
//                 CommitFile {
//                     filename: "file2.rs".to_string(),
//                     additions: 20,
//                     deletions: 15,
//                     ..Default::default()
//                 },
//             ]),
//             ..Default::default()
//         };

//         // Capture stdout
//         let output = std::io::Cursor::new(Vec::new());
//         let _stdout_guard = std::io::stdout().lock();
//         std::io::set_print(Some(Box::new(output.clone())));

//         process_commit_changes(&commit);

//         // Reset stdout
//         std::io::set_print(None);

//         let output = String::from_utf8(output.into_inner()).unwrap();
//         assert!(output.contains("File: file1.rs, Additions: 10, Deletions: 5"));
//         assert!(output.contains("File: file2.rs, Additions: 20, Deletions: 15"));
//     }
// }
