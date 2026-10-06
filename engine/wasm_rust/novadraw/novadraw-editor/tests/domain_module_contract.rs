use novadraw_editor::{
    command::CommandStack, model::ModelRevision, policy::PolicyRole, text_input::TextInputPurpose,
    tool::ToolError, viewer::SelectionMode,
};

#[test]
fn editor_domains_are_available_through_named_modules() {
    let _: Option<CommandStack<()>> = None;
    let _: Option<ModelRevision> = None;
    let _: Option<PolicyRole> = None;
    let _: Option<TextInputPurpose> = None;
    let _: Option<ToolError> = None;
    let _: Option<SelectionMode> = None;
}
