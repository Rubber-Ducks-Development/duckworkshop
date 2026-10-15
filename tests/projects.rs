use duck_workshop::{
    models,
    patterns::Pattern,
    project::{Project, Session},
};

fn blank_project() -> Project {
    Project {
        format: "duck-workshop".into(),
        version: 1,
        session: Session::default(),
        model: None,
        tool: 1,
        dark_mode: false,
        comparison: [String::new(), String::new()],
        textures: vec![],
    }
}

#[test]
fn new_projects_start_without_example_content() {
    let project = blank_project();
    project.validate().unwrap();
    assert!(project.model.is_none());
    assert!(project.session.metas.iter().all(String::is_empty));
    assert!(project.session.model_name.is_empty());
    assert!(project.session.handling_name.is_empty());
    assert!(
        project
            .session
            .pattern
            .channels
            .iter()
            .all(|c| c.bits.iter().all(|b| !*b))
    );
}

#[test]
fn project_roundtrip_keeps_real_model_edits_and_comparison() {
    let mut project = blank_project();
    project.model = Some(
        models::parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3")
            .unwrap(),
    );
    project.session.pattern.channels[3].bits[31] = true;
    project.session.pattern.channels[3].name = "Rear Left".into();
    project.session.uv.channel = 0;
    project.session.uv.lod = "All".into();
    project.comparison = ["left input".into(), "right input".into()];
    let decoded = Project::from_json(&project.to_json().unwrap()).unwrap();
    assert_eq!(decoded.model.unwrap().triangles(), 1);
    assert!(decoded.session.pattern.channels[3].bits[31]);
    assert_eq!(decoded.session.pattern.channels[3].name, "Rear Left");
    assert_eq!(decoded.comparison[1], "right input");
}

#[test]
fn unsupported_project_and_unsafe_settings_are_rejected() {
    let mut project = blank_project();
    project.version = 999;
    assert!(project.validate().is_err());
    project = blank_project();
    project.session.uv.channel = 8;
    assert!(project.validate().is_err());
    project = blank_project();
    project.session.metas[0] = "broken XML".into();
    assert!(project.validate().is_err());
    assert!(Project::from_json(br#"{"format":"another-app"}"#).is_err());
    let pattern = Pattern::default();
    assert_eq!(pattern.name, "Untitled Sequence");
}
