use novadraw::{
    figure::PreparationBudgetHandle,
    text::{TextLayoutDiagnostics, shaping::TextStageContext},
};

/// Independent Native/Wasm consumer of the backend-neutral stage domain.
#[unsafe(no_mangle)]
pub extern "C" fn text_stage_session() -> bool {
    let session = TextStageContext::new(
        PreparationBudgetHandle::default(),
        TextLayoutDiagnostics::default(),
    );
    session.binding().is_none() && session.budget().check().is_ok()
}

#[cfg(test)]
mod contracts {
    include!("../../../../crates/novadraw/tests/p2_text_stages.rs");
}
