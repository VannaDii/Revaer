//! Locale selection dropdown used in the top bar.
//!
//! # Design
//! - Keep presentation focused on UI; selection state is managed by the caller.
//! - Use daisyUI dropdown classes for consistent styling with the Nexus layout.
//! - Avoid side effects inside the component; emit selected locale via callback.

use crate::components::daisy::Dropdown;
use crate::core::logic::locale::locale_flag;
use crate::i18n::LocaleCode;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct LocaleMenuProps {
    pub locale: LocaleCode,
    pub on_select: Callback<LocaleCode>,
}

#[function_component(LocaleMenu)]
pub(crate) fn locale_menu(props: &LocaleMenuProps) -> Html {
    let active_flag = locale_flag(props.locale);
    let active_flag_src = format!("https://flagcdn.com/{active_flag}.svg");

    html! {
        <Dropdown
            class={classes!("dropdown-bottom", "dropdown-center")}
            trigger_label={Some(AttrValue::from("Locale"))}
            trigger_class={classes!("btn-ghost", "btn-circle", "btn-sm", "cursor-pointer")}
            content_class={classes!(
                "mt-2",
                "w-40",
                "p-2",
                "shadow",
                "z-50",
                "locale-menu__content"
            )}
            trigger={html! {
                <img
                    src={active_flag_src}
                    alt="Locale"
                    class="rounded-full size-4.5 object-cover"
                />
            }}
        >
            {for LocaleCode::all().iter().map(|lc| {
                let flag = locale_flag(*lc);
                let flag_src = format!("https://flagcdn.com/{flag}.svg");
                let label = lc.label();
                let next = *lc;
                let on_select = props.on_select.clone();
                let onclick = Callback::from(move |_| on_select.emit(next));
                html! {
                    <li>
                        <button type="button" class="flex items-center gap-2" onclick={onclick}>
                            <img
                                src={flag_src}
                                alt="Locale"
                                class="rounded-full size-4.5 object-cover"
                            />
                            <span>{label}</span>
                        </button>
                    </li>
                }
            })}
        </Dropdown>
    }
}
