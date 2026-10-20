use git2::{Repository, Signature};
use local_review::git::RepositoryModel;
use std::fs;
use tempfile::tempdir;

fn commit(repository: &Repository, message: &str) {
    let mut index = repository.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repository.find_tree(tree_id).unwrap();
    let signature = Signature::now("Local Review", "review@example.test").unwrap();
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<_> = parent.iter().collect();
    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .unwrap();
}

#[test]
fn comparison_collects_changed_file_and_lines() {
    let directory = tempdir().unwrap();
    let repository = Repository::init(directory.path()).unwrap();
    fs::write(directory.path().join("demo.txt"), "before\n").unwrap();
    commit(&repository, "base");
    repository
        .branch(
            "feature",
            &repository.head().unwrap().peel_to_commit().unwrap(),
            false,
        )
        .unwrap();
    repository.set_head("refs/heads/feature").unwrap();
    repository.checkout_head(None).unwrap();
    fs::write(directory.path().join("demo.txt"), "after\nnew line\n").unwrap();
    commit(&repository, "feature");

    let model = RepositoryModel::open(Some(directory.path())).unwrap();
    let comparison = model.comparison("master", "feature", false).unwrap();
    assert_eq!(comparison.files.len(), 1);
    assert_eq!(comparison.files[0].path, "demo.txt");
    assert_eq!(comparison.files[0].additions, 2);
    assert_eq!(comparison.files[0].deletions, 1);

    let commits = model.commits_between("master", "feature").unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].title, "feature");
    assert_eq!(commits[0].short_id.len(), 8);
}

#[test]
fn comparison_normalizes_tabs_before_lines_reach_the_ui() {
    let directory = tempdir().unwrap();
    let repository = Repository::init(directory.path()).unwrap();
    fs::write(directory.path().join("demo.go"), "\tbefore\r\n").unwrap();
    commit(&repository, "base");
    repository
        .branch(
            "feature",
            &repository.head().unwrap().peel_to_commit().unwrap(),
            false,
        )
        .unwrap();
    repository.set_head("refs/heads/feature").unwrap();
    repository.checkout_head(None).unwrap();
    fs::write(directory.path().join("demo.go"), "\tafter\tvalue\r\n").unwrap();
    commit(&repository, "feature");

    let model = RepositoryModel::open(Some(directory.path())).unwrap();
    let comparison = model.comparison("master", "feature", false).unwrap();
    let contents: Vec<_> = comparison.files[0]
        .lines
        .iter()
        .map(|line| line.content.as_str())
        .collect();

    assert!(contents.iter().all(|line| !line.contains('\t')));
    assert!(contents.iter().all(|line| !line.ends_with('\r')));
    assert!(contents.contains(&"    after    value"));
}
