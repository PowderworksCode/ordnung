// Tests for `src/checks/ci_matrix_scoped.rs`.
use crate::support::*;

/// Modeled on a real repository whose fanout job enumerates the repository
/// rather than the change: the matrix expands identically on every pull
/// request, so depending on the fanout is not scoping. A fanout that reads
/// the diff, a job condition, or workflow path filters each short-circuit.
#[test]
fn matrix_jobs_must_short_circuit_on_pull_requests() {
    let status = |workflow: &str| {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
        fs::write(repo.path().join(".github/workflows/ci.yml"), workflow).unwrap();
        let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
        let report =
            run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
        report
            .results
            .iter()
            .find(|result| result.check == "ci-matrix-scoped")
            .unwrap()
            .clone()
    };

    let enumerating = status(
        "on: pull_request\njobs:\n  discover:\n    outputs:\n      langs: ${{ steps.list.outputs.langs }}\n    steps:\n      - id: list\n        run: find crates -name grammar.js\n  sweep:\n    needs: discover\n    strategy:\n      matrix:\n        lang: ${{ fromJSON(needs.discover.outputs.langs) }}\n    steps:\n      - run: ./sweep.sh ${{ matrix.lang }}\n",
    );
    assert_eq!(enumerating.status, CheckStatus::Fail);
    assert!(enumerating.message.contains("ci.yml:sweep"));

    let diff_aware = status(
        "on: pull_request\njobs:\n  discover:\n    outputs:\n      langs: ${{ steps.list.outputs.langs }}\n    steps:\n      - id: list\n        run: git diff --name-only origin/main | cut -d/ -f2 | sort -u\n  sweep:\n    needs: discover\n    strategy:\n      matrix:\n        lang: ${{ fromJSON(needs.discover.outputs.langs) }}\n    steps:\n      - run: ./sweep.sh ${{ matrix.lang }}\n",
    );
    assert_eq!(diff_aware.status, CheckStatus::Pass);

    let conditioned = status(
        "on: pull_request\njobs:\n  changes:\n    outputs:\n      rust: ${{ steps.filter.outputs.rust }}\n    steps:\n      - id: filter\n        uses: dorny/paths-filter@v3\n  build:\n    needs: changes\n    if: needs.changes.outputs.rust == 'true'\n    strategy:\n      matrix:\n        os: [ubuntu-latest, macos-latest]\n    steps:\n      - run: cargo test\n",
    );
    assert_eq!(conditioned.status, CheckStatus::Pass);

    let path_filtered = status(
        "on:\n  pull_request:\n    paths: [\"crates/**\"]\njobs:\n  build:\n    strategy:\n      matrix:\n        os: [ubuntu-latest, macos-latest]\n    steps:\n      - run: cargo test\n",
    );
    assert_eq!(path_filtered.status, CheckStatus::Pass);
}
