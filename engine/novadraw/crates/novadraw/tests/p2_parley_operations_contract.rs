//! Dependency operation admission through the existing Core session budget.
//! This does not enable the production staged engine or Flow adapter.
use novadraw::{
    figure::{PreparationBudget, PreparationBudgetHandle},
    text::{
        TextLayoutDiagnostics,
        shaping::{TextStageContext, TextStageCostPlan, TextStageError},
    },
};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, Layout, LayoutContext, LayoutOperation,
    LayoutOperationCost, LayoutOperationError, StyleProperty,
    fontique::{Blob, Collection, CollectionOptions},
};
use std::sync::Arc;

fn layout() -> Layout<usize> {
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    fonts.collection.register_fonts(
        Blob::new(Arc::new(
            include_bytes!("../../../assets/fonts/InterVariable.ttf").to_vec(),
        )),
        None,
    );
    let mut context = LayoutContext::new();
    let text = "office a b c d e f g h i j k l m n o p\n";
    let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(14.0));
    let family = FontFamily::parse_list("Inter Variable").collect::<Vec<_>>();
    builder.push_default(family.as_slice());
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(90.0));
    layout.align(Some(90.0), Alignment::Justify, AlignmentOptions::default());
    assert!(layout.retained_fonts().len() > 0);
    assert!(layout.len() > 1);
    layout
}

fn charge(
    context: &TextStageContext,
    step: LayoutOperation,
    cost: LayoutOperationCost,
) -> Result<(), TextStageError> {
    let stage = match step {
        LayoutOperation::ClonePlanning => "parley-clone-plan",
        LayoutOperation::Clone => "parley-clone",
        LayoutOperation::AlignmentPlanning => "parley-alignment-plan",
        LayoutOperation::Alignment => "parley-alignment",
        LayoutOperation::LineBreaking => "parley-break",
        LayoutOperation::LineCommit => "parley-commit",
        LayoutOperation::LineMetrics => "parley-metrics",
        LayoutOperation::LineReorder => "parley-reorder",
        LayoutOperation::LineStorage => "parley-line-storage",
        LayoutOperation::LineFinalize => "parley-finalize",
    };
    context.charge(
        stage,
        TextStageCostPlan {
            work: u64::try_from(cost.work).map_err(|_| TextStageError::AccountingOverflow)?,
            allocation: u64::try_from(cost.allocation)
                .map_err(|_| TextStageError::AccountingOverflow)?,
        },
    )
}

fn context(limit: PreparationBudget) -> TextStageContext {
    TextStageContext::new(
        PreparationBudgetHandle::new(limit),
        TextLayoutDiagnostics::default(),
    )
}

#[test]
fn exact_operation_costs_accumulate_in_one_core_session() {
    let source = layout();
    let session = context(PreparationBudget::default());
    let mut expected = PreparationBudget::new(0, 0);
    let mut admit = |step, cost: LayoutOperationCost| {
        expected.work += (cost.work + cost.allocation) as u64;
        expected.allocation += cost.allocation as u64;
        charge(&session, step, cost)
    };
    let mut copy = source.try_clone_admitted(&mut admit).unwrap();
    copy.try_align_admitted(
        Some(400.0),
        Alignment::End,
        AlignmentOptions::default(),
        &mut admit,
    )
    .unwrap();
    assert_eq!(session.budget().used(), expected);
    session.budget().check().unwrap();
    let slots = source
        .owned_storage()
        .arrays
        .iter()
        .map(|array| array.len as u64)
        .sum::<u64>();
    assert_eq!(expected.allocation, slots);
    assert_eq!(source.width(), copy.width());
}

#[test]
fn clone_allocation_denial_preserves_source_and_sticks_to_session() {
    let source = layout();
    let before = source.owned_storage();
    let session = context(PreparationBudget::new(1_000_000, 0));
    let result = source.try_clone_admitted(|step, cost| charge(&session, step, cost));
    let Err(LayoutOperationError::Admission(TextStageError::Budget(error))) = result else {
        panic!("allocation must be rejected before copying")
    };
    assert_eq!(error.stage, "parley-clone");
    assert!(error.allocation);
    assert_eq!(source.owned_storage(), before);
    assert_eq!(session.budget().check().unwrap_err(), error);
    let mut calls = 0;
    let retry = source.try_clone_admitted(|step, cost| {
        calls += 1;
        charge(&session, step, cost)
    });
    assert!(matches!(retry, Err(LayoutOperationError::Admission(_))));
    assert_eq!(calls, 1, "sticky failure stops retry at its planning step");
}

#[test]
fn alignment_denial_after_planning_preserves_justified_geometry() {
    let mut source = layout();
    let storage = source.owned_storage();
    let before = source.clone();
    let planning = source
        .owned_storage()
        .arrays
        .iter()
        .filter(|array| {
            matches!(
                array.field,
                parley::LayoutStorageField::Lines | parley::LayoutStorageField::LineItems
            )
        })
        .map(|array| array.len as u64)
        .sum();
    let session = context(PreparationBudget::new(planning, 0));
    let result = source.try_align_admitted(
        Some(400.0),
        Alignment::End,
        AlignmentOptions::default(),
        |step, cost| charge(&session, step, cost),
    );
    let Err(LayoutOperationError::Admission(TextStageError::Budget(error))) = result else {
        panic!("alignment must be rejected after admitted planning")
    };
    assert_eq!(error.stage, "parley-alignment");
    assert!(!error.allocation);
    assert_eq!(session.budget().used().work, planning);
    assert_eq!(source.owned_storage(), storage);
    for (actual, original) in source.lines().zip(before.lines()) {
        assert_eq!(actual.metrics(), original.metrics());
        for (actual, original) in actual.items().zip(original.items()) {
            if let (
                parley::PositionedLayoutItem::GlyphRun(actual),
                parley::PositionedLayoutItem::GlyphRun(original),
            ) = (actual, original)
            {
                assert_eq!(actual.offset(), original.offset());
                assert_eq!(
                    actual.glyphs().collect::<Vec<_>>(),
                    original.glyphs().collect::<Vec<_>>(),
                );
            }
        }
    }
}

#[test]
fn breaking_costs_accumulate_and_denial_remains_sticky() {
    let source = layout();
    let before = source.owned_storage();
    let session = context(PreparationBudget::new(1_000_000, 1_000_000));
    let mut expected = PreparationBudget::new(0, 0);
    let result = source
        .clone()
        .try_break_all_lines_admitted(Some(40.0), |step, cost| {
            expected.work += (cost.work + cost.allocation) as u64;
            expected.allocation += cost.allocation as u64;
            charge(&session, step, cost)
        })
        .unwrap();
    assert!(result.len() > source.len());
    assert_eq!(session.budget().used(), expected);
    assert_eq!(source.owned_storage(), before);
    assert!(expected.allocation > 0);

    let denied = context(PreparationBudget::new(1_000_000, 0));
    let failure = source
        .clone()
        .try_break_all_lines_admitted(Some(1.0), |step, cost| charge(&denied, step, cost));
    let Err(LayoutOperationError::Admission(TextStageError::Budget(error))) = failure else {
        panic!("new line capacity must be rejected")
    };
    assert!(error.allocation);
    assert_eq!(error.stage, "parley-line-storage");
    assert_eq!(denied.budget().check().unwrap_err(), error);
    let mut calls = 0;
    let retry = source
        .clone()
        .try_break_all_lines_admitted(Some(1.0), |step, cost| {
            calls += 1;
            charge(&denied, step, cost)
        });
    assert!(matches!(retry, Err(LayoutOperationError::Admission(_))));
    assert_eq!(calls, 1);
    assert_eq!(source.owned_storage(), before);
}

#[test]
fn final_line_scan_refusal_returns_no_candidate() {
    let source = layout();
    let session = context(PreparationBudget::new(1_000_000, 1_000_000));
    let mut reached_final = false;
    let failure = source
        .clone()
        .try_break_all_lines_admitted(Some(60.0), |step, cost| {
            if step == LayoutOperation::LineFinalize {
                reached_final = true;
                return Err(TextStageError::AccountingOverflow);
            }
            charge(&session, step, cost)
        });
    assert!(reached_final);
    assert!(matches!(
        failure,
        Err(LayoutOperationError::Admission(
            TextStageError::AccountingOverflow
        ))
    ));
    session.budget().check().unwrap();
}
