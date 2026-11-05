use kuro::agent::run_agent_diagnostics;

#[tokio::test]
async fn test_agent_diagnostics_suite() {
    let report = run_agent_diagnostics().await;
    assert!(report.all_passed());
    assert_eq!(report.checks.len(), 5);
}
