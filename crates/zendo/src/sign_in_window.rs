//! The sign-in window: a store's login page in a small window of Zendo's
//! own, so signing in needs nothing copied. It runs as a second process,
//! `zendo sign-in-window <store> <url>`, because the web view needs GTK's
//! own loop, apart from Iced's.
//!
//! Steam's is different ([`steam`]): its store page gives a signed-in user
//! a Web API token, which the window writes instead of a code. Its cookies
//! are kept in a folder of Zendo's, so Steam's page remembers the sign-in
//! and gives a new token later in a window that never shows
//! (`steam-renew`).
//!
//! It watches the addresses the page goes to. On the page the store's
//! sign-in ends on ([`stores::code_at`]) it writes the code to stdout and
//! closes; Zendo's window, waiting on it, finishes the sign-in. Closing it
//! ends it with nothing written, as does Zendo's window going away or
//! cancelling, which closes its stdin.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::process::{ChildStdin, Command, ExitCode, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use gtk::prelude::*;
use serde_json::Value;
use webkit2gtk::{SettingsExt, WebViewExt};
use wry::{
    NewWindowResponse, PageLoadEvent, WebContext, WebViewBuilder, WebViewBuilderExtUnix,
    WebViewExtUnix,
};
use zendo_core::steam_web::{self, Session};
use zendo_core::stores::{self, Store};

/// How the window says it was closed before the sign-in ended.
const CLOSED: u8 = 3;

/// `WebKitGTK`'s DMA-BUF renderer draws a blank page on some drivers
/// (NVIDIA's, on Wayland); a login page does not need it.
const NO_DMABUF: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// Run on every page; it only speaks on the page a sign-in ends on, giving
/// its address and, on Epic's, which is JSON, its text.
const SCRIPT: &str = r"
window.addEventListener('DOMContentLoaded', () => {
  const address = location.href;
  const epic = location.hostname === 'www.epicgames.com'
    && location.pathname.startsWith('/id/api/redirect');
  if (epic
      || address.startsWith('https://embed.gog.com/on_login_success')
      || address.includes('openid.oa2.authorization_code=')) {
    const text = epic && document.body ? document.body.innerText : '';
    window.ipc.postMessage(JSON.stringify({ address, text }));
  }
});
";

/// Takes in a page's address, and its text when it has it; true once the
/// code is found.
type Found = dyn Fn(&str, Option<&str>) -> bool;

fn store_named(name: &str) -> Option<Store> {
    match name {
        "epic" => Some(Store::Epic),
        "gog" => Some(Store::Gog),
        "amazon" => Some(Store::Amazon),
        _ => None,
    }
}

fn name_of(store: Store) -> &'static str {
    match store {
        Store::Epic => "epic",
        Store::Gog => "gog",
        Store::Amazon => "amazon",
    }
}

/// `zendo sign-in-window <store> <url>`: shows the page until the sign-in
/// ends or the window is closed.
pub fn main(store: &str, url: &str) -> ExitCode {
    let shown = match store {
        "steam" => steam::show(false, url),
        "steam-renew" => steam::show(true, url),
        name => {
            let Some(store) = store_named(name) else {
                eprintln!("zendo: no store called {name}");
                return ExitCode::from(2);
            };
            show(store, url)
        }
    };
    match shown {
        Ok(Some(code)) => {
            let mut stdout = std::io::stdout();
            if writeln!(stdout, "{code}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::from(CLOSED),
        Err(err) => {
            eprintln!("zendo: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn show(store: Store, url: &str) -> anyhow::Result<Option<String>> {
    // GTK takes the window's application id (Wayland) or class (X11) from
    // the program's name: Zendo's, as its main window has.
    gtk::glib::set_prgname(Some(crate::app::APP_ID));
    gtk::glib::set_application_name("Zendo");
    gtk::init().context("could not start GTK")?;
    crate::i18n::set_from_settings();
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title(&crate::i18n::t!(
        "sign-in-window-title",
        store = store.label()
    ));
    window.set_default_size(560, 760);
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add(&container);

    let code: Rc<RefCell<Option<String>>> = Rc::default();
    let found: Rc<Found> = {
        let code = Rc::clone(&code);
        Rc::new(move |address, text| {
            if code.borrow().is_some() {
                return true;
            }
            let Some(found) = stores::code_at(store, address, text) else {
                return false;
            };
            *code.borrow_mut() = Some(found);
            gtk::main_quit();
            true
        })
    };
    let popups: Popups = Rc::default();
    let webview = web_view(&container, Some(url), None, &found, &popups)
        .context("could not open the store's page")?;
    allow_popups(&webview.webview());

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        gtk::glib::Propagation::Proceed
    });
    // Zendo's window holds the other end; it closes on Cancel or exit.
    std::thread::spawn(|| {
        let mut stdin = std::io::stdin();
        let mut sink = [0_u8; 64];
        while matches!(stdin.read(&mut sink), Ok(read) if read > 0) {}
        gtk::glib::idle_add_once(gtk::main_quit);
    });

    window.show_all();
    gtk::main();
    popups.borrow_mut().clear();
    drop(webview);
    let code = code.borrow_mut().take();
    Ok(code)
}

/// The pop-ups open, kept alive with their windows.
type Popups = Rc<RefCell<Vec<wry::WebView>>>;

/// A web view watching for the code: the store's page, or with `related`,
/// a pop-up it opened, which the engine loads itself.
fn web_view(
    container: &gtk::Box,
    url: Option<&str>,
    related: Option<webkit2gtk::WebView>,
    found: &Rc<Found>,
    popups: &Popups,
) -> wry::Result<wry::WebView> {
    let mut builder = WebViewBuilder::new()
        // Cookies and caches kept in memory and gone with the window: the
        // engine keeps the sign-in, nothing else needs to. A pop-up shares
        // its opener's session.
        .with_incognito(true)
        .with_initialization_script(SCRIPT)
        // The last page is not even loaded once its address holds the code.
        .with_navigation_handler({
            let found = Rc::clone(found);
            move |address| !found(&address, None)
        })
        // A redirect can land there without a navigation of its own.
        .with_on_page_load_handler({
            let found = Rc::clone(found);
            move |_: PageLoadEvent, address| {
                found(&address, None);
            }
        })
        .with_ipc_handler({
            let found = Rc::clone(found);
            move |request| {
                let Ok(page) = serde_json::from_str::<Value>(request.body()) else {
                    return;
                };
                if let Some(address) = page.get("address").and_then(Value::as_str) {
                    found(address, page.get("text").and_then(Value::as_str));
                }
            }
        })
        // "Sign in with Google" and the like open a pop-up, which reports
        // back to the page that opened it.
        .with_new_window_req_handler({
            let found = Rc::clone(found);
            let popups = Rc::clone(popups);
            move |_, features| match popup(&features.opener.webview, &found, &popups) {
                Some(webview) => NewWindowResponse::Create { webview },
                None => NewWindowResponse::Deny,
            }
        });
    if let Some(url) = url {
        builder = builder.with_url(url);
    }
    if let Some(related) = related {
        builder = builder.with_related_view(related);
    }
    builder.build_gtk(container)
}

/// Lets the page open its pop-up even after a step of its own, not only
/// straight from a click: some sign-ins open it once a script has run.
fn allow_popups(view: &webkit2gtk::WebView) {
    if let Some(settings) = WebViewExt::settings(view) {
        settings.set_javascript_can_open_windows_automatically(true);
    }
}

/// Opens a pop-up's window, closed when its page closes it.
fn popup(
    opener: &webkit2gtk::WebView,
    found: &Rc<Found>,
    popups: &Popups,
) -> Option<webkit2gtk::WebView> {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title(&crate::i18n::t!("sign-in-popup-title"));
    window.set_default_size(520, 680);
    if let Some(parent) = opener
        .toplevel()
        .and_then(|toplevel| toplevel.downcast::<gtk::Window>().ok())
    {
        window.set_transient_for(Some(&parent));
    }
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add(&container);
    let view = web_view(&container, None, Some(opener.clone()), found, popups).ok()?;
    let inner = view.webview();
    allow_popups(&inner);
    inner.connect_close({
        let window = window.clone();
        move |_| window.close()
    });
    window.show_all();
    popups.borrow_mut().push(view);
    Some(inner)
}

/// Closes a sign-in window from Zendo's side.
#[derive(Debug, Clone, Default)]
pub struct Handle(Arc<Mutex<Option<ChildStdin>>>);

impl Handle {
    pub fn close(&self) {
        if let Ok(mut stdin) = self.0.lock() {
            stdin.take();
        }
    }
}

/// Opens the sign-in window and waits for it: the code, or `None` when it
/// was closed first. It blocks; the window calls it on a thread.
///
/// # Errors
///
/// When the window cannot be started or fails, as without `WebKitGTK`.
pub fn open(store: Store, url: &str, handle: &Handle) -> anyhow::Result<Option<String>> {
    run(name_of(store), url, handle)
}

/// Opens Steam's sign-in and waits for it: the session, or `None` when the
/// window was closed first. It blocks; the window calls it on a thread.
///
/// # Errors
///
/// When the window cannot be started or fails, or writes no session.
pub fn open_steam(handle: &Handle) -> anyhow::Result<Option<Session>> {
    steam_session(run("steam", steam_web::SIGN_IN_PAGE, handle)?)
}

/// Asks Steam's page for a new token, in a window that never shows: the
/// session, or `None` when Steam's page no longer knows the sign-in.
///
/// # Errors
///
/// When the window cannot be started or fails.
pub fn renew_steam() -> anyhow::Result<Option<Session>> {
    steam_session(run(
        "steam-renew",
        steam_web::STORE_PAGE,
        &Handle::default(),
    )?)
}

fn steam_session(text: Option<String>) -> anyhow::Result<Option<Session>> {
    text.map(|text| {
        Session::parse(&text).ok_or_else(|| anyhow::anyhow!("Steam's page gave no sign-in"))
    })
    .transpose()
}

/// Runs `zendo sign-in-window <name> <url>` and reads what it writes.
fn run(name: &str, url: &str, handle: &Handle) -> anyhow::Result<Option<String>> {
    let exe = std::env::current_exe().context("could not find Zendo's own program")?;
    let mut command = Command::new(exe);
    command
        .args(["sign-in-window", name, url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if std::env::var_os(NO_DMABUF).is_none() {
        command.env(NO_DMABUF, "1");
    }
    let mut child = command
        .spawn()
        .context("could not open the sign-in window")?;
    if let Ok(mut stdin) = handle.0.lock() {
        *stdin = child.stdin.take();
    }
    let output = child
        .wait_with_output()
        .context("the sign-in window did not end cleanly")?;
    handle.close();
    let code = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    match output.status.code() {
        Some(0) if !code.is_empty() => Ok(Some(code)),
        Some(code) if code == i32::from(CLOSED) => Ok(None),
        _ => {
            // WebKit logs freely; Zendo's own line starts with its name.
            let stderr = String::from_utf8_lossy(&output.stderr);
            let reason = stderr
                .lines()
                .rev()
                .find_map(|line| line.strip_prefix("zendo: "))
                .map_or_else(|| output.status.to_string(), str::to_owned);
            bail!("the sign-in window failed: {reason}")
        }
    }
}

/// Steam's sign-in on its own store page.
mod steam {
    use std::time::Duration;

    use super::{
        Context, Rc, Read, RefCell, Session, Value, WebContext, WebViewBuilder,
        WebViewBuilderExtUnix,
    };
    use gtk::prelude::*;
    use zendo_core::steam_web;

    /// How long a renewal waits for Steam's page.
    const RENEW_WAIT: Duration = Duration::from_secs(45);

    /// Run on Steam's store pages: whether the user is signed in (not on
    /// the sign-in page) and, when so, the Steam ID and the Web API token
    /// the page carries in `#application_config` (as Playnite reads it).
    const SCRIPT: &str = r"
window.addEventListener('DOMContentLoaded', () => {
  if (location.hostname !== 'store.steampowered.com') {
    return;
  }
  // Steam sends a browser not signed in to its sign-in page.
  if (location.pathname.startsWith('/login')) {
    window.ipc.postMessage(JSON.stringify({ steam: null }));
    return;
  }
  const config = document.getElementById('application_config');
  if (!config) {
    return;
  }
  const read = (text) => { try { return JSON.parse(text || '{}'); } catch (e) { return {}; } };
  const user = read(config.dataset.userinfo);
  const store = read(config.dataset.store_user_config);
  const signedIn = user.logged_in === true && !!user.steamid && !!store.webapi_token;
  window.ipc.postMessage(JSON.stringify(signedIn
    ? { steam: { steamid: String(user.steamid), token: store.webapi_token } }
    : { steam: null }));
});
";

    /// Shows Steam's page until it gives a token: written as a session, or
    /// `None` when the window was closed or, renewing, Steam's page is not
    /// signed in.
    pub fn show(renew: bool, url: &str) -> anyhow::Result<Option<String>> {
        gtk::glib::set_prgname(Some(crate::app::APP_ID));
        gtk::glib::set_application_name("Zendo");
        gtk::init().context("could not start GTK")?;
        crate::i18n::set_from_settings();
        let profile = steam_web::profile_dir().context("no home folder for Steam's page")?;
        zendo_core::steam_web::make_profile_dir(&profile)?;
        let mut context = WebContext::new(Some(profile));

        // Renewing, the window is drawn off screen: never seen.
        let window: gtk::Window = if renew {
            gtk::OffscreenWindow::new().upcast()
        } else {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(&crate::i18n::t!("sign-in-window-title", store = "Steam"));
            window.set_default_size(560, 760);
            window
        };
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.add(&container);

        let session: Rc<RefCell<Option<String>>> = Rc::default();
        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(url)
            .with_initialization_script(SCRIPT)
            .with_ipc_handler({
                let session = Rc::clone(&session);
                move |request| {
                    let Ok(page) = serde_json::from_str::<Value>(request.body()) else {
                        return;
                    };
                    let found =
                        page.get("steam")
                            .filter(|steam| !steam.is_null())
                            .and_then(|steam| {
                                Some(Session {
                                    steamid: steam.get("steamid")?.as_str()?.to_owned(),
                                    token: steam.get("token")?.as_str()?.to_owned(),
                                })
                            });
                    match found {
                        Some(found) => {
                            *session.borrow_mut() = Some(found.to_text());
                            gtk::main_quit();
                        }
                        // Signed out: a renewal ends; a sign-in waits for
                        // the user.
                        None if renew => gtk::main_quit(),
                        None => {}
                    }
                }
            })
            .build_gtk(&container)
            .context("could not open Steam's page")?;

        window.connect_delete_event(|_, _| {
            gtk::main_quit();
            gtk::glib::Propagation::Proceed
        });
        if renew {
            gtk::glib::timeout_add_local_once(RENEW_WAIT, gtk::main_quit);
        } else {
            // Zendo's window holds the other end; it closes on Cancel or exit.
            std::thread::spawn(|| {
                let mut stdin = std::io::stdin();
                let mut sink = [0_u8; 64];
                while matches!(stdin.read(&mut sink), Ok(read) if read > 0) {}
                gtk::glib::idle_add_once(gtk::main_quit);
            });
        }
        window.show_all();
        gtk::main();
        drop(webview);
        let session = session.borrow_mut().take();
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_each_store_both_ways() {
        for store in Store::ALL {
            assert_eq!(store_named(name_of(store)), Some(store));
        }
        assert_eq!(store_named("steam"), None);
    }
}
