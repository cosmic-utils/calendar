use cosmic::{iced::Length, theme::spacing, widget, Element};

use crate::app::{CalendarManagerState, Message};
use crate::fl;
use crate::models::LocalCalendarMeta;

pub fn view<'a>(
    calendars: &[&LocalCalendarMeta],
    state: &'a CalendarManagerState,
) -> Element<'a, Message> {
    let mut content = widget::column(Vec::new())
        .spacing(spacing().space_s)
        .width(Length::Fill);

    let can_delete = calendars.len() > 1;

    for calendar in calendars {
        let id = calendar.id;
        let is_editing = state
            .editing
            .as_ref()
            .is_some_and(|editing| editing.id == id);

        let row: Element<'a, Message> = if is_editing {
            let editing = state.editing.as_ref().unwrap();
            widget::column(Vec::new())
                .spacing(spacing().space_xxs)
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
                        .push(
                            widget::button::suggested(fl!("save")).on_press(Message::SaveCalendarEdit),
                        )
                        .push(
                            widget::button::standard(fl!("cancel"))
                                .on_press(Message::CancelCalendarEdit),
                        ),
                )
                .into()
        } else {
            let mut actions = widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .align_y(cosmic::iced::alignment::Vertical::Center)
                .push(
                    widget::toggler(calendar.visible)
                        .on_toggle(move |visible| Message::ToggleCalendarVisibility(id, visible)),
                )
                .push(widget::text::body(calendar.name.clone()).width(Length::Fill))
                .push(
                    widget::button::standard(fl!("rename")).on_press(Message::StartEditCalendar(id)),
                );

            if can_delete {
                actions = actions.push(
                    widget::button::destructive(fl!("delete")).on_press(Message::DeleteCalendar(id)),
                );
            }

            actions.into()
        };

        content = content.push(row).push(widget::divider::horizontal::default());
    }

    content = content
        .push(widget::text::body(fl!("new-calendar-name")))
        .push(
            widget::row(Vec::new())
                .spacing(spacing().space_xs)
                .push(
                    widget::text_input("", &state.new_name)
                        .on_input(Message::CalendarManagerNewNameChanged)
                        .width(Length::Fill),
                )
                .push(widget::button::suggested(fl!("create")).on_press(Message::CreateCalendar)),
        );

    content.into()
}
