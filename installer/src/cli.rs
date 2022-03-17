use crate::secure_boot::SigningInfo;
use clap::{ArgMatches, Args, Command, FromArgMatches};

#[derive(Debug, Default)]
pub struct OptionalSigningInfo(pub Option<SigningInfo>);

impl Args for OptionalSigningInfo {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_command_args() {
        use clap::CommandFactory;
        crate::Args::command().debug_assert();
    }
}
