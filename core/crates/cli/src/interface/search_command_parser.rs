use std::path::PathBuf;

use crate::*;

pub(crate) fn parse_search_options(args: &[String]) -> Result<SearchOptions, CliError> {
    let query = args
        .first()
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| CliError::Usage("A search query is required.".to_string()))?;
    let mut options = SearchOptions {
        query,
        engine: SearchEngine::Google,
        budget: DEFAULT_SEARCH_TOKENS,
        headed: false,
        profile_dir: None,
        session_file: None,
    };
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--engine" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--engine requires a value.".to_string()))?;
                options.engine = parse_search_engine(value)?;
                index += 2;
            }
            "--headed" => {
                options.headed = true;
                index += 1;
            }
            "--headless" => {
                options.headed = false;
                index += 1;
            }
            "--budget" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--budget requires a positive integer.".to_string())
                })?;
                options.budget = value.parse().map_err(|_| {
                    CliError::Usage("--budget requires a positive integer.".to_string())
                })?;
                if options.budget == 0 {
                    return Err(CliError::Usage(
                        "--budget requires a positive integer.".to_string(),
                    ));
                }
                index += 2;
            }
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                options.session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--profile-dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--profile-dir requires a path.".to_string()))?;
                options.profile_dir = Some(PathBuf::from(value));
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for search command."
                )));
            }
        }
    }

    Ok(options)
}

pub(crate) fn parse_search_open_result_options(
    args: &[String],
) -> Result<SearchOpenResultOptions, CliError> {
    let mut session_file = None;
    let mut engine = SearchEngine::Google;
    let mut rank = None;
    let mut headed = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--engine" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--engine requires a value.".to_string()))?;
                engine = parse_search_engine(value)?;
                index += 2;
            }
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--rank" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--rank requires a number.".to_string()))?;
                let parsed = value.parse::<usize>().map_err(|_| {
                    CliError::Usage("--rank requires a positive number.".to_string())
                })?;
                if parsed == 0 {
                    return Err(CliError::Usage(
                        "--rank requires a positive number.".to_string(),
                    ));
                }
                rank = Some(parsed);
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            "--headless" => {
                headed = false;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for search-open-result."
                )));
            }
        }
    }

    Ok(SearchOpenResultOptions {
        engine,
        session_file,
        rank: rank.ok_or_else(|| {
            CliError::Usage("search-open-result requires `--rank <number>`.".to_string())
        })?,
        headed,
    })
}

pub(crate) fn parse_search_open_top_options(
    args: &[String],
) -> Result<SearchOpenTopOptions, CliError> {
    let mut session_file = None;
    let mut engine = SearchEngine::Google;
    let mut limit = 3usize;
    let mut headed = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--engine" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--engine requires a value.".to_string()))?;
                engine = parse_search_engine(value)?;
                index += 2;
            }
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--limit requires a number.".to_string()))?;
                limit = value.parse::<usize>().map_err(|_| {
                    CliError::Usage("--limit requires a positive number.".to_string())
                })?;
                if limit == 0 {
                    return Err(CliError::Usage(
                        "--limit requires a positive number.".to_string(),
                    ));
                }
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            "--headless" => {
                headed = false;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for search-open-top."
                )));
            }
        }
    }

    Ok(SearchOpenTopOptions {
        engine,
        session_file,
        limit,
        headed,
    })
}

pub(crate) fn parse_search_engine(value: &str) -> Result<SearchEngine, CliError> {
    match value {
        "google" => Ok(SearchEngine::Google),
        "brave" => Ok(SearchEngine::Brave),
        other => Err(CliError::Usage(format!(
            "Unknown search engine `{other}`. Use `google` or `brave`."
        ))),
    }
}
