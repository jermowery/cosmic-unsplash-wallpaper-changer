use std::time::Duration;

use crate::config::{
    ContentFilter, ImageResolution, LastPhotoInfo, Orientation, SettingsStore, UnsplashSettings,
    WallpaperScalingMode, APP_ID,
};
use crate::unsplash::fetch_and_apply_wallpaper;
use crate::wallpaper::set_cosmic_wallpaper;

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

const ICON_SYMBOLIC: &str = "io.github.cosmic.AppletUnsplash-symbolic";

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
                return if let Some(id) = self.popup.take() {
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
                };
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
                    let path = std::path::PathBuf::from(&last.file_path);
                    if path.exists() {
                        let _ = set_cosmic_wallpaper(&path, mode);
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
                if self.is_fetching {
                    return Task::none();
                }
                let _ = self.store.save(&self.settings);
                self.is_fetching = true;
                self.error_message = None;
                self.status_message = Some("Fetching wallpaper from Unsplash...".to_string());
                let current_settings = self.settings.clone();
                return cosmic::task::future(async move {
                    let result = fetch_and_apply_wallpaper(current_settings).await;
                    Message::FetchCompleted(result)
                });
            }
            Message::FetchCompleted(result) => {
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

    fn dialog_content(&self) -> Element<'_, Message> {
        let Spacing {
            space_xxs,
            space_xs,
            space_s,
            space_m,
            ..
        } = cosmic::theme::spacing();

        // Top header bar with title & primary action button
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
                    .push(text::caption("COSMIC Desktop Wallpaper Applet"))
                    .width(Length::Fill),
            )
            .push(fetch_btn)
            .align_y(Alignment::Center)
            .spacing(space_s);

        // Status / Error / Attribution banner
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

        // Section 1: Unsplash API Key
        let credentials_section = Column::new()
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
            );

        // Section 2: Search & Filter Parameters (Unsplash API)
        let search_section = Column::new()
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
                    .push(self.orientation_selector()),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Content Safety Filter"))
                    .push(self.content_filter_selector()),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Image Quality / Resolution"))
                    .push(self.resolution_selector()),
            );

        // Section 3: Desktop Wallpaper & Rotation Options
        let desktop_section = Column::new()
            .spacing(space_xs)
            .push(text::heading("Desktop & Rotation"))
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Wallpaper Scaling Mode"))
                    .push(self.scaling_selector()),
            )
            .push(
                Column::new()
                    .spacing(4)
                    .push(text::caption("Auto-Refresh Interval"))
                    .push(self.refresh_interval_selector()),
            );

        // Scrollable form body fills remaining height inside the fixed-height middle container
        // so dynamically added error/status lines never push the footer buttons down.
        let form_body = scrollable(
            Column::new()
                .spacing(space_s)
                .padding([space_s, space_m, space_s, space_s])
                .push(credentials_section)
                .push(divider::horizontal::light())
                .push(search_section)
                .push(divider::horizontal::light())
                .push(desktop_section),
        )
        .height(Length::Fill);

        let mut middle_col = Column::new().spacing(space_xs);
        if has_status {
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

        // Footer buttons
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

    fn orientation_selector(&self) -> Element<'_, Message> {
        let mut r = Row::new().spacing(4);
        for item in Orientation::ALL {
            let active = self.settings.orientation == item;
            let btn = if active {
                button::suggested(item.label())
            } else {
                button::standard(item.label())
            }
            .on_press(Message::OrientationSelected(item));
            r = r.push(btn);
        }
        r.into()
    }

    fn content_filter_selector(&self) -> Element<'_, Message> {
        let mut r = Row::new().spacing(4);
        for item in ContentFilter::ALL {
            let active = self.settings.content_filter == item;
            let btn = if active {
                button::suggested(item.label())
            } else {
                button::standard(item.label())
            }
            .on_press(Message::ContentFilterSelected(item));
            r = r.push(btn);
        }
        r.into()
    }

    fn resolution_selector(&self) -> Element<'_, Message> {
        let mut r = Row::new().spacing(4);
        for item in ImageResolution::ALL {
            let active = self.settings.resolution == item;
            let btn = if active {
                button::suggested(item.label())
            } else {
                button::standard(item.label())
            }
            .on_press(Message::ResolutionSelected(item));
            r = r.push(btn);
        }
        r.into()
    }

    fn scaling_selector(&self) -> Element<'_, Message> {
        let mut r = Row::new().spacing(4);
        for item in WallpaperScalingMode::ALL {
            let active = self.settings.scaling_mode == item;
            let btn = if active {
                button::suggested(item.label())
            } else {
                button::standard(item.label())
            }
            .on_press(Message::ScalingModeSelected(item));
            r = r.push(btn);
        }
        r.into()
    }

    fn refresh_interval_selector(&self) -> Element<'_, Message> {
        let mut r = Row::new().spacing(4);
        for (mins, label) in UnsplashSettings::REFRESH_PRESETS {
            let active = self.settings.auto_refresh_minutes == mins;
            let btn = if active {
                button::suggested(label)
            } else {
                button::standard(label)
            }
            .on_press(Message::AutoRefreshSelected(mins));
            r = r.push(btn);
        }
        r.into()
    }
}
