use std::time::Duration;

use cosmic::app::Core;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Alignment, Length, Limits, Subscription, Task};
use cosmic::widget::{
    button, container, divider, mouse_area, scrollable, secure_input, text, text_input, toggler,
    Column, Row,
};
use cosmic::Element;

use crate::config::{
    ContentFilter, ImageResolution, LastPhotoInfo, Orientation, SettingsStore, UnsplashSettings,
    WallpaperScalingMode, APP_ID,
};
use crate::unsplash::fetch_and_apply_wallpaper;
use crate::wallpaper::set_cosmic_wallpaper;

const ICON_SYMBOLIC: &str = "io.github.jermowery.CosmicExtAppletUnsplash-symbolic";

pub struct UnsplashApplet {
    core: Core,
    store: SettingsStore,
    settings: UnsplashSettings,
    popup: Option<Id>,
    standalone_window: bool,
    hide_access_key: bool,
    is_fetching: bool,
    status_message: Option<String>,
    error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    AccessKeyChanged(String),
    ToggleAccessKeyVisibility,
    QueryChanged(String),
    TopicsChanged(String),
    CollectionsChanged(String),
    UsernameChanged(String),
    OrientationSelected(Orientation),
    ContentFilterSelected(ContentFilter),
    FeaturedToggled(bool),
    ResolutionSelected(ImageResolution),
    ScalingModeSelected(WallpaperScalingMode),
    AutoRefreshSelected(u32),
    SaveSettings,
    FetchNow,
    FetchCompleted(Result<LastPhotoInfo, String>),
    AutoRefreshTick,
}

impl cosmic::Application for UnsplashApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = bool;
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(
        core: Core,
        standalone_window: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let store = SettingsStore::new();
        let settings = store.load();
        let app = Self {
            core,
            store,
            settings,
            popup: None,
            standalone_window,
            hide_access_key: true,
            is_fetching: false,
            status_message: None,
            error_message: None,
        };
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    self.persist_quietly();
                }
            }
            Message::TogglePopup => {
                return self.handle_toggle_popup();
            }
            Message::AccessKeyChanged(val) => {
                self.settings.access_key = val;
            }
            Message::ToggleAccessKeyVisibility => {
                self.hide_access_key = !self.hide_access_key;
            }
            Message::QueryChanged(val) => {
                self.settings.query = val;
            }
            Message::TopicsChanged(val) => {
                self.settings.topics = val;
            }
            Message::CollectionsChanged(val) => {
                self.settings.collections = val;
            }
            Message::UsernameChanged(val) => {
                self.settings.username = val;
            }
            Message::OrientationSelected(orientation) => {
                self.settings.orientation = orientation;
                self.persist_quietly();
            }
            Message::ContentFilterSelected(filter) => {
                self.settings.content_filter = filter;
                self.persist_quietly();
            }
            Message::FeaturedToggled(featured) => {
                self.settings.featured = featured;
                self.persist_quietly();
            }
            Message::ResolutionSelected(res) => {
                self.settings.resolution = res;
                self.persist_quietly();
            }
            Message::ScalingModeSelected(mode) => {
                self.settings.scaling_mode = mode;
                self.persist_quietly();
                if let Some(last) = &self.settings.last_photo {
                    if last.file_path.exists() {
                        let _ = set_cosmic_wallpaper(&last.file_path, mode);
                    }
                }
            }
            Message::AutoRefreshSelected(minutes) => {
                self.settings.auto_refresh_minutes = minutes;
                self.persist_quietly();
            }
            Message::SaveSettings => match self.store.save(&self.settings) {
                Ok(()) => {
                    self.status_message = Some("Settings saved.".to_string());
                    self.error_message = None;
                }
                Err(err) => {
                    self.error_message = Some(format!("Failed to save settings: {err}"));
                }
            },
            Message::FetchNow | Message::AutoRefreshTick => {
                return self.start_wallpaper_fetch();
            }
            Message::FetchCompleted(result) => {
                self.handle_fetch_completed(result);
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let minutes = self.settings.auto_refresh_minutes;
        if minutes > 0 {
            let secs = u64::from(minutes).saturating_mul(60).max(60);
            cosmic::iced::time::every(Duration::from_secs(secs)).map(|_| Message::AutoRefreshTick)
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        if self.standalone_window {
            return container(self.dialog_content())
                .padding(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        mouse_area(
            self.core
                .applet
                .icon_button(ICON_SYMBOLIC)
                .on_press(Message::TogglePopup),
        )
        .on_middle_press(Message::FetchNow)
        .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        self.core
            .applet
            .popup_container(self.dialog_content())
            .limits(
                Limits::NONE
                    .min_width(400.0)
                    .max_width(440.0)
                    .min_height(200.0)
                    .max_height(680.0),
            )
            .into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        if self.standalone_window {
            None
        } else {
            Some(cosmic::applet::style())
        }
    }
}

impl UnsplashApplet {
    fn persist_quietly(&mut self) {
        if let Err(err) = self.store.save(&self.settings) {
            self.error_message = Some(format!("Could not save settings: {err}"));
        }
    }

    fn handle_toggle_popup(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(id) = self.popup.take() {
            self.persist_quietly();
            destroy_popup(id)
        } else {
            let new_id = Id::unique();
            self.popup = Some(new_id);
            let mut popup_settings = self.core.applet.get_popup_settings(
                self.core.main_window_id().unwrap_or(Id::RESERVED),
                new_id,
                None,
                None,
                None,
            );
            popup_settings.positioner.size_limits = Limits::NONE
                .min_width(400.0)
                .max_width(440.0)
                .min_height(200.0)
                .max_height(680.0);
            get_popup(popup_settings)
        }
    }

    fn start_wallpaper_fetch(&mut self) -> Task<cosmic::Action<Message>> {
        if self.is_fetching {
            return Task::none();
        }
        let _ = self.store.save(&self.settings);
        self.is_fetching = true;
        self.error_message = None;
        self.status_message = Some("Fetching wallpaper from Unsplash...".to_string());
        let current_settings = self.settings.clone();
        cosmic::task::future(async move {
            let result = fetch_and_apply_wallpaper(&current_settings)
                .await
                .map_err(|e| e.to_string());
            Message::FetchCompleted(result)
        })
    }

    fn handle_fetch_completed(&mut self, result: Result<LastPhotoInfo, String>) {
        self.is_fetching = false;
        match result {
            Ok(info) => {
                self.status_message = Some(format!(
                    "Wallpaper updated: Photo by {} (@{})",
                    info.photographer, info.username
                ));
                self.error_message = None;
                self.settings.last_photo = Some(info);
                let _ = self.store.save(&self.settings);
            }
            Err(err) => {
                self.status_message = None;
                self.error_message = Some(err);
            }
        }
    }

    fn status_banner(&self, space_xxs: u16) -> Option<Element<'_, Message>> {
        let mut status_col = Column::new().spacing(space_xxs);
        let mut has_status = false;
        if let Some(err) = &self.error_message {
            status_col = status_col.push(text::body(format!("Error: {err}")));
            has_status = true;
        } else if let Some(status) = &self.status_message {
            status_col = status_col.push(text::caption(status.as_str()));
            has_status = true;
        }

        if let Some(last) = &self.settings.last_photo {
            let mut attribution = format!(
                "Current: Photo by {} (@{}) [{}x{}]",
                last.photographer, last.username, last.width, last.height
            );
            if let Some(desc) = &last.description {
                let short_desc: String = desc.chars().take(60).collect();
                attribution = format!("{attribution} — {short_desc}");
            }
            status_col = status_col.push(text::caption(attribution));
            has_status = true;
        }

        has_status.then(|| status_col.into())
    }

    fn credentials_section(&self, space_xs: u16) -> Element<'_, Message> {
        Column::new()
            .spacing(space_xs)
            .push(text::heading("Unsplash API Key"))
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Access Key (Client-ID)"))
                    .push(
                        secure_input(
                            "Paste your Unsplash Access Key...",
                            &self.settings.access_key,
                            Some(Message::ToggleAccessKeyVisibility),
                            self.hide_access_key,
                        )
                        .on_input(Message::AccessKeyChanged),
                    ),
            )
            .into()
    }

    fn search_section(&self, space_xs: u16) -> Element<'_, Message> {
        Column::new()
            .spacing(space_xs)
            .push(text::heading("Search & Filter Options"))
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Search Terms (query)"))
                    .push(
                        text_input(
                            "e.g. mountains, northern lights, space",
                            &self.settings.query,
                        )
                        .on_input(Message::QueryChanged)
                        .on_submit(|_| Message::FetchNow),
                    ),
            )
            .push(
                Row::new()
                    .spacing(space_xs)
                    .push(
                        Column::new()
                            .spacing(4)
                            .width(Length::FillPortion(1))
                            .push(text::caption("Topics (comma-separated)"))
                            .push(
                                text_input("wallpapers, nature", &self.settings.topics)
                                    .on_input(Message::TopicsChanged),
                            ),
                    )
                    .push(
                        Column::new()
                            .spacing(4)
                            .width(Length::FillPortion(1))
                            .push(text::caption("Collections (IDs)"))
                            .push(
                                text_input("e.g. 1053828", &self.settings.collections)
                                    .on_input(Message::CollectionsChanged),
                            ),
                    ),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Photographer Username (optional)"))
                    .push(
                        text_input("e.g. unsplash", &self.settings.username)
                            .on_input(Message::UsernameChanged),
                    ),
            )
            .push(
                Row::new()
                    .align_y(Alignment::Center)
                    .push(text::body("Featured Photos Only").width(Length::Fill))
                    .push(toggler(self.settings.featured).on_toggle(Message::FeaturedToggled)),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Orientation"))
                    .push(segmented_selector(
                        Orientation::ALL.into_iter().map(|o| (o, o.label())),
                        self.settings.orientation,
                        Message::OrientationSelected,
                    )),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Content Safety Filter"))
                    .push(segmented_selector(
                        ContentFilter::ALL.into_iter().map(|c| (c, c.label())),
                        self.settings.content_filter,
                        Message::ContentFilterSelected,
                    )),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Image Quality / Resolution"))
                    .push(segmented_selector(
                        ImageResolution::ALL.into_iter().map(|r| (r, r.label())),
                        self.settings.resolution,
                        Message::ResolutionSelected,
                    )),
            )
            .into()
    }

    fn desktop_section(&self, space_xs: u16) -> Element<'_, Message> {
        Column::new()
            .spacing(space_xs)
            .push(text::heading("Desktop & Rotation"))
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Wallpaper Scaling Mode"))
                    .push(segmented_selector(
                        WallpaperScalingMode::ALL
                            .into_iter()
                            .map(|m| (m, m.label())),
                        self.settings.scaling_mode,
                        Message::ScalingModeSelected,
                    )),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Auto-Refresh Interval"))
                    .push(segmented_selector(
                        UnsplashSettings::REFRESH_PRESETS,
                        self.settings.auto_refresh_minutes,
                        Message::AutoRefreshSelected,
                    )),
            )
            .into()
    }

    fn dialog_content(&self) -> Element<'_, Message> {
        let Spacing {
            space_xxs,
            space_xs,
            space_s,
            space_m,
            ..
        } = cosmic::theme::spacing();

        let fetch_btn_label = if self.is_fetching {
            "Fetching..."
        } else {
            "New Wallpaper"
        };
        let mut fetch_btn = button::suggested(fetch_btn_label);
        if !self.is_fetching {
            fetch_btn = fetch_btn.on_press(Message::FetchNow);
        }

        let header = Row::new()
            .push(
                Column::new()
                    .push(text::title4("Unsplash Wallpaper"))
                    .push(text::caption("Wallpaper Applet for the COSMIC™ Desktop"))
                    .width(Length::Fill),
            )
            .push(fetch_btn)
            .align_y(Alignment::Center)
            .spacing(space_s);

        // Scrollable form body fills remaining height inside the fixed-height middle container
        // so dynamically added error/status lines never push the footer buttons down.
        let form_body = scrollable(
            Column::new()
                .spacing(space_s)
                .padding([space_s, space_m, space_s, space_s])
                .push(self.credentials_section(space_xs))
                .push(divider::horizontal::light())
                .push(self.search_section(space_xs))
                .push(divider::horizontal::light())
                .push(self.desktop_section(space_xs)),
        )
        .height(Length::Fill);

        let mut middle_col = Column::new().spacing(space_xs);
        if let Some(status_col) = self.status_banner(space_xxs) {
            middle_col = middle_col
                .push(status_col)
                .push(divider::horizontal::light());
        }
        let middle_height = if self.standalone_window {
            Length::Fill
        } else {
            Length::Fixed(420.0)
        };
        let middle_section = middle_col.push(form_body).height(middle_height);

        let mut apply_btn = button::suggested("Save & Change Wallpaper");
        if !self.is_fetching {
            apply_btn = apply_btn.on_press(Message::FetchNow);
        }
        let footer = Row::new()
            .spacing(space_xs)
            .push(button::standard("Save Settings").on_press(Message::SaveSettings))
            .push(apply_btn);

        Column::new()
            .spacing(space_xs)
            .padding(space_m)
            .push(header)
            .push(divider::horizontal::default())
            .push(middle_section)
            .push(divider::horizontal::default())
            .push(footer)
            .into()
    }
}

/// Renders a horizontal row of option buttons where the active selection is highlighted.
fn segmented_selector<T, F>(
    options: impl IntoIterator<Item = (T, &'static str)>,
    selected: T,
    on_select: F,
) -> Element<'static, Message>
where
    T: Copy + PartialEq,
    F: Fn(T) -> Message,
{
    options
        .into_iter()
        .fold(Row::new().spacing(4), |row, (value, label)| {
            let btn = if value == selected {
                button::suggested(label)
            } else {
                button::standard(label)
            }
            .on_press(on_select(value));
            row.push(btn)
        })
        .into()
}
