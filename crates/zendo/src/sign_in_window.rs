//! The sign-in window: a store's login page in a small window of Zendo's
//! own, so signing in needs nothing copied. It runs as a second process,
//! `zendo sign-in-window <store> <url>`, because the web view needs GTK's
//! own loop, apart from Iced's.
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
    NewWindowResponse, PageLoadEvent, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix,
};
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
    let Some(store) = store_named(store) else {
        eprintln!("zendo: no store called {store}");
        return ExitCode::from(2);
    };
    match show(store, url) {
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
    gtk::init().context("could not start GTK")?;
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title(&format!("Sign in to {} · Zendo", store.label()));
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
    window.set_title("Sign in · Zendo");
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
    let exe = std::env::current_exe().context("could not find Zendo's own program")?;
    let mut command = Command::new(exe);
    command
        .args(["sign-in-window", name_of(store), url])
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
