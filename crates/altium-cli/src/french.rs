//! Clap's help and errors in French, like the rest of the CLI.
//!
//! Clap writes its help titles, its `-h` and `-V` flags and its errors in
//! English and has no translations: this module swaps in French help
//! templates and flags, and formats errors with [`French`], a port of clap's
//! `RichFormatter`.

use std::error::Error as _;
use std::fmt::Write as _;

use clap::builder::StyledStr;
use clap::builder::styling::{Style, Styles};
use clap::error::{ContextKind, ContextValue, Error, ErrorFormatter, ErrorKind};
use clap::{Arg, ArgAction, Command, CommandFactory, FromArgMatches};

/// Parses the command line like `Parser::parse`, in French.
pub fn parse<P: CommandFactory + FromArgMatches>() -> P {
    let mut command = localize(P::command());
    let parsed = match command.try_get_matches_from_mut(std::env::args_os()) {
        Ok(matches) => P::from_arg_matches(&matches).map_err(|error| error.format(&mut command)),
        Err(error) => Err(error),
    };
    parsed.unwrap_or_else(|error| error.apply::<French>().exit())
}

/// Replaces clap's English flags and help titles.
fn localize(command: Command) -> Command {
    let command = command
        .disable_help_flag(true)
        .disable_version_flag(true)
        // Its description cannot be translated; `--help` does the same job.
        .disable_help_subcommand(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .help("Affiche l'aide")
                .action(ArgAction::Help)
                .global(true),
        );
    let command = if command.get_version().is_some() {
        command.arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .help("Affiche la version")
                .action(ArgAction::Version),
        )
    } else {
        command
    };
    with_french_titles(command)
}

/// Gives `command` and its subcommands a help template with French titles.
fn with_french_titles(command: Command) -> Command {
    let styles = command.get_styles();
    let (usage, header) = (styles.get_usage(), styles.get_header());
    let mut template = format!("{{before-help}}{{about-with-newline}}\n{usage}Utilisation :{usage:#} {{usage}}");
    let mut section = |title: &str, tag: &str| {
        let _ = write!(template, "\n\n{header}{title} :{header:#}\n{{{tag}}}");
    };
    if command.has_subcommands() {
        section("Commandes", "subcommands");
    }
    if command.get_positionals().next().is_some() {
        section("Arguments", "positionals");
    }
    // Every command has at least `-h`.
    section("Options", "options");
    template.push_str("{after-help}");

    command
        .help_template(template)
        .subcommand_value_name("COMMANDE")
        .mut_subcommands(with_french_titles)
}

/// Clap's `RichFormatter`, in French.
pub struct French;

impl ErrorFormatter for French {
    fn format_error(error: &Error<Self>) -> StyledStr {
        let styles = Styles::styled();
        let (red, valid, literal) = (styles.get_error(), styles.get_valid(), styles.get_literal());
        let mut out = format!("{red}erreur :{red:#} {}", message(error, &styles));

        let mut tips: Vec<String> = [
            ContextKind::SuggestedSubcommand,
            ContextKind::SuggestedArg,
            ContextKind::SuggestedValue,
        ]
        .into_iter()
        .map(|kind| strings(error.get(kind)))
        .filter(|similar| !similar.is_empty())
        .map(|similar| {
            let similar = similar.iter().map(|text| quote(text, valid)).collect();
            format!("vouliez-vous dire {} ?", or_list(similar))
        })
        .collect();
        if let Some(ContextValue::StyledStrs(english)) = error.get(ContextKind::Suggested) {
            tips.extend(english.iter().map(|tip| tip_in_french(&tip.to_string(), &styles)));
        }
        if !tips.is_empty() {
            out.push('\n');
        }
        for tip in tips {
            let _ = write!(out, "\n  {valid}conseil :{valid:#} {tip}");
        }

        if let Some(ContextValue::StyledStr(usage)) = error.get(ContextKind::Usage) {
            // Clap's usage starts with its English title.
            let usage = usage.ansi().to_string().replacen("Usage:", "Utilisation :", 1);
            let _ = write!(out, "\n\n{usage}");
        }
        let _ = writeln!(
            out,
            "\n\nPour plus d'informations, essayez « {literal}--help{literal:#} »."
        );
        out.into()
    }
}

/// The error itself, after `erreur :`.
fn message(error: &Error<French>, styles: &Styles) -> String {
    let (invalid, literal, valid) = (styles.get_invalid(), styles.get_literal(), styles.get_valid());
    let text = |kind| match error.get(kind) {
        Some(ContextValue::String(text)) => Some(text.as_str()),
        _ => None,
    };
    let number = |kind| match error.get(kind) {
        Some(ContextValue::Number(number)) => Some(*number),
        _ => None,
    };
    let (arg, value) = (text(ContextKind::InvalidArg), text(ContextKind::InvalidValue));
    let received = |actual| format!("mais en a reçu {invalid}{actual}{invalid:#}");

    let detailed = match error.kind() {
        ErrorKind::ArgumentConflict => conflict(error, invalid),
        ErrorKind::NoEquals => {
            arg.map(|arg| format!("il faut un signe égal pour donner une valeur à {}", quote(arg, invalid)))
        }
        ErrorKind::InvalidValue => match (arg, value) {
            (Some(arg), Some("")) => Some(format!("il manque une valeur pour {}", quote(arg, invalid))),
            (Some(arg), Some(value)) => Some(format!(
                "valeur invalide {} pour {}{}",
                quote(value, invalid),
                quote(arg, literal),
                values_line("valeurs possibles", error.get(ContextKind::ValidValue), valid),
            )),
            _ => None,
        },
        ErrorKind::InvalidSubcommand => {
            text(ContextKind::InvalidSubcommand).map(|name| format!("commande inconnue {}", quote(name, invalid)))
        }
        ErrorKind::MissingRequiredArgument => {
            let missing = strings(error.get(ContextKind::InvalidArg));
            (!missing.is_empty()).then(|| {
                let mut out = String::from("arguments obligatoires manquants :");
                for arg in missing {
                    let _ = write!(out, "\n  {valid}{arg}{valid:#}");
                }
                out
            })
        }
        ErrorKind::MissingSubcommand => text(ContextKind::InvalidSubcommand).map(|name| {
            format!(
                "{} attend une commande{}",
                quote(name, invalid),
                values_line("commandes", error.get(ContextKind::ValidSubcommand), valid),
            )
        }),
        ErrorKind::TooManyValues => match (arg, value) {
            (Some(arg), Some(value)) => Some(format!(
                "valeur inattendue {} pour {} : aucune autre n'était attendue",
                quote(value, invalid),
                quote(arg, literal),
            )),
            _ => None,
        },
        ErrorKind::TooFewValues => {
            match (
                arg,
                number(ContextKind::MinValues),
                number(ContextKind::ActualNumValues),
            ) {
                (Some(arg), Some(min), Some(actual)) => Some(format!(
                    "{} attend au moins {valid}{min}{valid:#} valeurs {}",
                    quote(arg, literal),
                    received(actual),
                )),
                _ => None,
            }
        }
        ErrorKind::WrongNumberOfValues => {
            match (
                arg,
                number(ContextKind::ExpectedNumValues),
                number(ContextKind::ActualNumValues),
            ) {
                (Some(arg), Some(expected), Some(actual)) => Some(format!(
                    "{} attend {valid}{expected}{valid:#} valeurs {}",
                    quote(arg, literal),
                    received(actual),
                )),
                _ => None,
            }
        }
        ErrorKind::ValueValidation => match (arg, value) {
            (Some(arg), Some(value)) => Some(format!(
                "valeur invalide {} pour {}{}",
                quote(value, invalid),
                quote(arg, literal),
                error.source().map(|source| format!(" : {source}")).unwrap_or_default(),
            )),
            _ => None,
        },
        ErrorKind::UnknownArgument => arg.map(|arg| format!("argument inattendu {}", quote(arg, invalid))),
        ErrorKind::Io | ErrorKind::Format => error.source().map(|source| source.to_string()),
        _ => None,
    };
    detailed.unwrap_or_else(|| summary(error.kind()).to_owned())
}

/// `ArgumentConflict`: what cannot be used, then what it clashes with.
fn conflict(error: &Error<French>, invalid: &Style) -> Option<String> {
    let prior = error.get(ContextKind::PriorArg);
    let (mut out, prior) = match (
        error.get(ContextKind::InvalidArg),
        error.get(ContextKind::InvalidSubcommand),
    ) {
        (Some(ContextValue::String(arg)), _) if matches!(prior, Some(ContextValue::String(prior)) if prior == arg) => {
            let repeated = format!("l'argument {} ne peut être donné qu'une fois", quote(arg, invalid));
            (repeated, None)
        }
        (Some(ContextValue::String(arg)), _) => (
            format!("l'argument {} ne peut pas être utilisé avec", quote(arg, invalid)),
            prior,
        ),
        (_, Some(ContextValue::String(name))) => (
            format!("la commande {} ne peut pas être utilisée avec", quote(name, invalid)),
            prior,
        ),
        _ => return None,
    };
    match prior {
        Some(ContextValue::Strings(others)) => {
            out.push_str(" :");
            for other in others {
                let _ = write!(out, "\n  {invalid}{other}{invalid:#}");
            }
        }
        Some(ContextValue::String(other)) => {
            let _ = write!(out, " {}", quote(other, invalid));
        }
        Some(_) => out.push_str(" un autre des arguments donnés"),
        None => {}
    }
    Some(out)
}

/// What clap says when an error comes without details.
fn summary(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::InvalidValue => "une des valeurs n'est pas valide pour un argument",
        ErrorKind::UnknownArgument => "argument inattendu",
        ErrorKind::InvalidSubcommand => "commande inconnue",
        ErrorKind::NoEquals => "il faut un signe égal pour donner une valeur à l'un des arguments",
        ErrorKind::ValueValidation => "valeur invalide pour l'un des arguments",
        ErrorKind::TooManyValues => "valeur inattendue pour un argument",
        ErrorKind::TooFewValues => "il manque des valeurs à un argument",
        ErrorKind::WrongNumberOfValues => "mauvais nombre de valeurs pour un argument",
        ErrorKind::ArgumentConflict => "un argument ne peut pas être utilisé avec un autre des arguments donnés",
        ErrorKind::MissingRequiredArgument => "il manque des arguments obligatoires",
        ErrorKind::MissingSubcommand => "il manque une commande",
        ErrorKind::InvalidUtf8 => "un des arguments n'est pas de l'UTF-8 valide",
        _ => "ligne de commande invalide",
    }
}

/// Clap's tips that come as English sentences: the known ones are
/// translated, any other is kept as is.
fn tip_in_french(english: &str, styles: &Styles) -> String {
    let (invalid, valid) = (styles.get_invalid(), styles.get_valid());
    if let Some(rest) = english.strip_prefix("to pass '")
        && let Some((value, usage)) = rest.split_once("' as a value, use '")
    {
        let usage = usage.strip_suffix('\'').unwrap_or(usage);
        format!(
            "pour passer {} comme valeur, utilisez {}",
            quote(value, invalid),
            quote(usage, valid)
        )
    } else if let Some(rest) = english.strip_prefix("subcommand '")
        && let Some((name, _)) = rest.split_once("' exists;")
    {
        format!(
            "la commande {} existe : retirez le {} qui la précède",
            quote(name, valid),
            quote("--", invalid)
        )
    } else if let Some(found) = english
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix("' exists"))
    {
        format!("{} existe", quote(found, valid))
    } else {
        english.to_owned()
    }
}

/// Clap's `[label: a, b]` line, when it knows the accepted values.
fn values_line(label: &str, values: Option<&ContextValue>, style: &Style) -> String {
    let values: Vec<_> = strings(values)
        .iter()
        .map(|value| format!("{style}{value}{style:#}"))
        .collect();
    if values.is_empty() {
        return String::new();
    }
    format!("\n  [{label} : {}]", values.join(", "))
}

/// The text of a context holding one string or several.
fn strings(value: Option<&ContextValue>) -> Vec<&str> {
    match value {
        Some(ContextValue::String(one)) => vec![one.as_str()],
        Some(ContextValue::Strings(many)) => many.iter().map(String::as_str).collect(),
        _ => Vec::new(),
    }
}

/// `« text »`, with `text` in `style`.
fn quote(text: &str, style: &Style) -> String {
    format!("« {style}{text}{style:#} »")
}

/// `a`, `a ou b`, `a, b ou c`.
fn or_list(mut items: Vec<String>) -> String {
    let last = items.pop().unwrap_or_default();
    if items.is_empty() {
        last
    } else {
        format!("{} ou {last}", items.join(", "))
    }
}
