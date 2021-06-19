use std::str::FromStr;

use serde::{Deserialize, Deserializer};
use smol_str::SmolStr;

use crate::{ElementId, Key};

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = SmolStr::deserialize(deserializer)?;
        if let Ok(eid) = ElementId::from_str(&s) {
            Ok(Key::Seq(eid))
        } else {
            Ok(Key::Map(s))
        }
    }
}
