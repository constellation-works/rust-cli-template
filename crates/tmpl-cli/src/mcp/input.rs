//! How a command's arguments are read as a tool's input: the same `Args`
//! struct clap parses also derives `Deserialize` and `JsonSchema`, and a
//! field whose type parses from text (a title, a tag) reads its JSON string
//! through that type's `FromStr`, so a call is checked by the rules the
//! command line uses and the schema advertises no limit the parse does not
//! hold (STD-01 R25, R36).

use serde::{Deserialize, Deserializer};
use std::fmt::Display;
use std::str::FromStr;

/// A required field: a JSON string parsed by `T`'s `FromStr`.
pub(crate) fn parsed<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    let text = String::deserialize(deserializer)?;
    text.parse().map_err(serde::de::Error::custom)
}

/// An optional field: absent or `null` is `None`, else as [`parsed`].
pub(crate) fn parsed_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    Option::<String>::deserialize(deserializer)?
        .map(|text| text.parse().map_err(serde::de::Error::custom))
        .transpose()
}

/// A repeated field: a JSON array of strings, each as [`parsed`].
pub(crate) fn parsed_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|text| text.parse().map_err(serde::de::Error::custom))
        .collect()
}
