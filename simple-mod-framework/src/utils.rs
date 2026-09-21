use std::{
	env,
	fmt::Write as _,
	fs::{self, File},
	io::Write as _,
	path::PathBuf,
	sync::{
		LazyLock,
		atomic::{AtomicUsize, Ordering}
	},
	time::SystemTime
};

use color_eyre::{Section, SectionExt, eyre::Result};
use ecow::{EcoString, eco_format};
use rand::{
	rng,
	seq::{IteratorRandom, SliceRandom}
};

use super::{APP_VERSION, EXPERIMENT};

pub static DEBUG_PROFILE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	let path = std::env::current_dir()
		.expect("Can't access current directory")
		.join("debug");

	if path.exists() {
		fs::remove_dir_all(&path).expect("Couldn't remove existing debug profile directory");
	}

	fs::create_dir_all(path.join("artifacts")).expect("Couldn't create debug profile directory");

	let mut profile = File::create(path.join("README.txt")).expect("Couldn't write basic debug info");

	writeln!(profile, "Simple Mod Framework debug profile").expect("Couldn't write basic debug info");
	writeln!(
		profile,
		"Version: {} (experiment {})",
		*APP_VERSION,
		EXPERIMENT.unwrap_or("none")
	)
	.expect("Couldn't write basic debug info");
	writeln!(
		profile,
		"Time: {}",
		SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_secs()
	)
	.expect("Couldn't write basic debug info");
	writeln!(
		profile,
		"\nThe user's SMF config and deploy log should be present in this folder. Additionally, any artifacts \
		 referenced by the log can be found in the artifacts folder named as their ID."
	)
	.expect("Couldn't write basic debug info");

	fs::copy("config.json", path.join("config.json")).expect("Couldn't copy config to debug profile");

	path
});

#[allow(async_fn_in_trait)]
pub trait ResultArtifacts<T> {
	fn with_artifacts(self, f: impl Fn() -> Vec<(String, Vec<u8>)>) -> Result<T>;
	async fn with_async_artifacts(self, f: impl AsyncFn() -> Vec<(String, Vec<u8>)>) -> Result<T>;
}

impl<T, E: Into<color_eyre::Report>> ResultArtifacts<T> for Result<T, E> {
	fn with_artifacts(self, f: impl Fn() -> Vec<(String, Vec<u8>)>) -> Result<T> {
		match self {
			Ok(x) => Ok(x),
			Err(err) => {
				let artifacts = f();

				Err(err.into().section(
					{
						let mut s = String::new();

						for (name, data) in artifacts {
							writeln!(s, "{}: artifact {}", name, write_artifact(&data))?;
						}

						s
					}
					.header("Relevant artifacts:")
				))
			}
		}
	}

	async fn with_async_artifacts(self, f: impl AsyncFn() -> Vec<(String, Vec<u8>)>) -> Result<T> {
		match self {
			Ok(x) => Ok(x),
			Err(err) => {
				let artifacts = f().await;

				Err(err.into().section(
					{
						let mut s = String::new();

						for (name, data) in artifacts {
							writeln!(s, "{}: artifact {}", name, write_artifact(&data))?;
						}

						s
					}
					.header("Relevant artifacts:")
				))
			}
		}
	}
}

// Why is this not in the standard library
pub trait TryIter: Iterator {
	fn try_any<F>(&mut self, f: F) -> Result<bool>
	where
		F: FnMut(Self::Item) -> Result<bool>;

	fn try_all<F>(&mut self, f: F) -> Result<bool>
	where
		F: FnMut(Self::Item) -> Result<bool>;

	fn try_position<F>(&mut self, f: F) -> Result<Option<usize>>
	where
		F: FnMut(Self::Item) -> Result<bool>;
}

impl<T: Sized> TryIter for T
where
	T: Iterator
{
	fn try_all<F>(&mut self, mut f: F) -> Result<bool>
	where
		F: FnMut(Self::Item) -> Result<bool>
	{
		for x in self {
			if !(f(x)?) {
				return Ok(false);
			}
		}

		Ok(true)
	}

	fn try_any<F>(&mut self, mut f: F) -> Result<bool>
	where
		F: FnMut(Self::Item) -> Result<bool>
	{
		for x in self {
			if f(x)? {
				return Ok(true);
			}
		}

		Ok(false)
	}

	fn try_position<F>(&mut self, mut f: F) -> Result<Option<usize>>
	where
		F: FnMut(Self::Item) -> Result<bool>
	{
		for (i, x) in self.enumerate() {
			if f(x)? {
				return Ok(Some(i));
			}
		}

		Ok(None)
	}
}

static ARTIFACT_NAMES: LazyLock<[&str; 26]> = LazyLock::new(|| {
	let mut names = [
		"Alfred", "Basil", "Clyde", "Daisy", "Esme", "Felix", "Gus", "Hugo", "Ivy", "Jasmine", "Keira", "Lydia",
		"Miranda", "Noah", "Oswald", "Percy", "Quinn", "Ruth", "Sadie", "Theo", "Ursula", "Virgil", "Willow", "Xavier",
		"Yara", "Zachary"
	];

	names.shuffle(&mut rng());

	names
});

static ARTIFACT_IDX: AtomicUsize = AtomicUsize::new(0);

const ARTIFACT_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Write a debugging artifact to the disk. Returns the artifact ID.
pub fn write_artifact(data: &[u8]) -> EcoString {
	let idx = (ARTIFACT_IDX
		.try_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some((x + 1) % 26))
		.expect("Couldn't increment artifact index")
		+ 1) % 26;

	let id = eco_format!(
		"{}+{}{}{}{}{}",
		ARTIFACT_NAMES.get(idx).unwrap(),
		ARTIFACT_CHARS.chars().choose(&mut rng()).unwrap(),
		ARTIFACT_CHARS.chars().choose(&mut rng()).unwrap(),
		ARTIFACT_CHARS.chars().choose(&mut rng()).unwrap(),
		ARTIFACT_CHARS.chars().choose(&mut rng()).unwrap(),
		ARTIFACT_CHARS.chars().choose(&mut rng()).unwrap()
	);

	fs::write(
		env::current_dir()
			.expect("Can't access current directory")
			.join(DEBUG_PROFILE_DIR.as_path())
			.join("artifacts")
			.join(id.as_str()),
		data
	)
	.expect("Couldn't write debug artifact");

	id
}
