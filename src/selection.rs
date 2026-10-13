use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, str::FromStr};

/// Inclusive ranges, expanded only after checking the available item count.
#[derive(Clone, Debug)]
pub struct Selection(Vec<(usize, usize)>);

impl Selection {
    fn parse_with(text: &str, parse: impl Fn(&str) -> Result<usize>) -> Result<Self> {
        ensure!(
            !text.chars().any(char::is_control),
            "Selections must be a single line without control characters"
        );
        let ranges = text
            .split(',')
            .map(|part| {
                let part = part.trim();
                let (start, end) = match part.split_once('-') {
                    Some((start, end)) => (parse(start.trim())?, parse(end.trim())?),
                    None => {
                        let number = parse(part)?;
                        (number, number)
                    }
                };
                ensure!(start > 0, "Item and result numbers start at 1");
                ensure!(start <= end, "Selection ranges must be ascending: {part}");
                Ok((start, end))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self(ranges))
    }

    pub fn letters(text: &str) -> Result<Self> {
        Self::parse_with(text, crate::listing::letter_number)
    }

    /// Status commands reserve numeric selection syntax instead of adding it as a title.
    pub fn looks_numeric(text: &str) -> bool {
        text.bytes().any(|byte| byte.is_ascii_digit())
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b',' | b'-' | b' '))
    }

    pub fn resolve(&self, available: usize) -> Result<Vec<usize>> {
        for &(_, end) in &self.0 {
            ensure!(
                end <= available,
                "No item or result {end}; only {available} entries are available. Rerun the search or listing to see current numbers"
            );
        }
        let mut numbers = BTreeSet::new();
        for &(start, end) in &self.0 {
            numbers.extend(start..=end);
        }
        Ok(numbers.into_iter().collect())
    }
}

impl FromStr for Selection {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        Self::parse_with(text, |value| {
            ensure!(
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
                "Use item numbers, inclusive ranges, or comma-separated selections (for example 1-3,5)"
            );
            value.parse().context("Item number is too large")
        })
    }
}
