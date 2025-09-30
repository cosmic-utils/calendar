// SPDX-License-Identifier: GPL-3.0-only

use crate::components::LocalCalendar;
use crate::config::Config;
use crate::fl;
use crate::models::Calendar;
use crate::services::CalendarServiceFactory;
use crate::Result;
use accounts::models::Account;
use accounts::AccountsClient;
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::prelude::*;
use cosmic::theme::spacing;
use cosmic::widget::segmented_button::SingleSelect;
use cosmic::widget::{self, menu, nav_bar};
use cosmic::{cosmic_theme, theme};
use futures_util::SinkExt;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::Index;
use time::OffsetDateTime;

mod flags;
pub use flags::*;
mod settings;
pub use settings::*;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// Contains items assigned to the nav bar panel.
    nav: nav_bar::Model,
    /// Contains items assigned to the tab bar.
    tabs: widget::segmented_button::Model<SingleSelect>,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    // Configuration data that persists between application runs.
    config: Config,
    // Calendar data that persists between application runs.
    calendar: LocalCalendar,
    client: Option<AccountsClient>,
    accounts: VecDeque<Account>,
    calendars: BTreeMap<Account, Vec<Calendar>>,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    OpenRepositoryUrl,
    SubscriptionChannel,
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    TabSelected(widget::segmented_button::Entity),
    LaunchUrl(String),
    NavigateNextDay,
    NavigatePreviousDay,
    NavigateNextMonth,
    NavigatePreviousMonth,
    NavigateNextYear,
    NavigatePreviousYear,
    NavigateToday,
    AddEvent(OffsetDateTime),
    SelectDate(OffsetDateTime),
    SelectMonth(usize),
    SelectYear(usize),
    SelectDay(usize),
    LoadClient,
    SetClient(Option<AccountsClient>),
    LoadAccounts,
    SetAccounts(VecDeque<Account>),
    LoadCalendars,
    AddCalendars((Account, Vec<Calendar>)),
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = Flags;

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "dev.edfloreshz.Calendar";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Create a nav bar with three page items.
        let nav = nav_bar::Model::default();

        // Create a tab bar with three page items.
        let tabs = widget::segmented_button::Model::builder()
            .insert(|b| {
                b.text(fl!("month"))
                    .icon(widget::icon::from_name("office-calendar-symbolic"))
                    .data(Tab::Month)
                    .activate()
            })
            .insert(|b| {
                b.text(fl!("week"))
                    .icon(widget::icon::from_name("x-office-spreadsheet-symbolic"))
                    .data(Tab::Week)
            })
            .insert(|b| {
                b.text(fl!("day"))
                    .icon(widget::icon::from_name("calendar-go-today-symbolic"))
                    .data(Tab::Day)
            })
            .build();

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            nav,
            tabs,
            key_binds: HashMap::new(),
            // Optional configuration file for an application.
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => {
                        // for why in errors {
                        //     tracing::error!(%why, "error loading app config");
                        // }

                        config
                    }
                })
                .unwrap_or_default(),
            calendar: LocalCalendar::default(),
            client: None,
            accounts: VecDeque::new(),
            calendars: BTreeMap::new(),
        };

        app.core.nav_bar_set_toggled(false);

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (
            app,
            Task::batch(vec![command, cosmic::task::message(Message::LoadClient)]),
        )
    }

    /// Elements to pack at the start of the header bar.
    fn header_start<'a>(&'a self) -> Vec<Element<'a, Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button(fl!("about"), None, MenuAction::About)],
            ),
        )]);

        vec![menu_bar.into()]
    }

    fn header_center<'a>(&'a self) -> Vec<Element<'a, Self::Message>> {
        vec![widget::text(format!(
            "{} {}",
            self.calendar.selected_date.month(),
            self.calendar.selected_date.year()
        ))
        .width(Length::Fill)
        .into()]
    }

    fn footer<'a>(&'a self) -> Option<Element<'a, Self::Message>> {
        Some(
            widget::container(
                widget::row()
                    .push(
                        widget::button::icon(widget::icon::from_name("calendar-go-today-symbolic"))
                            .tooltip(fl!("today"))
                            .class(cosmic::style::Button::NavToggle)
                            .on_press_maybe(
                                (!self.calendar.today()).then_some(Message::NavigateToday),
                            ),
                    )
                    .push(widget::horizontal_space())
                    .push(
                        widget::row()
                            .push(
                                widget::button::icon(widget::icon::from_name(
                                    "go-previous-symbolic",
                                ))
                                .on_press(Message::NavigatePreviousDay),
                            )
                            .push(widget::dropdown(
                                self.calendar
                                    .days()
                                    .iter()
                                    .map(|m| m.to_string())
                                    .collect::<Vec<String>>(),
                                self.calendar
                                    .days()
                                    .iter()
                                    .position(|m| *m == self.calendar.selected_date.day()),
                                Message::SelectDay,
                            ))
                            .push(
                                widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                                    .on_press(Message::NavigateNextDay),
                            )
                            .push(
                                widget::button::icon(widget::icon::from_name(
                                    "go-previous-symbolic",
                                ))
                                .on_press(Message::NavigatePreviousMonth),
                            )
                            .push(widget::dropdown(
                                self.calendar
                                    .months()
                                    .iter()
                                    .map(|m| m.to_string())
                                    .collect::<Vec<String>>(),
                                self.calendar
                                    .months()
                                    .iter()
                                    .position(|m| *m == self.calendar.selected_date.month()),
                                Message::SelectMonth,
                            ))
                            .push(
                                widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                                    .on_press(Message::NavigateNextMonth),
                            )
                            .push(
                                widget::button::icon(widget::icon::from_name(
                                    "go-previous-symbolic",
                                ))
                                .on_press(Message::NavigatePreviousYear),
                            )
                            .push(widget::dropdown(
                                self.calendar
                                    .years()
                                    .iter()
                                    .map(|m| m.to_string())
                                    .collect::<Vec<String>>(),
                                self.calendar
                                    .years()
                                    .iter()
                                    .position(|y| *y == self.calendar.selected_date.year()),
                                Message::SelectYear,
                            ))
                            .push(
                                widget::button::icon(widget::icon::from_name("go-next-symbolic"))
                                    .on_press(Message::NavigateNextYear),
                            )
                            .align_y(Vertical::Center)
                            .spacing(spacing().space_xxs),
                    )
                    .push(widget::horizontal_space())
                    .push(
                        widget::row()
                            .push(
                                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                                    .tooltip(fl!("crate-event"))
                                    .on_press(Message::AddEvent(
                                        OffsetDateTime::now_local().unwrap(),
                                    )),
                            )
                            .align_y(Vertical::Center)
                            .spacing(spacing().space_xxs),
                    ),
            )
            .align_x(Horizontal::Center)
            .class(cosmic::theme::Container::Card)
            .padding(spacing().space_xxxs)
            .width(Length::Fill)
            .into(),
        )
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer<'a>(&'a self) -> Option<context_drawer::ContextDrawer<'a, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::context_drawer(
                self.about(),
                Message::ToggleContextPage(ContextPage::About),
            )
            .title(fl!("about")),
        })
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// Application events will be processed through the view. Any messages emitted by
    /// events received by widgets will be passed to the update method.
    fn view<'a>(&'a self) -> Element<'a, Self::Message> {
        let tabs = widget::segmented_button::horizontal(&self.tabs)
            .padding(2)
            .button_spacing(spacing().space_xxs)
            .button_alignment(cosmic::iced::Alignment::Center)
            .on_activate(Message::TabSelected);

        let active_tab = match self.tabs.active_data::<Tab>() {
            Some(active_tab) => match active_tab {
                Tab::Month => self.calendar.month_view().into(),
                Tab::Week => self.calendar.week_view().into(),
                Tab::Day => self.calendar.day_view().into(),
            },
            None => widget::text::title1("Welcome")
                .apply(widget::container)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .into(),
        };

        widget::column()
            .push(tabs)
            .push(active_tab)
            .spacing(spacing().space_xxs)
            .into()
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background which
    /// emit messages to the application through a channel. They are started at the
    /// beginning of the application, and persist through its lifetime.
    fn subscription(&self) -> Subscription<Self::Message> {
        struct MySubscription;

        Subscription::batch(vec![
            // Create a subscription which emits updates through a channel.
            Subscription::run_with_id(
                std::any::TypeId::of::<MySubscription>(),
                cosmic::iced::stream::channel(4, move |mut channel| async move {
                    _ = channel.send(Message::SubscriptionChannel).await;

                    futures_util::future::pending().await
                }),
            ),
            // Watch for application configuration changes.
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| {
                    // for why in update.errors {
                    //     tracing::error!(?why, "app config error");
                    // }

                    Message::UpdateConfig(update.config)
                }),
        ])
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        let mut tasks = vec![];
        match message {
            Message::TabSelected(tab) => {
                self.tabs.activate(tab);
            }
            Message::OpenRepositoryUrl => {
                _ = open::that_detached(REPOSITORY);
            }
            Message::SubscriptionChannel => {
                // For example purposes only.
            }
            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    // Close the context drawer if the toggled context page is the same.
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    // Open the context drawer to display the requested context page.
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }
            Message::UpdateConfig(config) => {
                self.config = config;
            }
            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },
            Message::LoadClient => tasks.push(Task::perform(
                async { AccountsClient::new().await.ok() },
                |client| cosmic::action::app(Message::SetClient(client)),
            )),
            Message::SetClient(client) => {
                self.client = client;
                tasks.push(cosmic::task::message(Message::LoadAccounts));
            }
            Message::LoadAccounts => {
                if let Some(client) = self.client.as_ref() {
                    let client = client.clone();
                    tasks.push(Task::perform(
                        async move {
                            let accounts = client.list_accounts().await?;
                            Ok(VecDeque::from(accounts))
                        },
                        |accounts: Result<VecDeque<Account>>| match accounts {
                            Ok(accounts) => cosmic::action::app(Message::SetAccounts(accounts)),
                            Err(err) => {
                                tracing::error!("Failed to load accounts: {}", err);
                                cosmic::action::none()
                            }
                        },
                    ));
                }
            }
            Message::SetAccounts(accounts) => {
                self.accounts = accounts;
                tasks.push(cosmic::task::message(Message::LoadCalendars));
            }
            Message::LoadCalendars => {
                let accounts = self.accounts.clone();
                for account in accounts {
                    tasks.push(cosmic::Task::perform(
                        async move {
                            let mut service = CalendarServiceFactory::get_service(&account).await?;
                            let calendars = service.fetch_calendars().await?;
                            Ok((account.clone(), calendars))
                        },
                        |calendars: Result<(Account, Vec<Calendar>)>| match calendars {
                            Ok((account, calendars)) => {
                                cosmic::action::app(Message::AddCalendars((account, calendars)))
                            }
                            Err(err) => {
                                tracing::error!("Failed to load calendars: {}", err);
                                cosmic::action::none()
                            }
                        },
                    ));
                }
            }
            Message::AddCalendars((account, calendars)) => {
                self.core.nav_bar_set_toggled(true);
                self.calendars.insert(account.clone(), calendars.clone());
                self.nav.insert().text(account.username);
                for calendar in calendars {
                    self.nav
                        .insert()
                        .indent(1)
                        .text(calendar.name.clone())
                        .icon(widget::icon::from_name("office-calendar-symbolic"))
                        .data(calendar);
                }
            }
            Message::AddEvent(date) => {
                tracing::info!("Adding event on {date}");
            }
            Message::SelectDate(date) => {
                self.calendar.set_date(date);
            }
            Message::SelectDay(idx) => {
                let days = self.calendar.days();
                let day = days.index(idx);
                if let Ok(date) = self.calendar.selected_date.replace_day(*day) {
                    self.calendar.set_date(date);
                } else {
                    tracing::error!("failed to select date");
                }
            }
            Message::SelectMonth(idx) => {
                let months = self.calendar.months();
                let month = months.index(idx);
                if let Ok(date) = self.calendar.selected_date.replace_month(*month) {
                    self.calendar.set_date(date);
                } else {
                    tracing::error!("failed to select date");
                }
            }
            Message::SelectYear(idx) => {
                let years = self.calendar.years();
                let year = years.index(idx);
                if let Ok(date) = self.calendar.selected_date.replace_year(*year) {
                    self.calendar.set_date(date);
                } else {
                    tracing::error!("failed to select date");
                }
            }
            Message::NavigateToday => self.calendar.set_today(),
            Message::NavigateNextDay => {
                if let Err(err) = self.calendar.next_day() {
                    tracing::error!("failed to navigate to next day: {err}");
                }
            }
            Message::NavigatePreviousDay => {
                if let Err(err) = self.calendar.previous_day() {
                    tracing::error!("failed to navigate to previous day: {err}");
                }
            }
            Message::NavigateNextMonth => {
                if let Err(err) = self.calendar.next_month() {
                    tracing::error!("failed to navigate to next month: {err}");
                }
            }
            Message::NavigatePreviousMonth => {
                if let Err(err) = self.calendar.previous_month() {
                    tracing::error!("failed to navigate to previous month: {err}");
                }
            }
            Message::NavigateNextYear => {
                if let Err(err) = self.calendar.next_year() {
                    tracing::error!("failed to navigate to next year: {err}");
                }
            }
            Message::NavigatePreviousYear => {
                if let Err(err) = self.calendar.previous_year() {
                    tracing::error!("failed to navigate to previous year: {err}");
                }
            }
        }
        Task::batch(tasks)
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        // Activate the page in the model.
        self.nav.activate(id);

        self.update_title()
    }
}

impl AppModel {
    /// The about page for this app.
    pub fn about<'a>(&'a self) -> Element<'a, Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

        let icon = widget::svg(widget::svg::Handle::from_memory(APP_ICON));

        let title = widget::text::title3(fl!("app-title"));

        let hash = env!("VERGEN_GIT_SHA");
        let short_hash: String = hash.chars().take(7).collect();
        let date = env!("VERGEN_GIT_COMMIT_DATE");

        let link = widget::button::link(REPOSITORY)
            .on_press(Message::OpenRepositoryUrl)
            .padding(0);

        widget::column()
            .push(icon)
            .push(title)
            .push(link)
            .push(
                widget::button::link(fl!(
                    "git-description",
                    hash = short_hash.as_str(),
                    date = date
                ))
                .on_press(Message::LaunchUrl(format!("{REPOSITORY}/commits/{hash}")))
                .padding(0),
            )
            .align_x(Alignment::Center)
            .spacing(space_xxs)
            .into()
    }

    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }
}

/// The tab to display in the application.
pub enum Tab {
    Month,
    Week,
    Day,
}

/// The context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
        }
    }
}
