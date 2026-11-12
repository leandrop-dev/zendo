//! The keys of one group of an INI-style file, as `.desktop` files and
//! KDE's `kdeglobals` write them.

/// The keys of one `[group]`, without the translated ones (`Name[pt]`).
pub(crate) struct Group<'a> {
    pairs: Vec<(&'a str, &'a str)>,
}

impl<'a> Group<'a> {
    /// The keys of the group whose header is `header`, brackets included.
    pub(crate) fn parse(text: &'a str, header: &str) -> Self {
        let mut pairs = Vec::new();
        let mut inside = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                inside = line == header;
            } else if inside
                && !line.starts_with('#')
                && let Some((key, value)) = line.split_once('=')
                && !key.contains('[')
            {
                pairs.push((key.trim(), value.trim()));
            }
        }
        Self { pairs }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&'a str> {
        self.pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_one_group() {
        let text = "[A]\nx=1\n# c=2\nName[pt]=z\n\n[B]\nx = 3\n";
        assert_eq!(Group::parse(text, "[A]").get("x"), Some("1"));
        assert_eq!(Group::parse(text, "[A]").get("c"), None);
        assert_eq!(Group::parse(text, "[A]").get("Name"), None);
        assert_eq!(Group::parse(text, "[B]").get("x"), Some("3"));
        assert_eq!(Group::parse(text, "[C]").get("x"), None);
    }
}
