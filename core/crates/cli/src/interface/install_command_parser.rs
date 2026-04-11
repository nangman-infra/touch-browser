use super::deps::{CliError, UninstallOptions, UpdateOptions};

pub(crate) fn parse_update_options(args: &[String]) -> Result<UpdateOptions, CliError> {
    let mut options = UpdateOptions {
        check: false,
        version: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--check" => {
                options.check = true;
                index += 1;
            }
            "--version" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--version requires a value.".to_string()))?;
                if value.trim().is_empty() {
                    return Err(CliError::Usage(
                        "--version requires a non-empty value.".to_string(),
                    ));
                }
                options.version = Some(value.clone());
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for update command."
                )));
            }
        }
    }

    Ok(options)
}

pub(crate) fn parse_uninstall_options(args: &[String]) -> Result<UninstallOptions, CliError> {
    let mut options = UninstallOptions {
        purge_data: false,
        purge_all: false,
        yes: false,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--purge-data" => {
                options.purge_data = true;
                index += 1;
            }
            "--purge-all" => {
                options.purge_all = true;
                options.purge_data = true;
                index += 1;
            }
            "--yes" => {
                options.yes = true;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for uninstall command."
                )));
            }
        }
    }

    Ok(options)
}
