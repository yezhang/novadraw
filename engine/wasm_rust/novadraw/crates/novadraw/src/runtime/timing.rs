use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

/// Cumulative CPU work in one diagnostic stage. These are not source-commit or present receipts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimePhaseTiming {
    pub stage: &'static str,
    pub calls: u64,
    pub inclusive_ns: u128,
    pub exclusive_ns: u128,
}

#[derive(Default)]
struct TimingState {
    stages: BTreeMap<&'static str, RuntimePhaseTiming>,
    children: Vec<u128>,
}

/// Only internal synchronous scopes write timings; disabled scopes never read a clock.
#[derive(Clone, Default)]
pub(crate) struct TimingRecorder(Option<Arc<Mutex<TimingState>>>);

impl TimingRecorder {
    pub(crate) fn enabled() -> Self {
        Self(Some(Arc::new(Mutex::new(TimingState::default()))))
    }

    pub(crate) fn start(&self, stage: &'static str) -> TimingScope {
        let active = self.0.as_ref().map(|state| {
            state.lock().unwrap().children.push(0);
            (state.clone(), stage, Instant::now())
        });
        TimingScope(active)
    }

    pub(crate) fn take(&self) -> Vec<RuntimePhaseTiming> {
        let Some(state) = &self.0 else {
            return Vec::new();
        };
        let mut state = state.lock().unwrap();
        assert!(state.children.is_empty(), "diagnostic scope still active");
        std::mem::take(&mut state.stages).into_values().collect()
    }
}

pub(crate) struct TimingScope(Option<(Arc<Mutex<TimingState>>, &'static str, Instant)>);

fn finish(state: &mut TimingState, stage: &'static str, elapsed: u128) {
    let children = state.children.pop().expect("paired timing scope");
    let entry = state
        .stages
        .entry(stage)
        .or_insert_with(|| RuntimePhaseTiming {
            stage,
            ..Default::default()
        });
    entry.calls += 1;
    entry.inclusive_ns += elapsed;
    entry.exclusive_ns += elapsed.saturating_sub(children);
    if let Some(parent) = state.children.last_mut() {
        *parent += elapsed;
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        if let Some((state, stage, start)) = &self.0 {
            let elapsed = start.elapsed().as_nanos();
            finish(&mut state.lock().unwrap(), stage, elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_accounts_children_once_and_accumulates_repeated_stages() {
        let mut state = TimingState::default();
        state.children.extend([0, 0, 0]);
        finish(&mut state, "leaf", 3);
        finish(&mut state, "child", 7);
        finish(&mut state, "root", 11);
        state.children.push(0);
        finish(&mut state, "leaf", 2);
        assert_eq!(state.stages["root"].exclusive_ns, 4);
        assert_eq!(state.stages["child"].exclusive_ns, 4);
        assert_eq!(state.stages["leaf"].exclusive_ns, 5);
        assert_eq!(state.stages["leaf"].calls, 2);
        assert_eq!(
            state.stages.values().map(|v| v.exclusive_ns).sum::<u128>(),
            13
        );
    }

    #[test]
    fn disabled_and_failed_scopes_leave_no_live_stack_or_cross_instance_work() {
        fn attempt(recorder: &TimingRecorder) -> Result<(), ()> {
            let _outer = recorder.start("attempt");
            let _inner = recorder.start("failure");
            Err(())
        }
        let disabled = TimingRecorder::default();
        assert!(attempt(&disabled).is_err());
        assert!(disabled.take().is_empty());
        let enabled = TimingRecorder::enabled();
        assert!(attempt(&enabled).is_err());
        let samples = enabled.take();
        assert_eq!(samples.len(), 2);
        assert!(
            samples
                .iter()
                .all(|v| v.calls == 1 && v.exclusive_ns <= v.inclusive_ns)
        );
        assert!(enabled.take().is_empty());
        assert!(TimingRecorder::enabled().take().is_empty());
    }
}
