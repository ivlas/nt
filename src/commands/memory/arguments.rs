use crate::error::{NtError, Result};
use crate::memory::SummaryNodeId;

pub(super) enum PendingRequest {
    List(Option<i64>),
    Inspect(SummaryNodeId),
}

pub(super) fn parse_pending(arguments: &[String]) -> Result<PendingRequest> {
    match arguments {
        [] => Ok(PendingRequest::List(None)),
        [argument] if argument.starts_with("limit:") => {
            let limit = parse_positive(
                argument.strip_prefix("limit:").unwrap_or_default(),
                "memory pending limit",
            )?;
            Ok(PendingRequest::List(Some(limit)))
        }
        [node] => Ok(PendingRequest::Inspect(node.parse()?)),
        _ => Err(NtError::InvalidValue {
            field: "memory pending arguments",
            value: arguments.join(" "),
        }),
    }
}

pub(super) fn parse_positive(value: &str, field: &'static str) -> Result<i64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid_positive(field, value);
    }
    let parsed = value.parse::<i64>().map_err(|_| NtError::InvalidValue {
        field,
        value: value.to_string(),
    })?;
    if parsed == 0 {
        return invalid_positive(field, value);
    }
    Ok(parsed)
}

fn invalid_positive<T>(field: &'static str, value: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field,
        value: value.to_string(),
    })
}
