use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::{NtError, Result};

pub fn home_dir() -> Result<PathBuf> {
    select_home(env::var_os("HOME"), env::var_os("USERPROFILE")).ok_or(NtError::HomeNotFound)
}

pub fn nt_home() -> Result<PathBuf> {
    Ok(home_dir()?.join(".nt"))
}

fn select_home(home: Option<OsString>, user_profile: Option<OsString>) -> Option<PathBuf> {
    home.into_iter()
        .chain(user_profile)
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::select_home;

    #[test]
    fn selects_only_absolute_home_paths() {
        let absolute = std::env::current_dir().unwrap();
        assert_eq!(
            select_home(Some(absolute.clone().into_os_string()), None),
            Some(absolute.clone())
        );
        for value in ["", ".", "relative/home"] {
            assert_eq!(select_home(Some(OsString::from(value)), None), None);
        }
    }

    #[test]
    fn falls_back_to_a_valid_user_profile() {
        let absolute = std::env::current_dir().unwrap();
        for home in [
            None,
            Some(OsString::new()),
            Some(OsString::from("relative")),
        ] {
            assert_eq!(
                select_home(home, Some(absolute.clone().into_os_string())),
                Some(absolute.clone())
            );
        }
        assert_eq!(select_home(None, Some(OsString::new())), None);
    }
}
