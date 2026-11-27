use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Repo(PathBuf);

impl Repo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pi-git-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        let repo = Self(path);
        repo.git(&["init", "-b", "main"]);
        repo.git(&["config", "user.name", "Fixture"]);
        repo.git(&["config", "user.email", "fixture@example.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }
    fn git(&self, args: &[&str]) -> String {
        run(&self.0, &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>()).unwrap()
    }
    fn write(&self, path: &str, text: &str) { std::fs::write(self.0.join(path), text).unwrap(); }
    fn commit(&self) { self.git(&["add", "-A"]); self.git(&["commit", "-m", "fixture"]); }
    fn file(&self, path: &str) -> GitFile {
        snapshot(&self.0).unwrap().status.files.into_iter().find(|f| f.path == path).unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) { std::fs::remove_dir_all(&self.0).unwrap(); }
}

#[test]
fn unborn_branch_stage_and_unstage_preserve_files() {
    let repo = Repo::new();
    assert_eq!(snapshot(&repo.0).unwrap().status.branch, "main");
    repo.write("a -> b.txt", "new\n");
    repo.git(&["add", "-A"]);
    let file = repo.file("a -> b.txt");
    assert!(file.staged());
    unstage(&repo.0, Some(&file)).unwrap();
    assert!(repo.file("a -> b.txt").untracked());
    assert_eq!(std::fs::read_to_string(repo.0.join("a -> b.txt")).unwrap(), "new\n");
    repo.git(&["add", "-A"]);
    unstage(&repo.0, None).unwrap();
    assert_eq!(snapshot(&repo.0).unwrap().status.staged_count(), 0);
}

#[test]
fn quoted_and_nul_paths_do_not_split_arrows_in_filenames() {
    for text in ["## No commits yet on main\n?? \"a -> b.txt\"\n", "## No commits yet on main\0?? a -> b.txt\0"] {
        let status = parse_status(text);
        assert_eq!(status.branch, "main");
        assert_eq!(status.files[0].path, "a -> b.txt");
        assert_eq!(status.files[0].old_path, None);
    }
    let status = parse_status("## main\nR  \"old -> file\" -> \"new -> file\"\n");
    assert_eq!(status.files[0].path, "new -> file");
    assert_eq!(status.files[0].old_path.as_deref(), Some("old -> file"));
    let status = parse_status("## main\0R  new -> file\0old -> file\0");
    assert_eq!(status.files[0].old_path.as_deref(), Some("old -> file"));
}

#[test]
fn unicode_whitespace_and_literal_pathspecs_work() {
    let repo = Repo::new();
    for name in ["a -> b.txt", "ação.txt", "a\nb.txt", " space ", "*.txt"] { repo.write(name, "before\n"); }
    repo.commit();
    for name in ["a -> b.txt", "ação.txt", "a\nb.txt", " space ", "*.txt"] {
        repo.write(name, "after\n");
        let file = repo.file(name);
        assert!(run(&repo.0, &diff_args(&file, false)).unwrap().contains("+after"));
    }
    repo.git(&["add", "--", "*.txt"]);
    assert_eq!(snapshot(&repo.0).unwrap().status.staged_count(), 1);
}

#[test]
fn detached_partial_rename_and_no_color_are_reported() {
    let repo = Repo::new();
    repo.write("before.txt", "one\n"); repo.commit();
    repo.git(&["mv", "before.txt", "a -> b.txt"]);
    let renamed = repo.file("a -> b.txt");
    assert_eq!(renamed.old_path.as_deref(), Some("before.txt"));
    unstage(&repo.0, Some(&renamed)).unwrap();
    assert_eq!(snapshot(&repo.0).unwrap().status.staged_count(), 0);
    repo.git(&["add", "-A"]); repo.git(&["commit", "-m", "rename"]);
    repo.write("a -> b.txt", "two\n"); repo.git(&["add", "-A"]); repo.write("a -> b.txt", "three\n");
    let file = repo.file("a -> b.txt");
    assert_eq!(file.label(), "partial");
    repo.git(&["config", "color.ui", "always"]);
    assert!(!run(&repo.0, &diff_args(&file, false)).unwrap().contains('\x1b'));
    repo.git(&["restore", "--worktree", "--", "a -> b.txt"]); unstage(&repo.0, None).unwrap();
    repo.git(&["switch", "--detach"]);
    assert_eq!(snapshot(&repo.0).unwrap().status.branch, "detached");
    assert!(parse_status("## HEAD (no branch)\n").detached);
}

#[test]
fn discard_stashes_selected_content_and_can_be_recovered() {
    let repo = Repo::new();
    repo.write("one", "base\n"); repo.write("two", "base\n"); repo.commit();
    repo.write("one", "recover me\n"); repo.write("two", "leave me\n");
    let message = stash_discard(&repo.0, &repo.file("one")).unwrap();
    assert!(message.contains("stash@{0}"));
    assert_eq!(std::fs::read_to_string(repo.0.join("one")).unwrap(), "base\n");
    assert_eq!(std::fs::read_to_string(repo.0.join("two")).unwrap(), "leave me\n");
    assert!(repo.git(&["stash", "list"]).contains("discard/one/"));
    repo.git(&["stash", "apply", "stash@{0}"]);
    assert_eq!(std::fs::read_to_string(repo.0.join("one")).unwrap(), "recover me\n");
}

#[test]
fn discard_preserves_partial_index_and_untracked_content() {
    let repo = Repo::new();
    repo.write("one", "base\n"); repo.commit();
    repo.write("one", "staged\n"); repo.git(&["add", "one"]); repo.write("one", "unstaged\n");
    stash_discard(&repo.0, &repo.file("one")).unwrap();
    assert_eq!(repo.git(&["show", ":one"]), "staged\n");
    assert_eq!(std::fs::read_to_string(repo.0.join("one")).unwrap(), "staged\n");
    repo.write("new file", "recover new\n");
    stash_discard(&repo.0, &repo.file("new file")).unwrap();
    assert!(!repo.0.join("new file").exists());
    assert_eq!(repo.git(&["show", "stash@{0}^3:new file"]), "recover new\n");
}

fn conflict(repo: &Repo, rebase: bool) {
    repo.write("conflict", "base\n"); repo.commit();
    repo.git(&["switch", "-c", "other"]);
    repo.write("conflict", "other\n"); repo.commit();
    repo.git(&["switch", "main"]);
    repo.write("conflict", "main\n"); repo.commit();
    assert!(run(&repo.0, &[if rebase { "rebase".into() } else { "merge".into() }, "other".into()]).is_err());
}

#[test]
fn merge_and_rebase_conflicts_have_abort_and_continue_paths() {
    for rebase in [false, true] {
        let repo = Repo::new();
        conflict(&repo, rebase);
        let snapshot = snapshot(&repo.0).unwrap();
        assert_eq!(snapshot.operation, Some(if rebase { "rebase" } else { "merge" }));
        assert_eq!(snapshot.status.files[0].label(), "conflict");
        assert!(!snapshot.status.files[0].staged());
        repo.git(&[if rebase { "rebase" } else { "merge" }, "--abort"]);
        assert!(super::snapshot(&repo.0).unwrap().operation.is_none());
        assert!(run(&repo.0, &[if rebase { "rebase".into() } else { "merge".into() }, "other".into()]).is_err());
        repo.write("conflict", "resolved\n"); repo.git(&["add", "conflict"]);
        assert!(!super::snapshot(&repo.0).unwrap().status.files.iter().any(GitFile::conflicted));
        repo.git(&[if rebase { "rebase" } else { "merge" }, "--continue"]);
        assert!(super::snapshot(&repo.0).unwrap().operation.is_none());
    }
}

#[test]
fn publish_fetch_and_divergent_pull_use_a_local_remote() {
    let remote = Repo::new();
    let bare = remote.0.join("origin.git");
    remote.git(&["init", "--bare", bare.to_str().unwrap()]);
    let repo = Repo::new();
    repo.write("one", "base\n"); repo.commit();
    repo.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    run(&repo.0, &publish_args("main")).unwrap();
    assert_eq!(snapshot(&repo.0).unwrap().status.upstream, "origin/main");
    let other = Repo::new();
    other.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    other.git(&["fetch", "origin"]); other.git(&["switch", "-C", "main", "origin/main"]);
    other.write("remote", "remote\n"); other.commit(); other.git(&["push", "origin", "main"]);
    repo.write("local", "local\n"); repo.commit(); repo.git(&["fetch"]);
    let before = repo.git(&["rev-parse", "HEAD"]);
    assert!(run(&repo.0, &["pull".into(), "--ff-only".into()]).is_err());
    assert_eq!(before, repo.git(&["rev-parse", "HEAD"]));
    assert!(snapshot(&repo.0).unwrap().operation.is_none());
    run(&repo.0, &switch_args("new-branch", true).unwrap()).unwrap();
    run(&repo.0, &publish_args("new-branch")).unwrap();
    assert_eq!(snapshot(&repo.0).unwrap().status.upstream, "origin/new-branch");
}

#[test]
fn untracked_diff_and_invalid_branch_do_not_mutate_files() {
    let repo = Repo::new();
    repo.write("a -> b.txt", "new file\n");
    let file = repo.file("a -> b.txt");
    assert!(run(&repo.0, &diff_args(&file, false)).unwrap().contains("+new file"));
    assert!(switch_args("--force", false).is_err());
    assert!(stash_discard(&repo.0, &file).is_err()); // Git cannot stash an unborn tree.
    assert_eq!(std::fs::read_to_string(repo.0.join(&file.path)).unwrap(), "new file\n");
}

#[test]
fn batch_stage_and_unstage_touch_only_selected_literal_paths() {
    let repo = Repo::new();
    for path in ["one", "two", "a*[x]", "aZZx"] { repo.write(path, "base\n"); }
    repo.commit();
    for path in ["one", "two", "a*[x]", "aZZx"] { repo.write(path, "changed\n"); }
    stage_files(&repo.0, &[repo.file("one"), repo.file("a*[x]")]).unwrap();
    assert!(repo.file("one").staged() && repo.file("a*[x]").staged());
    assert!(!repo.file("two").staged() && !repo.file("aZZx").staged());
    repo.write("one", "working\n");
    unstage_files(&repo.0, &[repo.file("one")]).unwrap();
    assert!(!repo.file("one").staged() && repo.file("a*[x]").staged());
    assert_eq!(std::fs::read_to_string(repo.0.join("one")).unwrap(), "working\n");
    assert!(stage_files(&repo.0, &[]).is_err());
    assert!(unstage_files(&repo.0, &[]).is_err());
}

#[test]
fn batch_unstage_handles_unborn_and_renamed_files() {
    let repo = Repo::new();
    repo.write("one", "base\n");
    repo.write("two", "other\n");
    repo.git(&["add", "-A"]);
    unstage_files(&repo.0, &[repo.file("one")]).unwrap();
    assert!(repo.file("one").untracked() && repo.file("two").staged());
    stage_files(&repo.0, &[repo.file("one")]).unwrap();
    repo.git(&["commit", "-m", "base"]);
    repo.git(&["mv", "one", "renamed"]);
    unstage_files(&repo.0, &[repo.file("renamed")]).unwrap();
    assert_eq!(snapshot(&repo.0).unwrap().status.staged_count(), 0);
    assert!(repo.0.join("renamed").exists());
}

#[test]
fn history_handles_unborn_and_missing_repository() {
    let repo = Repo::new();
    assert_eq!(crate::git_history::read(&repo.0, 200).unwrap(), crate::git_history::History::default());
    let not_repo = Repo::new();
    let outside = not_repo.0.join("outside");
    std::fs::create_dir(&outside).unwrap();
    // A missing directory is not silently reported as an empty history.
    assert!(crate::git_history::read(&outside.join("missing"), 200).is_err());
}

#[test]
fn history_preserves_message_author_refs_and_limits() {
    let repo = Repo::new();
    repo.write("one", "base\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "--author=Álvaro <alvaro@example.invalid>", "-m", "Subject\n\nBody with\tcolumns\nand separators \u{1e} \u{1f}"]);
    repo.git(&["tag", "v-fixture"]);
    repo.write("two", "next\n"); repo.commit();
    let history = crate::git_history::read(&repo.0, 1).unwrap();
    assert_eq!(history.commits.len(), 1);
    assert_eq!(history.head.as_deref(), Some(history.commits[0].oid.as_str()));
    let all = crate::git_history::read(&repo.0, 200).unwrap();
    assert_eq!(all.commits.len(), 2);
    let root = &all.commits[1];
    assert_eq!(root.subject, "Subject");
    assert_eq!(root.author, "Álvaro");
    assert!(root.refs.contains("v-fixture"));
    assert!(root.message.contains("Body with\tcolumns\nand separators \u{1e} \u{1f}"));
    assert!(root.parents.is_empty());
}

#[test]
fn history_root_rename_delete_binary_and_read_only_browsing() {
    use crate::git_history as history;
    let repo = Repo::new();
    repo.write("odd\nname\t*[x]", "base\n");
    std::fs::write(repo.0.join("binary"), [0, 1, 2, 3]).unwrap();
    repo.commit();
    let root = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["mv", "odd\nname\t*[x]", "renamed -> file"]);
    repo.git(&["commit", "-m", "rename"]);
    let rename = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
    repo.git(&["rm", "renamed -> file"]);
    repo.git(&["commit", "-m", "delete"]);
    repo.write("local", "staged\n"); repo.git(&["add", "local"]);
    repo.write("local", "unstaged\n");
    let before = snapshot(&repo.0).unwrap();
    let index = repo.git(&["write-tree"]);
    for commit in history::read(&repo.0, 200).unwrap().commits {
        for change in history::changes(&repo.0, &commit.oid).unwrap() {
            history::diff(&repo.0, &commit.oid, &change).unwrap();
        }
    }
    let changes = history::changes(&repo.0, &root).unwrap();
    assert_eq!(changes.len(), 2);
    let text = changes.iter().find(|c| c.path.starts_with("odd")).unwrap();
    assert!(history::diff(&repo.0, &root, text).unwrap().contains("+base"));
    let binary = changes.iter().find(|c| c.path == "binary").unwrap();
    assert!(history::diff(&repo.0, &root, binary).unwrap().contains("Binary files"));
    let renamed = history::changes(&repo.0, &rename).unwrap();
    assert_eq!(renamed[0].old_path.as_deref(), Some("odd\nname\t*[x]"));
    assert_eq!(renamed[0].kind, 'R');
    assert!(history::diff(&repo.0, &rename, &renamed[0]).unwrap().contains("rename from"));
    assert_eq!(before, snapshot(&repo.0).unwrap());
    assert_eq!(index, repo.git(&["write-tree"]));
    assert_eq!(std::fs::read_to_string(repo.0.join("local")).unwrap(), "unstaged\n");
}

#[test]
fn history_merge_graph_tracks_both_parents_and_first_parent_diff() {
    use crate::git_history as history;
    let repo = Repo::new();
    repo.write("one", "base\n"); repo.commit();
    repo.git(&["switch", "-c", "other"]);
    repo.write("branch", "feature\n"); repo.commit();
    repo.git(&["switch", "main"]);
    repo.write("main", "main\n"); repo.commit();
    repo.git(&["merge", "--no-ff", "other", "-m", "merge"]);
    let data = history::read(&repo.0, 200).unwrap();
    assert_eq!(data.commits.len(), 4);
    assert_eq!(data.commits[0].parents.len(), 2);
    let graph = history::graph(&data.commits);
    assert_eq!(graph[0].parents.len(), 2);
    assert!(!graph[0].incoming);
    assert!(graph.iter().skip(1).all(|row| row.incoming));
    assert_ne!(graph[0].parents[0], graph[0].parents[1]);
    assert!(graph.iter().any(|row| !row.continuations.is_empty()));
    assert!(graph.last().unwrap().parents.is_empty());
    let changes = history::changes(&repo.0, &data.commits[0].oid).unwrap();
    assert_eq!(changes.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), vec!["branch"]);
    assert!(history::diff(&repo.0, &data.commits[0].oid, &changes[0]).unwrap().contains("+feature"));
}

#[test]
fn history_includes_detached_head_and_rejects_revision_options() {
    use crate::git_history as history;
    let repo = Repo::new();
    repo.write("one", "base\n"); repo.commit();
    repo.git(&["switch", "--detach"]);
    repo.write("detached", "new\n"); repo.commit();
    let data = history::read(&repo.0, 200).unwrap();
    assert_eq!(data.commits.len(), 2);
    assert_eq!(data.head.as_deref(), Some(data.commits[0].oid.as_str()));
    assert!(history::changes(&repo.0, "--all").is_err());
    assert!(history::parse("broken\0record\0").is_err());
    assert!(history::parse_changes("R100\0old\0").is_err());
    assert!(history::parse_changes("M\0").is_err());
}

#[test]
fn history_pagination_extends_the_same_topological_prefix() {
    let repo = Repo::new();
    repo.write("one", "base\n"); repo.commit();
    let tree = repo.git(&["write-tree"]);
    let mut parent = repo.git(&["rev-parse", "HEAD"]);
    for i in 0..204 {
        parent = repo.git(&["commit-tree", tree.trim(), "-p", parent.trim(), "-m", &format!("commit {i}")]);
    }
    repo.git(&["update-ref", "HEAD", parent.trim()]);
    let first = crate::git_history::read(&repo.0, 200).unwrap();
    let next = crate::git_history::read(&repo.0, 400).unwrap();
    assert_eq!(first.commits.len(), 200);
    assert_eq!(next.commits.len(), 205);
    assert_eq!(&first.commits, &next.commits[..200]);
    assert_eq!(crate::git_history::graph(&next.commits).last().unwrap().parents.len(), 0);
}

#[test]
fn repository_selection_resolves_nested_folders_and_rejects_non_worktrees() {
    let repo = Repo::new();
    let nested = repo.0.join("folder with spaces").join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let expected = std::fs::canonicalize(&repo.0).unwrap();
    assert_eq!(repository_root(&repo.0).unwrap(), expected);
    assert_eq!(repository_root(&nested).unwrap(), expected);
    assert!(repository_root(&repo.0.join("missing")).is_err());
    let bare = repo.0.join("bare.git");
    repo.git(&["init", "--bare", bare.to_str().unwrap()]);
    assert!(repository_root(&bare).is_err());
    assert!(repository_root(&repo.0.join(".git")).is_err());
    let parent = std::env::temp_dir().join(format!("pi-non-repo-{}", std::process::id()));
    std::fs::create_dir_all(&parent).unwrap();
    assert!(repository_root(&parent).is_err());
    std::fs::remove_dir(&parent).unwrap();
}
