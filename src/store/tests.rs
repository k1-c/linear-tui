use super::*;
use crate::entity::Connection;

fn issue(id: &str, title: &str) -> Issue {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","identifier":"ENG-{id}","title":"{title}","priority":0}}"#
    ))
    .unwrap()
}

fn page(items: Vec<Issue>, append: bool) -> Page<Issue> {
    Page::new(items, PageInfo::default(), append)
}

#[test]
fn a_patch_reaches_every_copy_of_the_issue() {
    let mut store = Store::default();
    store.issues[IssueSource::Team].items = vec![issue("1", "old"), issue("2", "other")];
    store.issues[IssueSource::My].items = vec![issue("1", "old")];
    store.current_issue = Some(issue("1", "old"));

    store.patch_issue(&IssueId::from("1"), |i| i.title = "new".into());

    assert_eq!(store.issues[IssueSource::Team].items[0].title, "new");
    assert_eq!(store.issues[IssueSource::Team].items[1].title, "other");
    assert_eq!(store.issues[IssueSource::My].items[0].title, "new");
    assert_eq!(store.current_issue.unwrap().title, "new");
}

#[test]
fn a_refreshed_issue_keeps_each_list_copy_s_comments() {
    let mut store = Store::default();
    store.issues[IssueSource::Team].items = vec![issue("1", "old")];

    let mut fresh = issue("1", "new");
    fresh.comments = Some(Connection {
        nodes: Vec::new(),
        page_info: PageInfo::default(),
    });
    store.current_issue = Some(issue("1", "old"));
    store.refresh_issue(fresh);

    let listed = &store.issues[IssueSource::Team].items[0];
    assert_eq!(listed.title, "new");
    assert!(
        listed.comments.is_none(),
        "the list copy has no thread loaded"
    );
    assert!(store.current_issue.unwrap().comments.is_some());
}

#[test]
fn an_appended_page_drops_issues_already_held() {
    let mut rows = Rows::default();
    rows.accept_issues(page(vec![issue("1", "a"), issue("2", "b")], false));
    rows.accept_issues(page(vec![issue("2", "b"), issue("3", "c")], true));

    let ids: Vec<_> = rows.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["1", "2", "3"]);
    assert!(rows.loaded);
}

#[test]
fn a_first_page_replaces_the_rows() {
    let mut rows = Rows::default();
    rows.accept_issues(page(vec![issue("1", "a")], false));
    rows.accept_issues(page(vec![issue("2", "b")], false));

    assert_eq!(rows.items.len(), 1);
    assert_eq!(rows.items[0].id.as_str(), "2");
}

#[test]
fn an_issue_is_found_in_any_list_or_the_detail_view() {
    let mut store = Store::default();
    store.issues[IssueSource::Cycle].items = vec![issue("1", "in a cycle")];
    store.current_issue = Some(issue("2", "open"));

    assert_eq!(
        store.issue(&IssueId::from("1")).unwrap().title,
        "in a cycle"
    );
    assert_eq!(store.issue(&IssueId::from("2")).unwrap().title, "open");
    assert!(store.issue(&IssueId::from("3")).is_none());
}
