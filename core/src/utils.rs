use std::fmt::Display;

use color_eyre::{Help, Result, eyre::WrapErr};
use fn_wrap_err::wrap_err;
use tryvial::try_fn;

pub trait ResultExt<T> {
	fn intentional(self) -> Result<T>;
}

impl<T, E: Into<color_eyre::Report>> ResultExt<T> for Result<T, E> {
	fn intentional(self) -> Result<T> {
		self.note("this error is due to a mod problem; remove any mods which emit warnings/errors and try again")
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntentionalHalt {
	pub target: Option<String>,
	pub message: String
}

impl Display for IntentionalHalt {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.target.as_deref().unwrap_or("Main"), self.message)
	}
}

impl std::error::Error for IntentionalHalt {}

#[macro_export]
macro_rules! intentional_halt {
	($target:expr, $err:expr) => {
		return Err($crate::utils::IntentionalHalt {
			target: Some(format!("{}", $target)),
			message: format!("{}", $err)
		}
		.into());
	};

	($err:expr) => {
		return Err($crate::utils::IntentionalHalt {
			target: None,
			message: format!("{}", $err)
		}
		.into());
	};
}

#[try_fn]
#[wrap_err("Couldn't format JSON")]
pub fn format_json(data: &str) -> Result<String> {
	biome_json_formatter::format_node(
		biome_json_formatter::context::JsonFormatOptions::new()
			.with_indent_style(biome_formatter::IndentStyle::Tab)
			.with_indent_width(biome_formatter::IndentWidth::from(4))
			.with_line_ending(biome_formatter::LineEnding::Lf)
			.with_line_width("75".parse().unwrap())
			.with_trailing_commas(biome_json_formatter::context::TrailingCommas::None),
		&biome_json_parser::parse_json(
			data,
			biome_json_parser::JsonParserOptions {
				allow_comments: true,
				allow_trailing_commas: true
			}
		)
		.syntax()
	)
	.wrap_err("Couldn't format with Biome")?
	.print()
	.wrap_err("Couldn't print formatted JSON")?
	.into_code()
}
