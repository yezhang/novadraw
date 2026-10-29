use std::sync::{Arc, Mutex};

use novadraw::event::{
    FigureEvent, ListenerDirective, NotificationEffect, NotificationRecord, ObservationListener,
    property,
};
use novadraw::render::{BackendCapabilities, RenderDisposition, SurfaceInfo};
use novadraw::runtime::{StableQueryError, StableSceneQuery};
use novadraw::{
    figure::RectangleFigure, geometry::Rectangle, runtime::Runtime, tree::FigureId,
    tree::FigureTree,
};

#[derive(Clone, Debug)]
struct ObservedMove {
    source_epoch: u64,
    sequence: u64,
    event: FigureEvent,
    query_epoch: u64,
    latest_bounds: Result<Rectangle, novadraw::tree::TreeQueryError>,
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
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 20.0, 20.0).expect("valid figure bounds"),
        ))
        .expect("valid FigureTree construction");
    let mut runtime = Runtime::new(tree);
    let baseline = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    runtime
        .frames()
        .complete_submission(
            baseline.session_id,
            baseline.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("frame completion");
    let baseline_epoch = runtime.observations().stable_query().unwrap().epoch();

    let records = Arc::new(Mutex::new(Vec::new()));
    runtime
        .observations()
        .add_observation_listener(Box::new(CaptureMoves {
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
        runtime.observations().stable_query(),
        Err(StableQueryError::NotStable {
            latest_stable_epoch
        }) if latest_stable_epoch == baseline_epoch
    ));

    let submission = runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
        .into_ready()
        .unwrap();
    let (stable_epoch, stable_bounds) = {
        let observations = runtime.observations();
        let stable = observations.stable_query().unwrap();
        (stable.epoch(), stable.bounds(figure))
    };
    assert!(stable_epoch > baseline_epoch);
    assert_eq!(stable_bounds, Ok(second));

    let records = records.lock().unwrap();
    assert_eq!(records.len(), 2);
    assert!(records[0].sequence < records[1].sequence);
    assert_eq!(records[0].source_epoch, stable_epoch);
    assert_eq!(records[1].source_epoch, stable_epoch);
    assert_eq!(records[0].query_epoch, stable_epoch);
    assert_eq!(records[1].query_epoch, stable_epoch);
    assert_eq!(records[0].latest_bounds, Ok(second));
    assert_eq!(records[1].latest_bounds, Ok(second));
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

    runtime
        .frames()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .expect("frame completion");

    let notification_only = Arc::new(Mutex::new(Vec::new()));
    runtime
        .observations()
        .add_observation_listener(Box::new(CaptureAll {
            records: Arc::clone(&notification_only),
        }));
    let previous_epoch = runtime.observations().stable_query().unwrap().epoch();
    assert!(
        runtime
            .figure(figure)
            .unwrap()
            .set_focusable(true)
            .expect("valid Runtime mutation")
    );
    assert!(matches!(
        runtime.observations().stable_query(),
        Err(StableQueryError::NotStable {
            latest_stable_epoch
        }) if latest_stable_epoch == previous_epoch
    ));
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        novadraw::runtime::FramePreparation::Idle
    ));
    let stable_epoch = runtime.observations().stable_query().unwrap().epoch();
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
            if event.figure_id() == figure && event.property().is(property::FOCUSABLE)
    )));
}
