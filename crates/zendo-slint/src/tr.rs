//! The window's words in Slint: the `Tr` global's lookups, answered from the
//! Fluent bundles in `zendo-shell`. The same messages the Iced window shows.

use slint::ComponentHandle;
use zendo_shell::fluent_bundle::{FluentArgs, FluentValue};
use zendo_shell::i18n::{self, Language};

use crate::{MainWindow, Tr};

/// Answers the `Tr` global's lookups. Each one takes the language as its
/// first argument only so a change of language makes Slint ask again.
pub fn install(window: &MainWindow) {
    let tr = window.global::<Tr>();
    tr.on_lookup(|_, id| i18n::tr(&id, None).into());
    tr.on_lookup_with(|_, id, name, value| {
        let mut args = FluentArgs::new();
        args.set(name.as_str(), value.as_str());
        i18n::tr(&id, Some(&args)).into()
    });
    tr.on_lookup_count(|_, id, name, number| {
        let mut args = FluentArgs::new();
        args.set(name.as_str(), FluentValue::from(number));
        i18n::tr(&id, Some(&args)).into()
    });
    show(window, i18n::current());
}

/// Changes the window's language: every text asks again.
pub fn show(window: &MainWindow, language: Language) {
    i18n::set(language);
    let index = Language::ALL
        .iter()
        .position(|known| *known == language)
        .unwrap_or_default();
    window
        .global::<Tr>()
        .set_language(i32::try_from(index).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Page;

    /// One test touches the language, which is global: a second would race
    /// it.
    #[test]
    fn a_change_of_language_changes_every_text_at_once() {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().unwrap();
        install(&window);

        show(&window, Language::English);
        assert_eq!(window.get_page_title(), "Library");
        window.set_page(Page::Store);
        assert_eq!(window.get_page_title(), "Store");

        show(&window, Language::Portuguese);
        assert_eq!(window.get_page_title(), "Loja");
        window.set_page(Page::Library);
        assert_eq!(window.get_page_title(), "Biblioteca");

        // A plain lookup, asked directly (the ones with a value in them,
        // `lookup-with` and `lookup-count`, come with the first page that
        // uses one).
        let tr = window.global::<Tr>();
        assert_eq!(tr.invoke_lookup(0, "page-stats".into()), "Estatísticas");
        show(&window, Language::English);
        assert_eq!(tr.invoke_lookup(0, "page-stats".into()), "Stats");
    }
}
