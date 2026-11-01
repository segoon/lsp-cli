use super::*;

#[test]
fn recognizes_luarocks_recipe_families() {
    let family = InstallationFamily::from_source_id("pkg:luarocks/fennel-ls@0.2.1-1")
        .expect("LuaRocks recipes should be classified for CI");
    assert_eq!(family, InstallationFamily::Luarocks);
    assert_eq!(family.label(), "luarocks");
    assert_eq!(
        InstallationFamily::parse("luarocks").expect("selector should parse"),
        family
    );
}

fn workflow_inventory() -> (
    Manifest,
    BTreeMap<String, WorkflowPackage>,
    RegistrySnapshot,
) {
    let manifest = Manifest::load().expect("manifest should load");
    let families = manifest
        .downloadable_servers()
        .expect("downloadable servers should load")
        .into_iter()
        .map(|server| {
            assert!(!server.name.is_empty() && !server.program.is_empty());
            (
                server.id,
                WorkflowPackage {
                    installation_family: InstallationFamily::Github,
                    source_id: "pkg:github/example/server@v1.0.0".to_string(),
                },
            )
        })
        .collect();
    let snapshot = RegistrySnapshot {
        release_tag: "2026-09-25".to_string(),
        refreshed_at_epoch_seconds: 1_797_000_000,
        digest: Some("sha256:0123".to_string()),
    };

    (manifest, families, snapshot)
}

#[test]
fn builds_serializable_shards_without_manifest_family_metadata() {
    let (manifest, families, snapshot) = workflow_inventory();
    let (plan, report) = manifest
        .workflow_plan(WorkflowSelector::All, None, &families, &snapshot)
        .expect("workflow plan should build");

    let serialized = serde_json::to_value(plan).expect("plan should serialize");
    assert_eq!(serialized["families"]["github"]["has_runnable"], true);
    assert_eq!(serialized["families"]["npm"]["has_runnable"], false);
    assert!(report.lines().any(|line| line.contains("executable")));
    assert!(report.contains("pkg:github/example/server@v1.0.0"));
    assert!(report.contains("2026-09-25"));
    assert!(report.contains("sha256:0123"));
    assert_eq!(
        InstallationFamily::from_source_id("pkg:golang/example/tool")
            .expect("known family should parse"),
        InstallationFamily::Golang
    );
    assert!(report.contains("## CI matrices"));
}

fn shards(family: InstallationFamily, count: usize) -> Vec<WorkflowShard> {
    (0..count)
        .map(|index| WorkflowShard {
            name: format!("language-{index}/{}", family.label()),
            language: format!("language-{index}"),
            installation_family: family,
            cases: format!("language-{index}/server-a,language-{index}/server-b"),
        })
        .collect()
}

#[test]
fn splits_259_groups_without_losing_or_merging_cases() {
    let original = shards(InstallationFamily::Github, 256)
        .into_iter()
        .chain(shards(InstallationFamily::Npm, 3))
        .collect::<Vec<_>>();
    let expected = serde_json::to_value(&original).expect("shards should serialize");
    let plan = WorkflowPlan::from_shards(original).expect("each family fits GitHub's limit");
    let serialized = serde_json::to_value(plan).expect("plan should serialize");
    let families = serialized["families"]
        .as_object()
        .expect("family map should serialize");
    assert_eq!(families.len(), InstallationFamily::ALL.len());
    let actual = families
        .values()
        .flat_map(|family| {
            family["matrix"]["include"]
                .as_array()
                .expect("matrix should be an array")
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(serde_json::Value::Array(actual), expected);
    assert_eq!(
        families["github"]["matrix"]["include"]
            .as_array()
            .expect("GitHub matrix should exist")
            .len(),
        256
    );
    assert_eq!(
        families["npm"]["matrix"]["include"]
            .as_array()
            .expect("npm matrix should exist")
            .len(),
        3
    );
    for (label, family) in families {
        let include = family["matrix"]["include"]
            .as_array()
            .expect("matrix should be an array");
        assert!(include.len() <= 256);
        assert_eq!(family["has_runnable"], !include.is_empty());
        assert!(
            include
                .iter()
                .all(|shard| shard["installation_family"] == *label)
        );
    }
}

#[test]
fn rejects_a_single_family_exceeding_the_matrix_limit() {
    let error = WorkflowPlan::from_shards(shards(InstallationFamily::Npm, 257))
        .err()
        .expect("oversized family should fail planning");
    assert!(error.contains("npm"));
    assert!(error.contains("257 jobs"));
    assert!(error.contains("limit of 256 jobs per matrix"));
}

#[test]
fn empty_selection_marks_every_family_as_skipped() {
    let plan = WorkflowPlan::from_shards([]).expect("empty selection should plan");
    assert_eq!(plan.families.len(), InstallationFamily::ALL.len());
    assert!(
        plan.families
            .values()
            .all(|family| !family.has_runnable && family.matrix.include.is_empty())
    );
}

#[test]
fn selectors_preserve_case_membership_and_skip_unselected_families() {
    let (manifest, mut packages, snapshot) = workflow_inventory();
    // Deliberately vary registry metadata: routing must follow the recipe, not the server ID.
    let rust_package = packages
        .get_mut("rust_analyzer")
        .expect("Rust server should exist");
    rust_package.installation_family = InstallationFamily::Npm;
    rust_package.source_id = "pkg:npm/example-server@1.0.0".to_string();
    let rust_cases = manifest
        .pairs
        .iter()
        .filter(|pair| pair.language == "rust" && pair.has_executable_behavior())
        .map(PairCase::label)
        .collect::<Vec<_>>();
    for (selector, value, expected) in [
        (WorkflowSelector::Language, "rust", rust_cases),
        (
            WorkflowSelector::Server,
            "rust_analyzer",
            vec!["rust/rust_analyzer".to_string()],
        ),
        (
            WorkflowSelector::InstallationFamily,
            "npm",
            vec!["rust/rust_analyzer".to_string()],
        ),
        (WorkflowSelector::InstallationFamily, "cargo", vec![]),
    ] {
        let (plan, _) = manifest
            .workflow_plan(selector, Some(value), &packages, &snapshot)
            .expect("selector should plan");
        let actual = plan
            .families
            .values()
            .flat_map(|family| &family.matrix.include)
            .flat_map(|shard| shard.cases.split(','))
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual,
            expected.into_iter().collect(),
            "selector value {value}"
        );
        for (family, plan) in &plan.families {
            assert_eq!(plan.has_runnable, !plan.matrix.include.is_empty());
            for shard in &plan.matrix.include {
                for case in shard.cases.split(',') {
                    let (_, server) = case.split_once('/').expect("case should identify a server");
                    assert_eq!(packages[server].installation_family, *family);
                }
            }
        }
    }
}

#[test]
fn workflow_covers_every_family_and_continues_after_failed_or_skipped_families() {
    let workflow: serde_json::Value =
        serde_saphyr::from_str(include_str!("../../../.github/workflows/e2e.yml"))
            .expect("workflow YAML should parse");
    let jobs = workflow["jobs"]
        .as_object()
        .expect("workflow should declare jobs");
    assert_eq!(jobs.len(), InstallationFamily::ALL.len() + 1);
    let mut previous = None;
    for family in InstallationFamily::ALL {
        let label = family.label();
        let job = &jobs[&format!("test-{label}")];
        assert_eq!(job["uses"], "./.github/workflows/e2e-family.yml");
        let condition = job["if"].as_str().expect("family should have a condition");
        assert!(condition.contains("!cancelled()"));
        assert!(condition.contains("needs.plan.result == 'success'"));
        assert!(condition.contains(&format!("families.{label}.has_runnable")));
        assert_eq!(
            job["with"]["matrix"],
            format!("${{{{ toJSON(fromJSON(needs.plan.outputs.plan).families.{label}.matrix) }}}}")
        );
        let mut needs = vec!["plan".to_string()];
        if let Some(previous) = previous {
            needs.push(format!("test-{previous}"));
        }
        assert_eq!(job["needs"], serde_json::json!(needs));
        previous = Some(label);
    }
    let reusable: serde_json::Value =
        serde_saphyr::from_str(include_str!("../../../.github/workflows/e2e-family.yml"))
            .expect("reusable workflow YAML should parse");
    assert_eq!(
        reusable["on"]["workflow_call"]["inputs"]["matrix"]["type"],
        "string"
    );
    let test = &reusable["jobs"]["test"];
    assert_eq!(test["strategy"]["matrix"], "${{ fromJSON(inputs.matrix) }}");
    assert_eq!(test["strategy"]["max-parallel"], 4);
    assert_eq!(test["strategy"]["fail-fast"], false);
    assert_eq!(test["timeout-minutes"], 90);
}
