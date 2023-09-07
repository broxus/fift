use std::sync::atomic::{AtomicBool, Ordering};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static FALLBACK_TO_HELP: AtomicBool = AtomicBool::new(true);

pub struct ArgsOrVersion<T>(pub T);

impl<T: argh::FromArgs> argh::TopLevelCommand for ArgsOrVersion<T> {}

impl<T: argh::FromArgs> argh::FromArgs for ArgsOrVersion<T> {
    fn from_args(command_name: &[&str], args: &[&str]) -> Result<Self, argh::EarlyExit> {
        /// Also use argh for catching `--version`-only invocations
        #[derive(Debug, argh::FromArgs)]
        struct Version {
            /// print version information and exit
            #[argh(switch, short = 'v')]
            pub version: bool,
        }

        match Version::from_args(command_name, args) {
            Ok(v) if v.version => {
                FALLBACK_TO_HELP.store(false, Ordering::Release);

                Err(argh::EarlyExit {
                    output: format!("{} {}", command_name.first().unwrap_or(&""), VERSION),
                    status: Ok(()),
                })
            }
            Err(exit) if exit.status.is_ok() => {
                let help = match T::from_args(command_name, &["--help"]) {
                    Ok(_) => unreachable!(),
                    Err(exit) => exit.output,
                };
                Err(argh::EarlyExit {
                    output: format!("{help}  -v, --version     print version information and exit"),
                    status: Ok(()),
                })
            }
            _ => T::from_args(command_name, args).map(|app| Self(app)),
        }
    }
}

pub struct RestArgs<T, D>(pub T, pub Vec<String>, pub D);

impl<T: argh::FromArgs, D: RestArgsDelimiter> argh::TopLevelCommand for RestArgs<T, D> {}

impl<T: argh::FromArgs, D: RestArgsDelimiter> argh::FromArgs for RestArgs<T, D> {
    fn from_args(command_name: &[&str], args: &[&str]) -> Result<Self, argh::EarlyExit> {
        let (args, rest_args) = if let Some(pos) = args.iter().position(|arg| *arg == D::DELIM) {
            let (args, rest) = args.split_at(pos);
            (args, &rest[1..])
        } else {
            (args, [].as_slice())
        };

        match T::from_args(command_name, args) {
            Ok(args) => Ok(Self(
                args,
                rest_args.iter().map(ToString::to_string).collect(),
                D::default(),
            )),
            Err(exit) if exit.status.is_ok() && FALLBACK_TO_HELP.load(Ordering::Acquire) => {
                let help = match T::from_args(command_name, &["--help"]) {
                    Ok(_) => unreachable!(),
                    Err(exit) => exit.output,
                };
                Err(argh::EarlyExit {
                    output: format!("{help}\n  {:<16}  {}", D::DELIM, D::DESCR),
                    status: Ok(()),
                })
            }
            Err(exit) => Err(exit),
        }
    }
}

pub trait RestArgsDelimiter: Default {
    const DELIM: &'static str;
    const DESCR: &'static str;
}
