// Tests for `src/checks/website.rs`.
use crate::support::*;

#[test]
fn website_requires_and_probes_github_homepage_metadata() {
    let mut repository = facts();
    repository.homepage = None;
    let report = run_github_checks(&repository);
    let website = report
        .results
        .iter()
        .find(|result| result.check == "website")
        .unwrap();
    assert_eq!(website.status, CheckStatus::Fail);
    assert!(website.message.contains("homepage is not set"));

    repository.homepage = Some(format!("{}/missing", WEBSITE_SERVER.as_str()));
    let report = run_github_checks(&repository);
    let website = report
        .results
        .iter()
        .find(|result| result.check == "website")
        .unwrap();
    assert_eq!(website.status, CheckStatus::Fail);
    assert!(website.message.contains("HTTP 404"));

    let unavailable = TcpListener::bind("127.0.0.1:0").unwrap();
    repository.homepage = Some(format!("http://{}", unavailable.local_addr().unwrap()));
    drop(unavailable);
    let report = run_github_checks(&repository);
    let website = report
        .results
        .iter()
        .find(|result| result.check == "website")
        .unwrap();
    assert_eq!(website.status, CheckStatus::Error);
}
