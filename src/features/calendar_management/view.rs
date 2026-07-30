use cosmic::{iced::Length, theme::spacing, widget, Element};

use crate::app::Message;
use crate::features::calendar_management::state::CalendarManagerState;
use crate::fl;

/// Single-form drawer content for creating a new calendar or editing an
/// existing one's name/color. Per-calendar listing, visibility toggling, and
/// deletion now live in the nav bar's own rows and context menu.
pub fn view<'a>(state: &'a CalendarManagerState) -> Element<'a, Message> {
    let mut content = widget::column(Vec::new())
        .spacing(spacing().space_s)
        .width(Length::Fill);

    if let Some(editing) = &state.editing {
        content = content
            .push(
                widget::text_input("", &editing.name)
                    .on_input(Message::CalendarEditNameChanged)
                    .width(Length::Fill),
            )
            .push(
                widget::text_input("#rrggbb", &editing.color)
                    .on_input(Message::CalendarEditColorChanged)
                    .width(Length::Fill),
            )
            .push(
                widget::row(Vec::new())
                    .spacing(spacing().space_xs)
                    .push(widget::button::suggested(fl!("save")).on_press(Message::SaveCalendarEdit))
                    .push(
                        widget::button::standard(fl!("cancel")).on_press(Message::CancelCalendarEdit),
                    ),
            );
    } else {
        content = content
            .push(widget::text::body(fl!("new-calendar-name")))
            .push(
                widget::text_input("", &state.new_name)
                    .on_input(Message::CalendarManagerNewNameChanged)
                    .width(Length::Fill),
            )
            .push(widget::button::suggested(fl!("create")).on_press(Message::CreateCalendar));
    }

    content.into()
}
