use crate::session::{CanvasAction, EditorSession};
use crate::widget::EditorCanvas;
use novadraw_editor::{EditPartFactory, ModelAdapter};
use std::marker::PhantomData;
use xilem_masonry::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::{Pod, ViewCtx};

/// Send+Sync description; callbacks access session state only on the UI thread.
pub struct CanvasView<State, A: ModelAdapter, F: EditPartFactory<A>, Build, Sync, ActionFn> {
    build: Build,
    sync: Sync,
    action: ActionFn,
    disabled: bool,
    marker: PhantomData<fn(State) -> (A, F)>,
}

pub fn editor_canvas<State, A: ModelAdapter, F: EditPartFactory<A>, Build, Sync, ActionFn>(
    build: Build,
    sync: Sync,
    action: ActionFn,
) -> CanvasView<State, A, F, Build, Sync, ActionFn> {
    CanvasView {
        build,
        sync,
        action,
        disabled: false,
        marker: PhantomData,
    }
}

impl<State, A: ModelAdapter, F: EditPartFactory<A>, B, S, C> CanvasView<State, A, F, B, S, C> {
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<State, A: ModelAdapter, F: EditPartFactory<A>, B, S, C> ViewMarker
    for CanvasView<State, A, F, B, S, C>
{
}

impl<State, Action, A, F, B, S, C> View<State, Action, ViewCtx> for CanvasView<State, A, F, B, S, C>
where
    State: 'static,
    Action: 'static,
    A: ModelAdapter + 'static,
    F: EditPartFactory<A> + 'static,
    B: Fn(&mut State) -> EditorSession<A, F> + Send + Sync + 'static,
    S: Fn(&mut State, &mut EditorSession<A, F>) -> Result<bool, String> + Send + Sync + 'static,
    C: Fn(&mut State, CanvasAction) -> Action + Send + Sync + 'static,
{
    type Element = Pod<EditorCanvas<A, F>>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, app: &mut State) -> (Self::Element, ()) {
        (
            ctx.with_action_widget(|ctx| {
                let mut pod = ctx.create_pod(EditorCanvas::new((self.build)(app)));
                pod.new_widget.options.disabled = self.disabled;
                pod
            }),
            (),
        )
    }
    fn rebuild(
        &self,
        _: &Self,
        _: &mut (),
        _: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app: &mut State,
    ) {
        element.ctx.set_disabled(self.disabled);
        match (self.sync)(app, &mut element.widget.session) {
            Ok(changed) => {
                if changed {
                    element.ctx.request_anim_frame();
                    element
                        .ctx
                        .submit_action::<CanvasAction>(element.widget.session.snapshot(None));
                }
            }
            Err(error) => {
                element
                    .ctx
                    .submit_action::<CanvasAction>(element.widget.session.snapshot(Some(error)));
            }
        }
    }
    fn teardown(&self, _: &mut (), ctx: &mut ViewCtx, element: Mut<'_, Self::Element>) {
        element.widget.cancel();
        ctx.teardown_action_source(element);
    }
    fn message(
        &self,
        _: &mut (),
        message: &mut MessageCtx,
        _: Mut<'_, Self::Element>,
        app: &mut State,
    ) -> MessageResult<Action> {
        if message.take_first().is_some() {
            return MessageResult::Stale;
        }
        match message.take_message::<CanvasAction>() {
            Some(action) => MessageResult::Action((self.action)(app, *action)),
            None => MessageResult::Stale,
        }
    }
}
