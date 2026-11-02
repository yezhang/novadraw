use novadraw::{event::ListenerScope, figure::RectangleFigure, runtime::Runtime};
use novadraw_inspector::FigureInspector;
use novadraw_service_failure_consumer::inspection::recover;
use std::{
    io::{self, BufRead},
    num::NonZeroUsize,
};

const EVENT_CAPACITY: usize = 16;
const FIGURE_SIZE: f64 = 40.0;
const MOVE_DISTANCE: f64 = 10.0;

fn main() {
    let mut runtime = Runtime::empty();
    let figure = runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, FIGURE_SIZE, FIGURE_SIZE).unwrap(),
        ))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let mut inspector = FigureInspector::new(NonZeroUsize::new(EVENT_CAPACITY).unwrap());
    let mut tokens = vec![inspector.attach(&mut runtime).unwrap()];
    println!("m: move | e: events | d: detach | r: rebuild subscription | c: clear | q: quit");
    for line in io::stdin().lock().lines() {
        match line.expect("terminal input").trim() {
            "m" => {
                runtime
                    .figure(figure)
                    .unwrap()
                    .translate(MOVE_DISTANCE, 0.0)
                    .unwrap();
                runtime.frames().stabilize_for_query().unwrap();
                println!(
                    "x={}; checkpoint={:?}",
                    runtime.tree().figure_bounds(figure).unwrap().x,
                    runtime.observations().checkpoint().unwrap()
                );
            }
            "e" => {
                for event in inspector.events().unwrap() {
                    println!(
                        "{}:{} {:?}",
                        event.source_epoch, event.sequence, event.effect
                    );
                }
            }
            "d" => {
                for token in &tokens {
                    println!("detach={}", token.detach(&mut runtime).unwrap());
                }
            }
            "r" => {
                match recover(
                    &mut runtime,
                    &tokens,
                    &[ListenerScope::Runtime],
                    NonZeroUsize::new(EVENT_CAPACITY).unwrap(),
                ) {
                    Ok(baseline) => {
                        println!(
                            "gap accepted; x={}; baseline={:?}; new events={}",
                            baseline.snapshot.figure(figure).unwrap().bounds.x,
                            baseline.checkpoint,
                            baseline.inspector.events().unwrap().len()
                        );
                        inspector = baseline.inspector;
                        tokens = baseline.subscriptions;
                    }
                    Err(error) => {
                        eprintln!("recovery failed; host remains paused: {error:?}");
                        break;
                    }
                }
            }
            "c" => {
                inspector.clear_events().unwrap();
                println!("events cleared");
            }
            "q" => break,
            _ => println!("m / e / d / r / c / q"),
        }
    }
    for token in &tokens {
        token.detach(&mut runtime).unwrap();
    }
}
