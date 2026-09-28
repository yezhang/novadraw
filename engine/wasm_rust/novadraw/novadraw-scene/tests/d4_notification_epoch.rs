use std::sync::{Arc, Mutex};

use novadraw_render::{BackendCapabilities, RenderOutcome, SurfaceInfo};
use novadraw_scene::{
    FigureEvent, FigureId, FigureTree, ListenerDirective, NotificationEffect, NotificationRecord,
    ObservationListener, Rectangle, RectangleFigure, Runtime, StableQueryError, StableSceneQuery,
};

#[derive(Clone, Debug)]
struct ObservedMove {
    source_epoch: u64,
    sequence: u64,
    event: FigureEvent,
    query_epoch: u64,
    latest_bounds: Option<Rectangle>,
}

struct CaptureMoves {
    figure: FigureId,
    records: Arc<Mutex<Vec<ObservedMove>>>,
}

struct CaptureAll {
    records: Arc<Mutex<Vec<(NotificationRecord, u64)>>>,
}

impl ObservationListener for CaptureAll {
    fn observed(
        &self,
        record: &NotificationRecord,
        latest: StableSceneQuery<'_>,
    ) -> ListenerDirective {
        self.records
            .lock()
            .unwrap()
            .push((record.clone(), latest.epoch()));
        ListenerDirective::Keep
    }
}

impl ObservationListener for CaptureMoves {
    fn observed(
        &self,
        record: &NotificationRecord,
        latest: StableSceneQuery<'_>,
    ) -> ListenerDirective {
        if let NotificationEffect::EmitFigure(event @ FigureEvent::FigureMoved { .. }) =
            record.effect.clone()
        {
            self.records.lock().unwrap().push(ObservedMove {
                source_epoch: record.source_epoch,
                sequence: record.sequence,
                event,
                query_epoch: latest.epoch(),
                latest_bounds: latest.bounds(self.figure),
            });
        }
        ListenerDirective::Keep
    }
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 160.0,
        logical_height: 120.0,
        pixel_width: 160,
        pixel_height: 120,
        scale_factor: 1.0,
    }
}

#[test]
fn historical_records_keep_event_order_while_queries_read_latest_stable_scene() {
    let mut tree = FigureTree::new();
    let figure = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
    let mut runtime = Runtime::new(tree);
    let baseline = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    runtime.complete_submission(
        baseline.session_id,
        baseline.frame_id,
        RenderOutcome::Presented,
    );
    let baseline_epoch = runtime.stable_query().unwrap().epoch();

    let records = Arc::new(Mutex::new(Vec::new()));
    runtime.add_observation_listener(Box::new(CaptureMoves {
        figure,
        records: Arc::clone(&records),
    }));

    let first = Rectangle::new(5.0, 6.0, 30.0, 20.0);
    let second = Rectangle::new(9.0, 10.0, 40.0, 24.0);
    assert!(
        runtime
            .figure(figure)
            .unwrap()
            .set_bounds(first)
            .expect("valid Runtime mutation")
    );
    assert!(
        runtime
            .figure(figure)
            .unwrap()
            .set_bounds(second)
            .expect("valid Runtime mutation")
    );
    assert!(matches!(
        runtime.stable_query(),
        Err(StableQueryError::NotStable {
            latest_stable_epoch
        }) if latest_stable_epoch == baseline_epoch
    ));

    let submission = runtime
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .unwrap();
    let stable = runtime.stable_query().unwrap();
    assert!(stable.epoch() > baseline_epoch);
    assert_eq!(stable.bounds(figure), Some(second));

    let records = records.lock().unwrap();
    assert_eq!(records.len(), 2);
    assert!(records[0].sequence < records[1].sequence);
    assert_eq!(records[0].source_epoch, stable.epoch());
    assert_eq!(records[1].source_epoch, stable.epoch());
    assert_eq!(records[0].query_epoch, stable.epoch());
    assert_eq!(records[1].query_epoch, stable.epoch());
    assert_eq!(records[0].latest_bounds, Some(second));
    assert_eq!(records[1].latest_bounds, Some(second));
    assert!(matches!(
        records[0].event,
        FigureEvent::FigureMoved {
            old_bounds,
            new_bounds,
            ..
        } if old_bounds == Rectangle::new(0.0, 0.0, 20.0, 20.0) && new_bounds == first
    ));
    assert!(matches!(
        records[1].event,
        FigureEvent::FigureMoved {
            old_bounds,
            new_bounds,
            ..
        } if old_bounds == first && new_bounds == second
    ));
    drop(records);

    runtime.complete_submission(
        submission.session_id,
        submission.frame_id,
        RenderOutcome::Presented,
    );

    let notification_only = Arc::new(Mutex::new(Vec::new()));
    runtime.add_observation_listener(Box::new(CaptureAll {
        records: Arc::clone(&notification_only),
    }));
    let previous_epoch = runtime.stable_query().unwrap().epoch();
    assert!(
        runtime
            .figure(figure)
            .unwrap()
            .set_focusable(true)
            .expect("valid Runtime mutation")
    );
    assert!(matches!(
        runtime.stable_query(),
        Err(StableQueryError::NotStable {
            latest_stable_epoch
        }) if latest_stable_epoch == previous_epoch
    ));
    assert!(matches!(
        runtime.prepare_submission_state(surface(), BackendCapabilities::RETAINED_PARTIAL),
        novadraw_scene::FramePreparation::Idle
    ));
    let stable_epoch = runtime.stable_query().unwrap().epoch();
    let notification_only = notification_only.lock().unwrap();
    assert!(
        notification_only
            .windows(2)
            .all(|records| records[0].0.sequence < records[1].0.sequence)
    );
    assert!(
        notification_only
            .iter()
            .all(|(record, query_epoch)| record.source_epoch == stable_epoch
                && *query_epoch == stable_epoch)
    );
    assert!(notification_only.iter().any(|(record, _)| matches!(
        record.effect,
        NotificationEffect::EmitProperty(ref event)
            if event.figure_id == figure && event.property == "focusable"
    )));
}
