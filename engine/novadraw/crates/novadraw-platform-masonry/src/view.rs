use crate::session::{CanvasAction, EditorSession};
use crate::widget::EditorCanvas;
use novadraw_editor::{EditPartFactory, ModelAdapter};
use std::marker::PhantomData;
use std::sync::Arc;
use xilem_masonry::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::{Pod, ViewCtx};

/// Send+Sync description; callbacks access session state only on the UI thread.
pub struct CanvasView<
    State,
    A: ModelAdapter + 'static,
    F: EditPartFactory<A>,
    Build,
    Sync,
    ActionFn,
> {
    build: Build,
    sync: Sync,
    action: ActionFn,
    disabled: bool,
    project: Option<Projection<State, A, F>>,
    key_handler: Option<crate::widget::KeyHandler<A, F>>,
    marker: PhantomData<fn(State) -> (A, F)>,
}

type Projection<State, A, F> =
    Arc<dyn Fn(&mut State, &EditorSession<A, F>, novadraw::geometry::Rectangle) + Send + Sync>;

pub fn editor_canvas<
    State,
    A: ModelAdapter + 'static,
    F: EditPartFactory<A>,
    Build,
    Sync,
    ActionFn,
>(
    build: Build,
    sync: Sync,
    action: ActionFn,
) -> CanvasView<State, A, F, Build, Sync, ActionFn> {
    CanvasView {
        build,
        sync,
        action,
        disabled: false,
        project: None,
        key_handler: None,
        marker: PhantomData,
    }
}

impl<State, A: ModelAdapter + 'static, F: EditPartFactory<A>, B, S, C>
    CanvasView<State, A, F, B, S, C>
{
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Handles application shortcuts while this canvas owns keyboard focus.
    /// Returning false leaves the key available to the enclosing native UI.
    pub fn on_key(
        mut self,
        handler: impl Fn(
            &mut EditorSession<A, F>,
            &masonry::core::keyboard::KeyboardEvent,
        ) -> Result<bool, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.key_handler = Some(Arc::new(handler));
        self
    }

    /// Projects application-owned model state after a stable canvas action.
    /// Window bounds allow a native drag source to route a drop to this canvas.
    pub fn project(
        mut self,
        callback: impl Fn(&mut State, &EditorSession<A, F>, novadraw::geometry::Rectangle)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.project = Some(Arc::new(callback));
        self
    }
}

impl<State, A: ModelAdapter + 'static, F: EditPartFactory<A>, B, S, C> ViewMarker
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
                let mut widget = EditorCanvas::new((self.build)(app));
                widget.key_handler = self.key_handler.clone();
                let mut pod = ctx.create_pod(widget);
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
        element.widget.key_handler = self.key_handler.clone();
        match (self.sync)(app, &mut element.widget.session) {
            Ok(changed) => {
                if changed {
                    element.widget.set_error(None);
                    element.ctx.request_anim_frame();
                    if let Some(action) = element.widget.take_action() {
                        element.ctx.submit_action::<CanvasAction>(action);
                    }
                }
            }
            Err(error) => {
                element.widget.set_error(Some(error));
                if let Some(action) = element.widget.take_action() {
                    element.ctx.submit_action::<CanvasAction>(action);
                }
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
        element: Mut<'_, Self::Element>,
        app: &mut State,
    ) -> MessageResult<Action> {
        if message.take_first().is_some() {
            return MessageResult::Stale;
        }
        match message.take_message::<CanvasAction>() {
            Some(action) => {
                if let Some(project) = &self.project {
                    let origin = element.ctx.to_window(masonry::kurbo::Point::ZERO);
                    let size = element.widget.size;
                    project(
                        app,
                        &element.widget.session,
                        novadraw::geometry::Rectangle::new(
                            origin.x,
                            origin.y,
                            size.width,
                            size.height,
                        ),
                    );
                }
                MessageResult::Action((self.action)(app, *action))
            }
            None => MessageResult::Stale,
        }
    }
}
