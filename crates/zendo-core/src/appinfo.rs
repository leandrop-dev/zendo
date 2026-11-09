//! Steam's app cache, `appcache/appinfo.vdf`: what the Steam client knows
//! of every app it has seen (the account's games among them, installed or
//! not), in Valve's binary key-values. Zendo reads each game's size from
//! it: the depots Steam would install, their sizes on disk and to
//! download.
//!
//! The file starts with a magic number naming its version. Version 29
//! keeps keys as numbers into a table of strings at an offset the header
//! gives; version 28 writes them out. Each app is its id, its length, 60
//! bytes Zendo does not need (state, dates, hashes), and its key-values.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::BuildHasher;
use std::path::PathBuf;

use anyhow::{Context, bail};

const V28: u32 = 0x0756_4428;
const V29: u32 = 0x0756_4429;
/// An app's fields before its key-values.
const APP_HEADER: usize = 60;

/// An app's size, as its depots add up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizes {
    /// On disk once installed.
    pub install: u64,
    pub download: u64,
}

/// The sizes of these apps the cache knows, from the first Steam folder
/// that has the file; none when there is none or it cannot be read.
#[must_use]
pub fn sizes<S: BuildHasher>(roots: &[PathBuf], apps: &HashSet<u64, S>) -> HashMap<u64, Sizes> {
    roots
        .iter()
        .find_map(|root| fs::read(root.join("appcache/appinfo.vdf")).ok())
        .and_then(|data| read(&data, apps).ok())
        .unwrap_or_default()
}

/// The apps wanted, by their sizes; the others are skipped unread.
fn read<S: BuildHasher>(
    data: &[u8],
    apps: &HashSet<u64, S>,
) -> anyhow::Result<HashMap<u64, Sizes>> {
    let mut at = Reader { data, at: 0 };
    let magic = at.u32()?;
    let _universe = at.u32()?;
    let strings = match magic {
        V29 => {
            let offset = usize::try_from(at.u64()?).context("string table too far")?;
            Some(string_table(data, offset)?)
        }
        V28 => None,
        other => bail!("appinfo.vdf of an unknown version ({other:#x})"),
    };
    let mut found = HashMap::new();
    loop {
        let app = u64::from(at.u32()?);
        if app == 0 {
            break;
        }
        let length = at.u32()? as usize;
        let end = at
            .at
            .checked_add(length)
            .context("an app runs past the end")?;
        if end > data.len() {
            bail!("an app runs past the end");
        }
        if apps.contains(&app) {
            let mut body = Reader {
                data: &data[..end],
                at: at.at + APP_HEADER,
            };
            if let Ok(tree) = body.map(strings.as_deref())
                && let Some(sizes) = app_sizes(&tree)
            {
                found.insert(app, sizes);
            }
        }
        at.at = end;
    }
    Ok(found)
}

fn string_table(data: &[u8], offset: usize) -> anyhow::Result<Vec<String>> {
    let mut at = Reader { data, at: offset };
    let count = at.u32()?;
    (0..count).map(|_| at.text()).collect()
}

/// A key-value: a map of more, text, or a number.
#[derive(Debug, PartialEq)]
enum Value {
    Map(Vec<(String, Value)>),
    Text(String),
    Number(u64),
    Other,
}

impl Value {
    fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(entries) => entries
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// A number, written as one or as text.
    fn number(&self) -> Option<u64> {
        match self {
            Self::Number(number) => Some(*number),
            Self::Text(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    fn entries(&self) -> &[(String, Self)] {
        match self {
            Self::Map(entries) => entries,
            _ => &[],
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, count: usize) -> anyhow::Result<&[u8]> {
        let end = self.at.checked_add(count).context("past the end")?;
        let bytes = self.data.get(self.at..end).context("the file ends early")?;
        self.at = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> anyhow::Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u32(&mut self) -> anyhow::Result<u32> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> anyhow::Result<u64> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.bytes(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    /// Text up to its NUL.
    fn text(&mut self) -> anyhow::Result<String> {
        let rest = self.data.get(self.at..).context("the file ends early")?;
        let length = rest
            .iter()
            .position(|&byte| byte == 0)
            .context("text without its end")?;
        let text = String::from_utf8_lossy(&rest[..length]).into_owned();
        self.at += length + 1;
        Ok(text)
    }

    /// A map's entries, up to its end mark.
    fn map(&mut self, strings: Option<&[String]>) -> anyhow::Result<Value> {
        let mut entries = Vec::new();
        loop {
            let kind = self.u8()?;
            if kind == 8 || kind == 11 {
                return Ok(Value::Map(entries));
            }
            let key = match strings {
                Some(strings) => {
                    let index = self.u32()? as usize;
                    strings
                        .get(index)
                        .context("a key not in the table")?
                        .clone()
                }
                None => self.text()?,
            };
            let value = match kind {
                0 => self.map(strings)?,
                1 => Value::Text(self.text()?),
                // A 32-bit number, a pointer, a colour.
                2 | 4 | 6 => Value::Number(u64::from(self.u32()?)),
                3 => {
                    self.u32()?;
                    Value::Other
                }
                7 | 10 => Value::Number(self.u64()?),
                other => bail!("a value of an unknown kind ({other})"),
            };
            entries.push((key, value));
        }
    }
}

/// The app's size: its own depots for one system, in English, the 64-bit
/// ones where both are; not other apps' (shared redistributables), not
/// optional or DLC content, not the low-violence cut. The system is Linux
/// when the app has a released Linux depot, as Steam installs the native
/// version then, else Windows. `None` when no depot says its size.
fn app_sizes(tree: &Value) -> Option<Sizes> {
    /// A depot that may be installed: its systems, its arch, its size.
    struct Depot<'a> {
        systems: &'a str,
        arch: Option<u64>,
        sizes: Sizes,
    }

    let app = tree.get("appinfo").unwrap_or(tree);
    let depots: Vec<Depot<'_>> = app
        .get("depots")?
        .entries()
        .iter()
        .filter(|(id, _)| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|(_, depot)| {
            let config = depot.get("config").unwrap_or(&Value::Other);
            let language = config.get("language").and_then(Value::text).unwrap_or("");
            let skip = depot.get("depotfromapp").is_some()
                || depot.get("sharedinstall").is_some()
                || depot.get("dlcappid").is_some()
                || config.get("optionaldlc").is_some()
                || config.get("lowviolence").is_some()
                || !(language.is_empty() || language == "english");
            if skip {
                return None;
            }
            // Unreleased depots have no public build.
            let public = depot.get("manifests")?.get("public")?;
            let install = public.get("size").and_then(Value::number)?;
            // Old apps give 0 to download: unknown.
            let download = public
                .get("download")
                .and_then(Value::number)
                .filter(|bytes| *bytes > 0)
                .unwrap_or(install);
            Some(Depot {
                systems: config.get("oslist").and_then(Value::text).unwrap_or(""),
                arch: config.get("osarch").and_then(Value::number),
                sizes: Sizes { install, download },
            })
        })
        .collect();
    let system = if depots.iter().any(|depot| depot.systems.contains("linux")) {
        "linux"
    } else {
        "windows"
    };
    let for_system: Vec<&Depot<'_>> = depots
        .iter()
        .filter(|depot| depot.systems.is_empty() || depot.systems.contains(system))
        .collect();
    let has_64 = for_system.iter().any(|depot| depot.arch == Some(64));
    let chosen: Vec<&Depot<'_>> = for_system
        .into_iter()
        .filter(|depot| !(has_64 && depot.arch == Some(32)))
        .collect();
    (!chosen.is_empty()).then(|| Sizes {
        install: chosen.iter().map(|depot| depot.sizes.install).sum(),
        download: chosen.iter().map(|depot| depot.sizes.download).sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes key-values as version 29 does, keys into `strings`.
    struct Writer {
        out: Vec<u8>,
        strings: Vec<String>,
    }

    enum Kv<'a> {
        Map(&'a str, Vec<Kv<'a>>),
        Text(&'a str, &'a str),
        Int(&'a str, u32),
        Long(&'a str, u64),
    }

    impl Writer {
        fn key(&mut self, key: &str) {
            let index = self
                .strings
                .iter()
                .position(|known| known == key)
                .unwrap_or_else(|| {
                    self.strings.push(key.to_owned());
                    self.strings.len() - 1
                });
            self.out
                .extend_from_slice(&u32::try_from(index).unwrap().to_le_bytes());
        }

        fn write(&mut self, kv: &Kv<'_>) {
            match kv {
                Kv::Map(key, entries) => {
                    self.out.push(0);
                    self.key(key);
                    for entry in entries {
                        self.write(entry);
                    }
                    self.out.push(8);
                }
                Kv::Text(key, text) => {
                    self.out.push(1);
                    self.key(key);
                    self.out.extend_from_slice(text.as_bytes());
                    self.out.push(0);
                }
                Kv::Int(key, number) => {
                    self.out.push(2);
                    self.key(key);
                    self.out.extend_from_slice(&number.to_le_bytes());
                }
                Kv::Long(key, number) => {
                    self.out.push(7);
                    self.key(key);
                    self.out.extend_from_slice(&number.to_le_bytes());
                }
            }
        }
    }

    /// A version 29 file with these apps.
    fn file(apps: &[(u32, Kv<'_>)]) -> Vec<u8> {
        let mut writer = Writer {
            out: Vec::new(),
            strings: Vec::new(),
        };
        let mut body = Vec::new();
        for (id, tree) in apps {
            writer.out.clear();
            writer.write(tree);
            writer.out.push(8);
            body.extend_from_slice(&id.to_le_bytes());
            let length = u32::try_from(APP_HEADER + writer.out.len()).unwrap();
            body.extend_from_slice(&length.to_le_bytes());
            body.extend_from_slice(&[0; APP_HEADER]);
            body.extend_from_slice(&writer.out);
        }
        body.extend_from_slice(&0_u32.to_le_bytes());
        let mut data = Vec::new();
        data.extend_from_slice(&V29.to_le_bytes());
        data.extend_from_slice(&1_u32.to_le_bytes());
        let offset = 16 + body.len() as u64;
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&body);
        data.extend_from_slice(&u32::try_from(writer.strings.len()).unwrap().to_le_bytes());
        for string in &writer.strings {
            data.extend_from_slice(string.as_bytes());
            data.push(0);
        }
        data
    }

    fn depot<'a>(id: &'a str, config: Vec<Kv<'a>>, size: &'a str, download: u64) -> Kv<'a> {
        Kv::Map(
            id,
            vec![
                Kv::Map("config", config),
                Kv::Map(
                    "manifests",
                    vec![Kv::Map(
                        "public",
                        vec![Kv::Text("size", size), Kv::Long("download", download)],
                    )],
                ),
            ],
        )
    }

    #[test]
    fn adds_up_the_depots_steam_would_install() {
        let game = Kv::Map(
            "appinfo",
            vec![
                Kv::Int("appid", 4_704_690),
                Kv::Map(
                    "depots",
                    vec![
                        depot("4704691", vec![Kv::Text("oslist", "windows")], "3000", 2000),
                        depot("4704692", vec![], "500", 400),
                        depot(
                            "4704693",
                            vec![
                                Kv::Text("oslist", "windows"),
                                Kv::Text("language", "german"),
                            ],
                            "900",
                            900,
                        ),
                        depot("4704694", vec![Kv::Text("oslist", "macos")], "800", 800),
                        depot("4704695", vec![Kv::Int("optionaldlc", 1)], "700", 700),
                        Kv::Map(
                            "228989",
                            vec![
                                Kv::Int("depotfromapp", 228_980),
                                Kv::Int("sharedinstall", 1),
                            ],
                        ),
                        Kv::Text("branches", "x"),
                    ],
                ),
            ],
        );
        let native = Kv::Map(
            "appinfo",
            vec![Kv::Map(
                "depots",
                vec![
                    depot("11", vec![Kv::Text("oslist", "windows")], "100", 90),
                    depot("12", vec![Kv::Text("oslist", "linux")], "70", 60),
                ],
            )],
        );
        // A Linux depot not released, 32 and 64-bit builds (the arch as a
        // number), and 0 to download: an old app's unknown.
        let windows_only = Kv::Map(
            "appinfo",
            vec![Kv::Map(
                "depots",
                vec![
                    depot(
                        "31",
                        vec![Kv::Text("oslist", "windows"), Kv::Int("osarch", 64)],
                        "400",
                        0,
                    ),
                    depot(
                        "32",
                        vec![Kv::Text("oslist", "windows"), Kv::Int("osarch", 32)],
                        "300",
                        200,
                    ),
                    Kv::Map(
                        "33",
                        vec![Kv::Map("config", vec![Kv::Text("oslist", "linux")])],
                    ),
                ],
            )],
        );
        let unwanted = Kv::Map("appinfo", vec![]);
        let data = file(&[
            (4_704_690, game),
            (10, native),
            (30, windows_only),
            (20, unwanted),
        ]);
        let found = read(&data, &HashSet::from([4_704_690, 10, 30, 20])).unwrap();
        assert_eq!(
            found[&4_704_690],
            Sizes {
                install: 3500,
                download: 2400
            }
        );
        // A native game: its Linux depots.
        assert_eq!(
            found[&10],
            Sizes {
                install: 70,
                download: 60
            }
        );
        assert_eq!(
            found[&30],
            Sizes {
                install: 400,
                download: 400
            }
        );
        assert!(!found.contains_key(&20), "no depots, no size");
        // Apps not asked for are skipped.
        assert_eq!(read(&data, &HashSet::from([10])).unwrap().len(), 1);
    }

    #[test]
    fn refuses_a_cut_short_or_unknown_file() {
        let data = file(&[(10, Kv::Map("appinfo", vec![]))]);
        assert!(read(&data[..data.len() / 2], &HashSet::from([10])).is_err());
        assert!(read(&[1, 2, 3, 4, 0, 0, 0, 0], &HashSet::new()).is_err());
    }

    /// Reads the user's real cache: `cargo test -p zendo-core appinfo -- --ignored`.
    #[test]
    #[ignore = "reads this computer's Steam cache"]
    fn reads_this_computers_cache() {
        let found = sizes(&crate::steam::roots(), &HashSet::from([620, 4_704_690]));
        assert!(!found.is_empty(), "{found:?}");
        for (app, sizes) in found {
            eprintln!("{app}: {sizes:?}");
        }
        let list = crate::owned::cached(crate::owned::Shop::Steam);
        let sized = list.iter().filter(|game| game.sizes.is_some()).count();
        eprintln!("{sized} of {} in the Store's Steam list", list.len());
    }
}
