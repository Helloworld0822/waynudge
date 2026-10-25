use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::TravelOption;
use crate::{
    error::{Result, invalid},
    routines::text,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FareProfile {
    #[default]
    Adult,
    Youth,
    Child,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FareQuotes {
    /// Caller-supplied provider or explicit synthetic fixture label; not verified by this server.
    pub source: String,
    pub adult: Option<u32>,
    pub youth: Option<u32>,
    pub child: Option<u32>,
}

impl TravelOption {
    pub(super) fn validate_fares(&self) -> Result<()> {
        if let Some(fares) = &self.fares_won {
            text(&fares.source)?;
            if [fares.adult, fares.youth, fares.child]
                .into_iter()
                .flatten()
                .any(|fare| fare > 1_000_000)
            {
                return Err(invalid("fare quotes must be 0..=1000000 won"));
            }
        }
        Ok(())
    }

    pub(super) fn fare(&self, profile: FareProfile) -> (Option<u32>, Option<String>) {
        let quoted = self.fares_won.as_ref().and_then(|fares| match profile {
            FareProfile::Adult => fares.adult,
            FareProfile::Youth => fares.youth,
            FareProfile::Child => fares.child,
        });
        if let Some(quoted) = quoted {
            return (
                Some(quoted),
                self.fares_won.as_ref().map(|fares| fares.source.clone()),
            );
        }
        match profile {
            FareProfile::Adult => (
                self.cost_won,
                self.cost_won.map(|_| "caller-supplied cost_won".into()),
            ),
            FareProfile::Youth | FareProfile::Child => (None, None),
        }
    }
}
