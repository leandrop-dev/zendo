//! Zendo's own sign-ins to Epic, GOG and Amazon, through the engines Heroic
//! uses: legendary, gogdl and nile. Each is run as a separate program, from
//! `PATH` or from the copies Heroic bundles, and keeps its files in a folder
//! of Zendo's under `$XDG_DATA_HOME/zendo/stores/`, apart from Heroic's
//! sign-ins and readable by the user only.
//!
//! Signing in happens on the store's own page, which ends on a page or an
//! address holding a one-time code: in Zendo's sign-in window, which reads
//! it with [`code_at`], or in the user's browser, from which they paste it
//! back and [`code_from`] finds it in whatever was pasted.

use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::{Source, paths};

/// Where Heroic installs its bundled engines.
const HEROIC_ENGINES: [&str; 2] = [
    "/usr/lib/heroic-games-launcher/app.asar.unpacked/build/bin/x64/linux",
    "/opt/Heroic/resources/app.asar.unpacked/build/bin/x64/linux",
];

/// legendary's page for Epic: it ends on a JSON page with an
/// `authorizationCode`.
const EPIC_LOGIN: &str = "https://legendary.gl/epiclogin";
/// GOG's sign-in for its Galaxy client: it ends on
/// `embed.gog.com/on_login_success?origin=client&code=…`.
const GOG_LOGIN: &str = "https://auth.gog.com/auth?client_id=46899977096215655\
    &redirect_uri=https%3A%2F%2Fembed.gog.com%2Fon_login_success%3Forigin%3Dclient\
    &response_type=code&layout=galaxy";

/// A store Zendo signs in to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Store {
    Epic,
    Gog,
    Amazon,
}

impl Store {
    pub const ALL: [Self; 3] = [Self::Epic, Self::Gog, Self::Amazon];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Epic => "Epic Games",
            Self::Gog => "GOG",
            Self::Amazon => "Amazon Games",
        }
    }

    /// The program that talks to the store.
    #[must_use]
    pub fn engine_name(self) -> &'static str {
        match self {
            Self::Epic => "legendary",
            Self::Gog => "gogdl",
            Self::Amazon => "nile",
        }
    }

    #[must_use]
    pub fn source(self) -> Source {
        match self {
            Self::Epic => Source::Epic,
            Self::Gog => Source::Gog,
            Self::Amazon => Source::Amazon,
        }
    }

    /// The store as the library's ids and Zendo's folders name it.
    pub(crate) fn source_key(self) -> &'static str {
        match self {
            Self::Epic => "epic",
            Self::Gog => "gog",
            Self::Amazon => "amazon",
        }
    }

    /// Zendo's folder for this store's engine.
    #[must_use]
    pub fn dir(self) -> Option<PathBuf> {
        paths::data_home().map(|dir| dir.join("zendo/stores").join(self.source_key()))
    }

    /// The engine, from `PATH` first, then Heroic's bundled copy.
    #[must_use]
    pub fn engine(self) -> Option<PathBuf> {
        bundled(self.engine_name())
    }

    /// The page the user signs in on, for Epic and GOG; Amazon's is made
    /// for each sign-in by [`Store::start_amazon`].
    #[must_use]
    pub fn login_page(self) -> Option<&'static str> {
        match self {
            Self::Epic => Some(EPIC_LOGIN),
            Self::Gog => Some(GOG_LOGIN),
            Self::Amazon => None,
        }
    }

    /// The engine as a command, pointed at Zendo's folder, which is made
    /// on the way.
    pub(crate) fn command(self) -> anyhow::Result<Command> {
        let engine = self.engine().with_context(|| {
            format!(
                "{} not found: install it, or Heroic, which bundles it",
                self.engine_name()
            )
        })?;
        let dir = self
            .dir()
            .context("no home folder to keep the sign-in in")?;
        private_dir(&dir)?;
        let mut command = Command::new(engine);
        command.stdin(Stdio::null());
        match self {
            Self::Epic => {
                command.env("LEGENDARY_CONFIG_PATH", &dir);
            }
            // gogdl keeps its manifests in GOGDL_CONFIG_PATH/heroic_gogdl
            // and the tokens where it is told.
            Self::Gog => {
                command
                    .env("GOGDL_CONFIG_PATH", &dir)
                    .arg("--auth-config-path")
                    .arg(dir.join("auth.json"));
            }
            // nile keeps its files in NILE_CONFIG_PATH/nile.
            Self::Amazon => {
                command.env("NILE_CONFIG_PATH", &dir);
            }
        }
        Ok(command)
    }

    pub(crate) fn run(self, args: &[&str]) -> anyhow::Result<Output> {
        let mut command = self.command()?;
        command.args(args);
        command
            .output()
            .with_context(|| format!("could not run {}", self.engine_name()))
    }

    /// [`Store::run`], failing when the engine does.
    pub(crate) fn run_checked(self, args: &[&str]) -> anyhow::Result<Output> {
        let output = self.run(args)?;
        if !output.status.success() {
            bail!("{}", failure(self.engine_name(), &output));
        }
        Ok(output)
    }

    /// [`Store::run_checked`] with more in the engine's environment.
    pub(crate) fn run_checked_with(
        self,
        args: &[&str],
        env: &[(&str, &std::ffi::OsStr)],
    ) -> anyhow::Result<Output> {
        let mut command = self.command()?;
        command.args(args).envs(env.iter().copied());
        let output = command
            .output()
            .with_context(|| format!("could not run {}", self.engine_name()))?;
        if !output.status.success() {
            bail!("{}", failure(self.engine_name(), &output));
        }
        Ok(output)
    }

    /// Who is signed in: `Ok(None)` when no one is.
    ///
    /// # Errors
    ///
    /// When the engine is missing or fails, or GOG cannot be reached.
    pub fn account(self) -> anyhow::Result<Option<String>> {
        match self {
            Self::Epic => {
                #[derive(Deserialize)]
                struct User {
                    #[serde(rename = "displayName")]
                    display_name: Option<String>,
                }
                let Some(dir) = self.dir() else {
                    return Ok(None);
                };
                match fs::read_to_string(dir.join("user.json")) {
                    Ok(text) => {
                        let user: User =
                            serde_json::from_str(&text).context("legendary's user.json")?;
                        Ok(Some(
                            user.display_name.unwrap_or_else(|| "Signed in".to_owned()),
                        ))
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(err) => Err(err).context("could not read legendary's user.json"),
                }
            }
            Self::Gog => {
                let Some(credentials) = self.gog_token()? else {
                    return Ok(None);
                };
                Ok(Some(
                    gog_username(&credentials).unwrap_or_else(|_| "Signed in".to_owned()),
                ))
            }
            Self::Amazon => {
                #[derive(Deserialize)]
                struct Status {
                    #[serde(rename = "Username")]
                    username: Option<String>,
                    #[serde(rename = "LoggedIn")]
                    logged_in: bool,
                }
                let output = self.run(&["auth", "--status"])?;
                let status: Status = serde_json::from_slice(&output.stdout)
                    .with_context(|| failure("nile", &output))?;
                Ok(status
                    .logged_in
                    .then(|| status.username.unwrap_or_else(|| "Signed in".to_owned())))
            }
        }
    }

    /// Whether someone is signed in, from the engine's own files (nile's
    /// status for Amazon), without asking the store or naming the account.
    ///
    /// # Errors
    ///
    /// When nile is missing or fails.
    pub fn signed_in(self) -> anyhow::Result<bool> {
        let file = match self {
            Self::Epic => "user.json",
            Self::Gog => "auth.json",
            Self::Amazon => return Ok(self.account()?.is_some()),
        };
        Ok(self.dir().is_some_and(|dir| dir.join(file).is_file()))
    }

    /// gogdl's current tokens, refreshed when they ran out; `None` when no
    /// one is signed in.
    pub(crate) fn gog_token(self) -> anyhow::Result<Option<GogCredentials>> {
        if !self
            .dir()
            .is_some_and(|dir| dir.join("auth.json").is_file())
        {
            return Ok(None);
        }
        let output = self.run(&["auth"])?;
        let text = String::from_utf8_lossy(&output.stdout);
        match text.trim() {
            "" | "null" => Ok(None),
            text => serde_json::from_str(text)
                .map(Some)
                .with_context(|| failure("gogdl", &output)),
        }
    }

    /// Starts an Amazon sign-in: nile makes the page and what finishing
    /// needs. Nothing is signed in yet.
    ///
    /// # Errors
    ///
    /// When nile is missing or fails.
    pub fn start_amazon() -> anyhow::Result<AmazonLogin> {
        let output = Self::Amazon.run(&["auth", "--login", "--non-interactive"])?;
        serde_json::from_slice(&output.stdout).with_context(|| failure("nile", &output))
    }

    /// Finishes a sign-in with the code the store gave, and says who is
    /// signed in.
    ///
    /// # Errors
    ///
    /// When the code is turned down or the engine fails.
    pub fn sign_in(self, code: &str, amazon: Option<&AmazonLogin>) -> anyhow::Result<String> {
        let output = match self {
            Self::Epic => self.run(&["auth", "--code", code])?,
            Self::Gog => {
                let output = self.run(&["auth", "--code", code])?;
                let reply: serde_json::Value =
                    serde_json::from_slice(&output.stdout).unwrap_or(serde_json::Value::Null);
                if reply.is_null() || reply.get("error").is_some() {
                    bail!("GOG turned the code down; codes work once, so sign in again");
                }
                output
            }
            Self::Amazon => {
                let login = amazon.context("start the Amazon sign-in again")?;
                let output = self.run(&[
                    "register",
                    "--code",
                    code,
                    "--code-verifier",
                    &login.code_verifier,
                    "--serial",
                    &login.serial,
                    "--client-id",
                    &login.client_id,
                ])?;
                if !String::from_utf8_lossy(&output.stderr).contains("registered a device") {
                    bail!(
                        "Amazon turned the code down ({}); sign in again",
                        last_line(&output)
                    );
                }
                output
            }
        };
        if !output.status.success() {
            bail!("{}", failure(self.engine_name(), &output));
        }
        self.account()?
            .with_context(|| format!("{} did not sign in", self.engine_name()))
    }

    /// Signs out, removing the engine's tokens.
    ///
    /// # Errors
    ///
    /// When the engine fails or the tokens cannot be removed.
    pub fn sign_out(self) -> anyhow::Result<()> {
        match self {
            Self::Epic => {
                let output = self.run(&["auth", "--delete"])?;
                if !output.status.success() {
                    bail!("{}", failure("legendary", &output));
                }
            }
            Self::Gog => {
                if let Some(dir) = self.dir() {
                    match fs::remove_file(dir.join("auth.json")) {
                        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                            return Err(err).context("could not remove gogdl's tokens");
                        }
                        _ => {}
                    }
                }
                crate::gog_playtime::forget()?;
                crate::galaxy::forget()?;
            }
            Self::Amazon => {
                let output = self.run(&["auth", "--logout"])?;
                if !output.status.success() {
                    bail!("{}", failure("nile", &output));
                }
            }
        }
        Ok(())
    }
}

/// What nile needs to finish an Amazon sign-in. Kept in memory only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AmazonLogin {
    /// The page to sign in on.
    pub url: String,
    code_verifier: String,
    serial: String,
    client_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GogCredentials {
    pub access_token: String,
    pub user_id: String,
}

pub(crate) fn gog_username(credentials: &GogCredentials) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct User {
        username: String,
    }
    let agent = crate::http::agent(Duration::from_secs(15));
    let text = agent
        .get(format!(
            "https://users.gog.com/users/{}",
            credentials.user_id
        ))
        .header(
            "Authorization",
            format!("Bearer {}", credentials.access_token),
        )
        .call()?
        .into_body()
        .read_to_string()?;
    let user: User = serde_json::from_str(&text)?;
    Ok(user.username)
}

/// The one-time code in what the user pasted: the whole JSON page Epic
/// shows, the address GOG or Amazon ended on, or the code alone.
#[must_use]
pub fn code_from(store: Store, pasted: &str) -> Option<String> {
    let pasted = pasted.trim();
    if pasted.starts_with('{') {
        let json: serde_json::Value = serde_json::from_str(pasted).ok()?;
        return ["authorizationCode", "code"]
            .iter()
            .find_map(|key| json.get(key)?.as_str())
            .filter(|code| !code.is_empty())
            .map(str::to_owned);
    }
    if pasted.contains("://") || pasted.contains('?') {
        let name = match store {
            Store::Epic | Store::Gog => "code",
            Store::Amazon => "openid.oa2.authorization_code",
        };
        return query_param(pasted, name);
    }
    let code = pasted.trim_matches('"');
    (!code.is_empty() && !code.contains(char::is_whitespace)).then(|| code.to_owned())
}

/// The code on the page a store's sign-in ends on, seen from a sign-in
/// window: GOG's `embed.gog.com/on_login_success?code=…`, Amazon's
/// address with `openid.oa2.authorization_code`, and the JSON page at
/// Epic's `/id/api/redirect`, whose text is `text`. Any other page gives
/// `None`, however much it looks like a code.
#[must_use]
pub fn code_at(store: Store, address: &str, text: Option<&str>) -> Option<String> {
    match store {
        Store::Gog if address.starts_with("https://embed.gog.com/on_login_success") => {
            query_param(address, "code")
        }
        Store::Amazon if address.starts_with("https://www.amazon.") => {
            query_param(address, "openid.oa2.authorization_code")
        }
        Store::Epic if address.starts_with("https://www.epicgames.com/id/api/redirect") => {
            let json: serde_json::Value = serde_json::from_str(text?.trim()).ok()?;
            json.get("authorizationCode")?
                .as_str()
                .filter(|code| !code.is_empty())
                .map(str::to_owned)
        }
        _ => None,
    }
}

/// A parameter of a URL's query, decoded.
fn query_param(url: &str, name: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    let query = query.split('#').next().unwrap_or(query);
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| crate::http::percent_decode(key) == name)
        .map(|(_, value)| crate::http::percent_decode(value))
        .filter(|value| !value.is_empty())
}

/// A folder only its owner can enter.
fn private_dir(dir: &Path) -> anyhow::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("could not make {}", dir.display()))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("could not protect {}", dir.display()))
}

/// A program Heroic bundles beside its engines (comet as well), from
/// `PATH` first.
pub(crate) fn bundled(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(HEROIC_ENGINES.iter().map(PathBuf::from))
        .map(|dir| dir.join(name))
        .find(|file| is_executable(file))
}

/// A Windows program Heroic bundles for its prefixes, beside its engines.
pub(crate) fn bundled_windows(name: &str) -> Option<PathBuf> {
    HEROIC_ENGINES
        .iter()
        .map(|dir| Path::new(dir).with_file_name("win32").join(name))
        .find(|file| file.is_file())
}

fn is_executable(file: &Path) -> bool {
    file.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The last thing an engine said, for an error message.
fn last_line(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    stderr
        .lines()
        .chain(stdout.lines())
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(String::new, |line| line.chars().take(200).collect())
}

fn failure(engine: &str, output: &Output) -> String {
    match last_line(output) {
        line if line.is_empty() => format!("{engine} failed ({})", output.status),
        line => format!("{engine}: {line}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_code_only_from_the_page_a_sign_in_ends_on() {
        let gog = "https://embed.gog.com/on_login_success?origin=client&code=g%2B1";
        assert_eq!(code_at(Store::Gog, gog, None).as_deref(), Some("g+1"));
        assert_eq!(
            code_at(Store::Gog, "https://login.gog.com/?code=nope", None),
            None
        );
        let amazon = "https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher&openid.oa2.authorization_code=ANabc";
        assert_eq!(
            code_at(Store::Amazon, amazon, None).as_deref(),
            Some("ANabc")
        );
        assert_eq!(
            code_at(
                Store::Amazon,
                "https://evil.example/?openid.oa2.authorization_code=x",
                None
            ),
            None
        );
        let epic = "https://www.epicgames.com/id/api/redirect?clientId=34a0&responseType=code";
        let page = r#"{"warning":"Do not share this code","redirectUrl":"https://localhost/launcher/authorized?code=zzz","authorizationCode":"abc123","sid":null}"#;
        assert_eq!(
            code_at(Store::Epic, epic, Some(page)).as_deref(),
            Some("abc123")
        );
        assert_eq!(code_at(Store::Epic, epic, None), None);
        assert_eq!(
            code_at(
                Store::Epic,
                "https://www.epicgames.com/id/login",
                Some(page)
            ),
            None
        );
    }

    #[test]
    fn finds_the_code_in_what_was_pasted() {
        let epic = r#"{"warning":"x","redirectUrl":"https://localhost/launcher/authorized?code=zzz","authorizationCode":"abc123","exchangeCode":null,"sid":null}"#;
        assert_eq!(code_from(Store::Epic, epic).as_deref(), Some("abc123"));
        assert_eq!(
            code_from(Store::Epic, " \"abc123\" ").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            code_from(
                Store::Gog,
                "https://embed.gog.com/on_login_success?origin=client&code=G-o_G%2B1"
            )
            .as_deref(),
            Some("G-o_G+1")
        );
        assert_eq!(
            code_from(
                Store::Amazon,
                "https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher\
                 &openid.oa2.authorization_code=ANxyz&openid.mode=id_res"
            )
            .as_deref(),
            Some("ANxyz")
        );
        assert_eq!(
            code_from(Store::Gog, "https://embed.gog.com/?origin=client"),
            None
        );
        assert_eq!(code_from(Store::Epic, "two words"), None);
        assert_eq!(code_from(Store::Epic, ""), None);
    }

    #[test]
    fn decodes_queries() {
        assert_eq!(
            query_param("x://h/p?a=1&b=two%26#frag", "b").as_deref(),
            Some("two&")
        );
    }

    #[test]
    fn reads_nile_login_data() {
        let login: AmazonLogin = serde_json::from_str(
            r#"{"client_id":"c","code_verifier":"v","serial":"s","url":"https://amazon.com/ap/signin?x"}"#,
        )
        .unwrap();
        assert_eq!(login.url, "https://amazon.com/ap/signin?x");
    }
}
