use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    ShowVersion,
    ShowHelp,
    Run { args: Vec<OsString> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse_args<I, S>(args: I) -> Result<Action, ParseError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let mut passthrough = Vec::new();
    let mut saw_help = false;
    let mut saw_version = false;

    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        match text.as_ref() {
            "-h" | "--help" => saw_help = true,
            "-v" | "--version" => saw_version = true,
            "--" => {
                passthrough.push(arg);
                passthrough.extend(args);
                break;
            }
            value if value.starts_with('-') && value != "-" => {
                return Err(ParseError {
                    message: format!("unknown option: {value}"),
                });
            }
            _ => passthrough.push(arg),
        }
    }

    if saw_help {
        return Ok(Action::ShowHelp);
    }
    if saw_version {
        return Ok(Action::ShowVersion);
    }
    Ok(Action::Run { args: passthrough })
}

pub fn help_text(program: &str) -> String {
    format!(
        "Usage: {program} [OPTION] [URL...]\n\n\
Open web addresses in Bitter Browser.\n\n\
Options:\n\
  -h, --help     Show this help and exit\n\
  -v, --version  Show the version and exit\n\n\
Profile selection is not implemented; bitter-cli uses the same profile as\n\
bitter-ui.\n"
    )
}

pub fn version_text() -> String {
    format!("Bitter Browser {VERSION}")
}

pub fn browser_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("BITTER_BROWSER_BIN") {
        return PathBuf::from(path);
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(dir) = current.parent()
    {
        let sibling = dir.join("bitter-ui");
        if sibling.exists() {
            return sibling;
        }
    }
    PathBuf::from("bitter-ui")
}

pub fn run<I, S>(args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let program = args
        .next()
        .unwrap_or_else(|| OsString::from("bitter-cli"))
        .to_string_lossy()
        .into_owned();
    let action = match parse_args(std::iter::once(OsString::from(&program)).chain(args)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("Try '{program} --help' for more information.");
            return 2;
        }
    };

    match action {
        Action::ShowHelp => {
            print!("{}", help_text(&program));
            0
        }
        Action::ShowVersion => {
            println!("{}", version_text());
            0
        }
        Action::Run { args } => {
            let binary = browser_binary();
            match Command::new(&binary).args(&args).status() {
                Ok(status) => status.code().unwrap_or(1),
                Err(error) => {
                    eprintln!("Could not launch {}: {error}", binary.display());
                    1
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn parses_help_and_version() {
        assert_eq!(
            parse_args(["bitter-cli", "--help"]).unwrap(),
            Action::ShowHelp
        );
        assert_eq!(
            parse_args(["bitter-cli", "-v"]).unwrap(),
            Action::ShowVersion
        );
        assert_eq!(
            parse_args(["bitter-cli", "--version", "https://example.test"]).unwrap(),
            Action::ShowVersion
        );
    }

    #[test]
    fn parses_urls_and_passthrough() {
        assert_eq!(
            parse_args(["bitter-cli", "https://example.test", "bitter://history"]).unwrap(),
            Action::Run {
                args: vec![
                    OsString::from("https://example.test"),
                    OsString::from("bitter://history")
                ]
            }
        );
        assert_eq!(
            parse_args(["bitter-cli", "--", "--not-an-option"]).unwrap(),
            Action::Run {
                args: vec![OsString::from("--"), OsString::from("--not-an-option")]
            }
        );
    }

    #[test]
    fn rejects_unknown_options() {
        let error = parse_args(["bitter-cli", "--nope"]).unwrap_err();
        assert!(error.to_string().contains("unknown option: --nope"));
    }

    #[test]
    fn help_and_version_text_are_stable() {
        let help = help_text("bitter-cli");
        assert!(help.contains("Usage: bitter-cli [OPTION] [URL...]"));
        assert!(help.contains("--help"));
        assert!(help.contains("--version"));
        assert!(help.contains("Profile selection is not implemented"));
        assert_eq!(version_text(), format!("Bitter Browser {VERSION}"));
    }

    #[test]
    fn browser_binary_honors_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let real_old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/tmp/sentinel-bitter-ui");
        }
        let old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/tmp/fake-bitter-ui");
        }
        assert_eq!(browser_binary(), PathBuf::from("/tmp/fake-bitter-ui"));
        unsafe {
            match old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
            match real_old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
        }
    }

    #[test]
    fn run_reports_launch_errors() {
        let _guard = ENV_LOCK.lock().unwrap();
        let real_old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/tmp/sentinel-bitter-ui");
        }
        let old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/definitely/missing/bitter-ui");
        }
        assert_eq!(run(["bitter-cli", "https://example.test"]), 1);
        unsafe {
            match old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
            match real_old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
        }
    }

    #[test]
    fn run_handles_help_version_and_bad_args() {
        assert_eq!(run(["bitter-cli", "--help"]), 0);
        assert_eq!(run(["bitter-cli", "--version"]), 0);
        assert_eq!(run(["bitter-cli", "--bogus"]), 2);
    }

    #[test]
    fn browser_binary_falls_back_without_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let real_old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/tmp/sentinel-bitter-ui");
        }
        let old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::remove_var("BITTER_BROWSER_BIN");
        }
        let binary = browser_binary();
        // With no override and no sibling next to the test binary, fall back to the name.
        assert!(
            binary.as_path() == std::path::Path::new("bitter-ui") || binary.ends_with("bitter-ui"),
            "{binary:?}"
        );
        unsafe {
            match old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
            match real_old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
        }
    }

    #[test]
    fn run_launches_configured_binary_and_defaults_program_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let real_old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/tmp/sentinel-bitter-ui");
        }
        let old = std::env::var_os("BITTER_BROWSER_BIN");
        unsafe {
            std::env::set_var("BITTER_BROWSER_BIN", "/bin/true");
        }
        // Empty args exercises the default program name; /bin/true exercises success path.
        assert_eq!(run(std::iter::empty::<&str>()), 0);
        assert_eq!(run(["bitter-cli"]), 0);
        unsafe {
            match old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
            match real_old {
                Some(value) => std::env::set_var("BITTER_BROWSER_BIN", value),
                None => std::env::remove_var("BITTER_BROWSER_BIN"),
            }
        }
    }
}
