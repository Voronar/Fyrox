use crate::grid::{Column, Row};
use crate::message::{MessageDirection, UiMessageData};
use crate::numeric::NumericUpDownMessage;
use crate::text::TextBuilder;
use crate::{
    core::pool::Handle,
    grid::GridBuilder,
    message::UiMessage,
    numeric::{NumericType, NumericUpDownBuilder},
    widget::{Widget, WidgetBuilder},
    BuildContext, Control, Thickness, UiNode, UserInterface, VerticalAlignment,
};
use std::ops::{Deref, DerefMut, Range};

#[derive(Debug, PartialEq)]
pub enum RangeEditorMessage<T>
where
    T: NumericType,
{
    Value(Range<T>),
}

impl<T: NumericType> RangeEditorMessage<T> {
    pub fn value(
        destination: Handle<UiNode>,
        direction: MessageDirection,
        value: Range<T>,
    ) -> UiMessage {
        UiMessage::user(
            destination,
            direction,
            Box::new(RangeEditorMessage::Value(value)),
        )
    }
}

#[derive(Debug, Clone)]
pub struct RangeEditor<T>
where
    T: NumericType,
{
    widget: Widget,
    value: Range<T>,
    start: Handle<UiNode>,
    end: Handle<UiNode>,
}

impl<T> Deref for RangeEditor<T>
where
    T: NumericType,
{
    type Target = Widget;

    fn deref(&self) -> &Self::Target {
        &self.widget
    }
}

impl<T> DerefMut for RangeEditor<T>
where
    T: NumericType,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.widget
    }
}

const SYNC_FLAG: u64 = 1;

impl<T> Control for RangeEditor<T>
where
    T: NumericType,
{
    fn handle_routed_message(&mut self, ui: &mut UserInterface, message: &mut UiMessage) {
        self.widget.handle_routed_message(ui, message);

        if message.direction() == MessageDirection::ToWidget && message.flags != SYNC_FLAG {
            if let UiMessageData::User(msg) = message.data() {
                if let Some(RangeEditorMessage::Value(range)) = msg.cast::<RangeEditorMessage<T>>()
                {
                    if message.destination() == self.handle && self.value != *range {
                        self.value = range.clone();

                        ui.send_message(NumericUpDownMessage::value(
                            self.start,
                            MessageDirection::ToWidget,
                            range.start,
                        ));
                        ui.send_message(NumericUpDownMessage::value(
                            self.end,
                            MessageDirection::ToWidget,
                            range.end,
                        ));

                        ui.send_message(message.reverse());
                    }
                } else if let Some(NumericUpDownMessage::Value(value)) =
                    msg.cast::<NumericUpDownMessage<T>>()
                {
                    if message.destination() == self.start {
                        if *value < self.value.end {
                            ui.send_message(RangeEditorMessage::value(
                                self.handle,
                                MessageDirection::ToWidget,
                                Range {
                                    start: *value,
                                    end: self.value.end,
                                },
                            ));
                        } else {
                            let mut msg = NumericUpDownMessage::value(
                                self.start,
                                MessageDirection::ToWidget,
                                self.value.end,
                            );
                            msg.flags = SYNC_FLAG;
                            ui.send_message(msg);
                        }
                    } else if message.destination() == self.end {
                        if *value > self.value.start {
                            ui.send_message(RangeEditorMessage::value(
                                self.handle,
                                MessageDirection::ToWidget,
                                Range {
                                    start: self.value.start,
                                    end: *value,
                                },
                            ));
                        } else {
                            let mut msg = NumericUpDownMessage::value(
                                self.end,
                                MessageDirection::ToWidget,
                                self.value.start,
                            );
                            msg.flags = SYNC_FLAG;
                            ui.send_message(msg);
                        }
                    }
                }
            }
        }
    }
}

pub struct RangeEditorBuilder<T>
where
    T: NumericType,
{
    widget_builder: WidgetBuilder,
    value: Range<T>,
}

impl<T> RangeEditorBuilder<T>
where
    T: NumericType,
{
    pub fn new(widget_builder: WidgetBuilder) -> Self {
        Self {
            widget_builder,
            value: Range::default(),
        }
    }

    pub fn with_value(mut self, value: Range<T>) -> Self {
        self.value = value;
        self
    }

    pub fn build(self, ctx: &mut BuildContext) -> Handle<UiNode> {
        let start;
        let end;
        let editor = RangeEditor {
            widget: self
                .widget_builder
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child(
                                TextBuilder::new(WidgetBuilder::new().on_column(0))
                                    .with_text("Start")
                                    .with_vertical_text_alignment(VerticalAlignment::Center)
                                    .build(ctx),
                            )
                            .with_child({
                                start = NumericUpDownBuilder::new(
                                    WidgetBuilder::new()
                                        .with_margin(Thickness::uniform(1.0))
                                        .on_column(1),
                                )
                                .with_value(self.value.start)
                                .build(ctx);
                                start
                            })
                            .with_child(
                                TextBuilder::new(WidgetBuilder::new().on_column(2))
                                    .with_vertical_text_alignment(VerticalAlignment::Center)
                                    .with_text("End")
                                    .build(ctx),
                            )
                            .with_child({
                                end = NumericUpDownBuilder::new(
                                    WidgetBuilder::new()
                                        .with_margin(Thickness::uniform(1.0))
                                        .on_column(3),
                                )
                                .with_value(self.value.end)
                                .build(ctx);
                                end
                            }),
                    )
                    .add_column(Column::strict(30.0))
                    .add_column(Column::stretch())
                    .add_column(Column::strict(30.0))
                    .add_column(Column::stretch())
                    .add_row(Row::stretch())
                    .build(ctx),
                )
                .build(),
            value: self.value,
            start,
            end,
        };

        ctx.add_node(UiNode::new(editor))
    }
}
