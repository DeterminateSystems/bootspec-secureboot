use crate::secure_boot::SigningInfo;

use clap::{ArgMatches, Args as ClapArgs, Command, FromArgMatches};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct OptionalSigningInfo(pub Option<SigningInfo>);

impl ClapArgs for OptionalSigningInfo {
    fn augment_args(cmd: Command<'_>) -> Command<'_> {
        SigningInfo::augment_args(cmd)
    }
    fn augment_args_for_update(cmd: Command<'_>) -> Command<'_> {
        SigningInfo::augment_args_for_update(cmd)
    }
}

impl FromArgMatches for OptionalSigningInfo {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let signing_key = matches.value_of_t("signing-key");
        let signing_cert = matches.value_of_t("signing-cert");
        let sbsign = matches.value_of_t("sbsign");
        let sbverify = matches.value_of_t("sbverify");

        match (signing_key, signing_cert, sbsign, sbverify) {
            (Err(_), Err(_), Err(_), Err(_)) => {
                Ok(None.into())
            },

            (Ok(signing_key), Ok(signing_cert), Ok(sbsign), Ok(sbverify)) => {
                Ok(Self(Some(SigningInfo {
                    signing_key,
                    signing_cert,
                    sbsign,
                    sbverify,
                })))
            }

            _ => {
                Err(clap::error::Error::raw(
                clap::ErrorKind::MissingRequiredArgument,
                "--signing-key, --signing-cert, --sbsign, and --sbverify are all required when signing for SecureBoot",
                ))
            },
        }
    }
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        let signing_key = matches.value_of_t("signing-key");
        let signing_cert = matches.value_of_t("signing-cert");
        let sbsign = matches.value_of_t("sbsign");
        let sbverify = matches.value_of_t("sbverify");

        match (signing_key, signing_cert, sbsign, sbverify) {
            (Err(_), Err(_), Err(_), Err(_)) => {
                self.0 = None;
                Ok(())
            },

            (Ok(signing_key), Ok(signing_cert), Ok(sbsign), Ok(sbverify)) => {
                self.0 = Some(SigningInfo {
                    signing_key,
                    signing_cert,
                    sbsign,
                    sbverify,
                });
                Ok(())
            }

            _ => {
                Err(clap::error::Error::raw(
                clap::ErrorKind::MissingRequiredArgument,
                "--signing-key, --signing-cert, --sbsign, and --sbverify are all required when signing for SecureBoot",
                ))
            },
        }
    }
}

impl From<Option<SigningInfo>> for OptionalSigningInfo {
    fn from(value: Option<SigningInfo>) -> Self {
        Self(value)
    }
}

impl std::ops::Deref for OptionalSigningInfo {
    type Target = Option<SigningInfo>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// TODO: separate by bootloader using a subcommand?
#[derive(clap::Parser, Default, Debug)]
pub struct Args {
    /// The path to the default configuration's toplevel.
    #[clap(long)]
    pub toplevel: PathBuf,

    /// Whether to actually touch stuff or not
    #[clap(long)]
    pub dry_run: bool,

    /// The directory that the generator created
    #[clap(long)]
    pub generated_entries: PathBuf,

    /// TODO
    #[clap(long)]
    pub timeout: Option<usize>,

    /// TODO
    #[clap(long)]
    pub console_mode: String,

    /// TODO
    #[clap(long)]
    pub configuration_limit: Option<usize>,

    /// TODO
    #[clap(long)]
    pub editor: bool,

    /// TODO
    #[clap(short, long, parse(from_occurrences))]
    pub verbosity: usize,

    /// TODO
    #[clap(long)]
    pub install: bool,

    // EFI-specific arguments
    /// The path to the EFI System Partition(s)
    #[clap(long)]
    pub esp: Vec<PathBuf>,

    /// Whether or not to touch EFI vars in the NVRAM
    #[clap(long)]
    pub can_touch_efi_vars: bool,

    /// TODO: bootctl path
    #[clap(long)]
    pub bootctl: Option<PathBuf>,

    /// Whether to use unified EFI files
    #[clap(long)]
    pub unified_efi: bool,

    /// The signing info used for Secure Boot
    #[clap(flatten)]
    pub signing_info: OptionalSigningInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_command_args() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
