use std::process::ExitCode;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
pub async fn main() -> color_eyre::Result<ExitCode, color_eyre::Report> {
	std::env::set_current_dir(std::env::current_exe()?.parent().unwrap())?;
	simple_mod_framework::run::main().await
}
