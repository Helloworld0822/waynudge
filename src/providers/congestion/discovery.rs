use crate::error::{Error, Result};
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) const CATALOG: &str = "https://infuser.odcloud.kr/oas/docs?namespace=15071311/v1";
#[derive(Deserialize)]
struct Catalog {
    host: String,
    #[serde(rename = "basePath")]
    base_path: String,
    schemes: Vec<String>,
    paths: BTreeMap<String, Path>,
}
#[derive(Deserialize)]
struct Path {
    get: Option<Operation>,
}
#[derive(Deserialize)]
struct Operation {
    summary: String,
}

pub(super) fn select(bytes: &[u8], today: NaiveDate) -> Result<(reqwest::Url, String)> {
    let catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| Error::Provider)?;
    if catalog.host != "api.odcloud.kr"
        || catalog.base_path != "/api"
        || !catalog.schemes.iter().any(|scheme| scheme == "https")
    {
        return Err(Error::Provider);
    }
    let pinned =
        NaiveDate::parse_from_str(super::DATASET_AS_OF, "%Y-%m-%d").map_err(|_| Error::Provider)?;
    let mut newest = None;
    for (path, operation) in catalog.paths {
        let Some(id) = path.strip_prefix("/15071311/v1/uddi:") else {
            continue;
        };
        if id.len() != 36
            || !id.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            })
        {
            continue;
        }
        let Some(operation) = operation.get else {
            continue;
        };
        let Some(date) = operation
            .summary
            .strip_prefix("서울교통공사_지하철혼잡도정보_")
        else {
            continue;
        };
        if date.len() != 8 || !date.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Ok(date) = NaiveDate::parse_from_str(date, "%Y%m%d") else {
            continue;
        };
        if date < pinned || date > today {
            continue;
        }
        if newest.as_ref().is_none_or(|(previous, _)| date > *previous) {
            newest = Some((date, path));
        }
    }
    let (date, path) = newest.ok_or(Error::Provider)?;
    let url = reqwest::Url::parse(&format!("https://api.odcloud.kr/api{path}"))
        .map_err(|_| Error::Provider)?;
    Ok((url, date.to_string()))
}

pub(super) fn enabled(value: Option<&str>) -> Result<bool> {
    match value {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(crate::error::invalid(
            "SUBWAY_CONGESTION_AUTO_DISCOVERY must be true or false",
        )),
    }
}

#[cfg(test)]
mod tests;
