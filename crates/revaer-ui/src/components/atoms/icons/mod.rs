use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconVariant {
    Outline,
    Solid,
}

impl Default for IconVariant {
    fn default() -> Self {
        Self::Outline
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct IconProps {
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub size: Option<AttrValue>,
    #[prop_or_default]
    pub variant: IconVariant,
}

fn size_class(size: &Option<AttrValue>) -> Option<String> {
    size.as_ref().map(|value| {
        let raw = value.as_ref();
        if raw.starts_with("size-") {
            raw.to_string()
        } else {
            format!("size-{raw}")
        }
    })
}

fn icon_svg(props: &IconProps, body: Html) -> Html {
    let mut classes = Classes::new();
    if let Some(size) = size_class(&props.size) {
        classes.push(size);
    }
    classes.extend(props.class.clone());
    let title = props.title.clone();
    let aria_hidden = title.is_none().then_some(AttrValue::from("true"));
    let (fill, stroke) = match props.variant {
        IconVariant::Outline => ("none", "currentColor"),
        IconVariant::Solid => ("currentColor", "currentColor"),
    };
    html! {
        <svg
            class={classes}
            viewBox="0 0 24 24"
            fill={fill}
            stroke={stroke}
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            role="img"
            aria-hidden={aria_hidden}
            aria-label={title.clone()}
        >
            {title.map(|text| html! { <title>{text}</title> }).unwrap_or_default()}
            {body}
        </svg>
    }
}

#[function_component(IconAlertTriangle)]
pub(crate) fn icon_alert_triangle(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="m21.73 18l-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3M12 9v4m0 4h.01" /> },
    )
}

#[function_component(IconArrowDown)]
pub(crate) fn icon_arrow_down(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="M12 5v14m7-7l-7 7l-7-7" /> })
}

#[function_component(IconArrowUp)]
pub(crate) fn icon_arrow_up(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="m5 12l7-7l7 7m-7 7V5" /> })
}

#[function_component(IconCheckCircle2)]
pub(crate) fn icon_check_circle_2(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <circle cx="12" cy="12" r="10" />
            <path d="m9 12l2 2l4-4" />
        </> },
    )
}

#[function_component(IconChevronDown)]
pub(crate) fn icon_chevron_down(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="m6 9l6 6l6-6" /> })
}

#[function_component(IconChevronUp)]
pub(crate) fn icon_chevron_up(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="m18 15l-6-6l-6 6" /> })
}

#[function_component(IconCircleDollarSign)]
pub(crate) fn icon_circle_dollar_sign(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <circle cx="12" cy="12" r="10" />
            <path d="M16 8h-6a2 2 0 1 0 0 4h4a2 2 0 1 1 0 4H8m4 2V6" />
        </> },
    )
}

#[function_component(IconDownload)]
pub(crate) fn icon_download(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M12 15V3m9 12v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <path d="m7 10l5 5l5-5" />
        </> },
    )
}

#[function_component(IconEraser)]
pub(crate) fn icon_eraser(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M21 21H8a2 2 0 0 1-1.42-.587l-3.994-3.999a2 2 0 0 1 0-2.828l10-10a2 2 0 0 1 2.829 0l5.999 6a2 2 0 0 1 0 2.828L12.834 21m-7.752-9.91l8.828 8.828" /> },
    )
}

#[function_component(IconEye)]
pub(crate) fn icon_eye(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M2.062 12.348a1 1 0 0 1 0-.696a10.75 10.75 0 0 1 19.876 0a1 1 0 0 1 0 .696a10.75 10.75 0 0 1-19.876 0" />
            <circle cx="12" cy="12" r="3" />
        </> },
    )
}

#[function_component(IconFile)]
pub(crate) fn icon_file(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
            <path d="M14 2v4a2 2 0 0 0 2 2h4" />
        </> },
    )
}

#[function_component(IconFileText)]
pub(crate) fn icon_file_text(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
            <path d="M14 2v4a2 2 0 0 0 2 2h4M10 9H8m8 4H8m8 4H8" />
        </> },
    )
}

#[function_component(IconFolder)]
pub(crate) fn icon_folder(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /> },
    )
}

#[function_component(IconGlobe2)]
pub(crate) fn icon_globe_2(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M21.54 15H17a2 2 0 0 0-2 2v4.54M7 3.34V5a3 3 0 0 0 3 3a2 2 0 0 1 2 2c0 1.1.9 2 2 2a2 2 0 0 0 2-2c0-1.1.9-2 2-2h3.17M11 21.95V18a2 2 0 0 0-2-2a2 2 0 0 1-2-2v-1a2 2 0 0 0-2-2H2.05" />
            <circle cx="12" cy="12" r="10" />
        </> },
    )
}

#[function_component(IconHome)]
pub(crate) fn icon_home(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" />
            <path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </> },
    )
}

#[function_component(IconLoader)]
pub(crate) fn icon_loader(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M12 2v4m4.2 1.8l2.9-2.9M18 12h4m-5.8 4.2l2.9 2.9M12 18v4m-7.1-2.9l2.9-2.9M2 12h4M4.9 4.9l2.9 2.9" /> },
    )
}

#[function_component(IconLogOut)]
pub(crate) fn icon_log_out(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="m16 17l5-5l-5-5m5 5H9m0 9H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" /> },
    )
}

#[function_component(IconMenu)]
pub(crate) fn icon_menu(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="M4 5h16M4 12h16M4 19h16" /> })
}

#[function_component(IconMessagesSquare)]
pub(crate) fn icon_messages_square(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M16 10a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 14.286V4a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2zm4-1a2 2 0 0 1 2 2v10.286a.71.71 0 0 1-1.212.502l-2.202-2.202A2 2 0 0 0 17.172 19H10a2 2 0 0 1-2-2v-1" /> },
    )
}

#[function_component(IconMoon)]
pub(crate) fn icon_moon(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401" /> },
    )
}

#[function_component(IconMoreHorizontal)]
pub(crate) fn icon_more_horizontal(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <circle cx="12" cy="12" r="1" />
            <circle cx="19" cy="12" r="1" />
            <circle cx="5" cy="12" r="1" />
        </> },
    )
}

#[function_component(IconPackage)]
pub(crate) fn icon_package(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73zm1 .27V12" />
            <path d="M3.29 7L12 12l8.71-5M7.5 4.27l9 5.15" />
        </> },
    )
}

#[function_component(IconPanelLeftClose)]
pub(crate) fn icon_panel_left_close(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <rect x="3" y="3" rx="2" />
            <path d="M9 3v18m7-6l-3-3l3-3" />
        </> },
    )
}

#[function_component(IconPanelLeftDashed)]
pub(crate) fn icon_panel_left_dashed(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <rect x="3" y="3" rx="2" />
            <path d="M9 14v1m0 4v2M9 3v2m0 4v1" />
        </> },
    )
}

#[function_component(IconPlus)]
pub(crate) fn icon_plus(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="M5 12h14m-7-7v14" /> })
}

#[function_component(IconRefreshCw)]
pub(crate) fn icon_refresh_cw(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M3 12a9 9 0 0 1 9-9a9.75 9.75 0 0 1 6.74 2.74L21 8" />
            <path d="M21 3v5h-5m5 4a9 9 0 0 1-9 9a9.75 9.75 0 0 1-6.74-2.74L3 16" />
            <path d="M8 16H3v5" />
        </> },
    )
}

#[function_component(IconSearch)]
pub(crate) fn icon_search(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="m21 21l-4.34-4.34" />
            <circle cx="11" cy="11" r="8" />
        </> },
    )
}

#[function_component(IconServer)]
pub(crate) fn icon_server(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <rect x="2" y="2" rx="2" ry="2" />
            <rect x="2" y="14" rx="2" ry="2" />
            <path d="M6 6h.01M6 18h.01" />
        </> },
    )
}

#[function_component(IconSettings)]
pub(crate) fn icon_settings(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0a2.34 2.34 0 0 0 3.319 1.915a2.34 2.34 0 0 1 2.33 4.033a2.34 2.34 0 0 0 0 3.831a2.34 2.34 0 0 1-2.33 4.033a2.34 2.34 0 0 0-3.319 1.915a2.34 2.34 0 0 1-4.659 0a2.34 2.34 0 0 0-3.32-1.915a2.34 2.34 0 0 1-2.33-4.033a2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915" />
            <circle cx="12" cy="12" r="3" />
        </> },
    )
}

#[function_component(IconShoppingBag)]
pub(crate) fn icon_shopping_bag(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M16 10a4 4 0 0 1-8 0M3.103 6.034h17.794" />
            <path d="M3.4 5.467a2 2 0 0 0-.4 1.2V20a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6.667a2 2 0 0 0-.4-1.2l-2-2.667A2 2 0 0 0 17 2H7a2 2 0 0 0-1.6.8z" />
        </> },
    )
}

#[function_component(IconSun)]
pub(crate) fn icon_sun(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32l1.41 1.41M2 12h2m16 0h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
        </> },
    )
}

#[function_component(IconTrash)]
pub(crate) fn icon_trash(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /> },
    )
}

#[function_component(IconUnplug)]
pub(crate) fn icon_unplug(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="m19 5l3-3M2 22l3-3m1.3 1.3a2.4 2.4 0 0 0 3.4 0L12 18l-6-6l-2.3 2.3a2.4 2.4 0 0 0 0 3.4Zm1.2-6.8L10 11m.5 5.5L13 14m-1-8l6 6l2.3-2.3a2.4 2.4 0 0 0 0-3.4l-2.6-2.6a2.4 2.4 0 0 0-3.4 0Z" /> },
    )
}

#[function_component(IconUpload)]
pub(crate) fn icon_upload(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <path d="M12 3v12m5-7l-5-5l-5 5m14 7v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /> },
    )
}

#[function_component(IconUsers)]
pub(crate) fn icon_users(props: &IconProps) -> Html {
    icon_svg(
        props,
        html! { <>
            <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M16 3.128a4 4 0 0 1 0 7.744M22 21v-2a4 4 0 0 0-3-3.87" />
            <circle cx="9" cy="7" r="4" />
        </> },
    )
}

#[function_component(IconX)]
pub(crate) fn icon_x(props: &IconProps) -> Html {
    icon_svg(props, html! { <path d="M18 6L6 18M6 6l12 12" /> })
}
