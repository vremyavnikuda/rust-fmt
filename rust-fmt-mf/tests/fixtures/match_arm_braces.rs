macro_rules! build_prompt {
    ($kind:ident) => {
        match $kind {
            AgentKind::User => r#"You are a friendly agent."#,
            AgentKind::Analyzer => r#"You are analytical."#,
        }
    };
}
