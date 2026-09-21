use std::sync::Arc;

use color_eyre::{
	Section, SectionExt,
	eyre::{Result, WrapErr, bail, eyre}
};
use educe::Educe;
use glacier_commons::{game::GlacierGame, metadata::RuntimeID};
use rkyv::rancor;
use rune::{
	Sources, TypeHash, Unit, Vm,
	runtime::{RuntimeContext, SyncFunction}
};
use simple_mod_framework_core::{
	game::{NominalPartition, ResourceSpecifier},
	utils::ResultExt
};
use simple_mod_framework_types::{Config, ScriptError, VersionPlatform};
use tokio::runtime::Handle;
use tryvial::try_fn;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
	graph::{Attribution, GraphOperation, RUNE_OPERATIONS, RuneOperation, rune_operation},
	register_operation,
	state::{Mutation, ResourceState, State},
	world::World
};

#[derive(Educe, better_rune_derive::Any)]
#[educe(Debug)]
pub struct ScriptOperation {
	pub required: Vec<ResourceSpecifier>,
	pub affected: Vec<ResourceSpecifier>,

	/// The operation to perform.
	///
	/// This MUST be deterministic based on the script's source code and the state of any required resources.
	/// If the operation captures values which are non-deterministic in the enclosing script (e.g. if you generate a random value in the operations() function and then use it in your script operation)
	/// this will not work, as the operation will be cached and it will not be run again.
	///
	/// If you wish to perform a non-deterministic operation, you should incorporate any randomness into the operation's ID, thus preventing it from being cached.
	///
	/// Must also be idempotent, as if the operation returns a type error it will be re-run to obtain debug information. This should be the case anyway unless something is broken.
	#[educe(Debug(ignore))]
	pub operation: SyncFunction,

	#[educe(Debug(ignore))]
	pub rune_context: (Arc<RuntimeContext>, Arc<Unit>, Arc<Sources>),

	/// A hash for the function. Since Rune functions cannot themselves be serialised or hashed, this is simply a hash of the enclosing script's source code.
	#[educe(Debug(ignore))]
	pub hash: u64
}

#[derive(better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework, install_with = Self::rune_install)]
#[rune_functions(
	Self::get_resource,
	Self::get_resource_history,
	Self::infer_resource_specifier,
	Self::realise_resource_specifier
)]
pub struct OperationContext {
	#[rune(get)]
	pub game: VersionPlatform,

	state: Arc<State<dyn World>>,
	required: Vec<ResourceSpecifier>
}

impl OperationContext {
	#[try_fn]
	fn rune_install(module: &mut rune::Module) -> Result<(), rune::ContextError> {
		module.field_function(&rune::runtime::Protocol::GET, "config", |s: &Self| -> Config {
			(*s.state.config).to_owned()
		})?;
	}

	#[try_fn]
	#[rune::function(instance, path = Self::get_resource)]
	fn get_resource(&self, required: &mut ResourceSpecifier) -> Result<ResourceState, ScriptError> {
		if !self.required.contains(required) {
			return Err(eyre!("Resource {:?} was not marked as required", required).into());
		}

		let states = tokio::task::block_in_place(|| {
			Handle::current().block_on(self.state.deployment.resource_states.get(required).unwrap().lock())
		});

		let mut res = (!states.is_empty())
			.then_some(states)
			.ok_or_else(|| eyre!("Necessary resource {:?} not preloaded", required))
			.suggestion("is a file being patched that has not yet been deployed?");

		if self.game.version != GlacierGame::H3
			&& required.partition
				!= *NominalPartition("super".into())
					.real_candidates(self.game)
					.unwrap()
					.first()
					.unwrap()
		{
			res = res.suggestion("is the game DLC for this location installed?");
		}

		res?.back().unwrap().to_owned().1
	}

	#[try_fn]
	#[rune::function(instance, path = Self::get_resource_history)]
	fn get_resource_history(
		&self,
		required: &mut ResourceSpecifier
	) -> Result<Vec<(Option<Attribution>, ResourceState)>, ScriptError> {
		if !self.required.contains(required) {
			return Err(eyre!("Cannot get resource {:?} which is not marked as required", required).into());
		}

		let states = tokio::task::block_in_place(|| {
			Handle::current().block_on(self.state.deployment.resource_states.get(required).unwrap().lock())
		});

		let mut res = (!states.is_empty())
			.then_some(states)
			.ok_or_else(|| eyre!("Necessary resource {:?} not preloaded", required))
			.suggestion("is a file being patched that has not yet been deployed?");

		if self.game.version != GlacierGame::H3
			&& required.partition
				!= *NominalPartition("super".into())
					.real_candidates(self.game)
					.unwrap()
					.first()
					.unwrap()
		{
			res = res.suggestion("is the game DLC for this location installed?");
		}

		res?.iter().cloned().collect()
	}

	#[try_fn]
	#[rune::function(instance, path = Self::infer_resource_specifier)]
	fn infer_resource_specifier(&self, id: &mut RuntimeID) -> Result<Option<ResourceSpecifier>, ScriptError> {
		self.state.game.infer_resource_specifier(*id)?
	}

	#[try_fn]
	#[rune::function(instance, path = Self::realise_resource_specifier)]
	fn realise_resource_specifier(
		&self,
		id: &mut RuntimeID,
		partition: &mut NominalPartition
	) -> Result<Option<ResourceSpecifier>, ScriptError> {
		self.state.game.realise_resource_specifier(*id, partition.to_owned())?
	}
}

register_operation!(ScriptOperation);

impl GraphOperation for ScriptOperation {
	fn get_required(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		self.required.to_owned()
	}

	fn get_affected(&self, _: VersionPlatform) -> Vec<ResourceSpecifier> {
		self.affected.to_owned()
	}

	fn get_hash(&self) -> u64 {
		xxh3_64(
			&rkyv::to_bytes::<rancor::BoxedError>(&(self.required.to_owned(), self.affected.to_owned(), self.hash))
				.expect("Couldn't serialise operation")
		)
	}

	#[try_fn]
	async fn evaluate(self, _: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>> {
		let res = self
			.operation
			.call::<rune::Value>((&mut OperationContext {
				game: (&*state.game).into(),
				state: State {
					game: state.game.clone(),
					config: state.config.clone(),
					deployment: state.deployment.clone(),
					localisation_hash_list: state.localisation_hash_list.clone(),
					world: state.world.clone() as Arc<dyn World>
				}
				.into(),
				required: self.required.to_owned()
			},))
			.into_result()
			.map_err(|e| {
				let mut output = rune::termcolor::Buffer::ansi();

				let mut err = eyre!("Error in script execution");

				if e.emit(&mut output, &self.rune_context.2).is_ok() {
					err = err.section(
						String::from_utf8_lossy(output.as_slice())
							.trim()
							.to_owned()
							.header("Script error:")
					);
				} else {
					err = err.section(format!("{e}"));
				}

				err
			})
			.intentional()?;

		if res.type_hash() == Result::<Vec<Mutation>, rune::Value>::HASH {
			let mut vm = Vm::new(self.rune_context.0, self.rune_context.1);
			return rune::from_value::<Result<Vec<Mutation>, rune::Value>>(res)
				.wrap_err("Failed to parse operation result")?
				.map_err(|e| {
					eyre!("Script returned error").section(if e.type_hash() == anyhow::Error::HASH {
						format!("{:?}", rune::from_value::<anyhow::Error>(e).unwrap()).header("Script error:")
					} else {
						vm.call([format!("r{}_display", self.hash).as_str()], (&e,))
							.or_else(|_| vm.call([format!("r{}_dbg", self.hash).as_str()], (&e,)))
							.ok()
							.and_then(|v| rune::from_value::<String>(v).ok().map(|x| x.trim().to_owned()))
							.unwrap_or_else(|| {
								vm.with(|| e.into_type_name())
									.into_result()
									.map(|x| format!("Can't display error of type {x:?}"))
									.unwrap_or_else(|_| "Can't display error".into())
							})
							.header("Script error:")
					})
				})
				.intentional();
		}

		let res: Vec<Mutation> = match rune::from_value(res) {
			Ok(value) => value,

			Err(e) => {
				// Re-run to stringify the resulting value
				let res = self
					.operation
					.call::<rune::Value>((&mut OperationContext {
						game: (&*state.game).into(),
						state: State {
							game: state.game.clone(),
							config: state.config.clone(),
							deployment: state.deployment.clone(),
							localisation_hash_list: state.localisation_hash_list.clone(),
							world: state.world.clone() as Arc<dyn World>
						}
						.into(),
						required: self.required
					},))
					.into_result()
					.intentional()
					.wrap_err("Operation returned incorrect type but second run failed")?;

				let mut vm = Vm::new(self.rune_context.0, self.rune_context.1);

				return Err(eyre!(e).with_note(|| {
					vm.call([format!("r{}_display", self.hash).as_str()], (res.to_owned(),))
						.or_else(|_| vm.call([format!("r{}_dbg", self.hash).as_str()], (res,)))
						.ok()
						.and_then(|v| rune::from_value::<String>(v).ok().map(|x| x.trim().to_owned()))
						.map(|v| format!("operation should return Vec<Mutation> but got value: {v}"))
						.unwrap_or_else(|| "operation should return Vec<Mutation>".into())
				}))
				.wrap_err("Incorrect return type")
				.intentional();
			}
		};

		if let Some(x) = self.affected.iter().find(|&x| {
			!res.iter()
				.any(|y| matches!(y, Mutation::SetResourceValue { resource, .. } if resource == x))
		}) {
			bail!("Resource {x:?} was marked as affected but was not mutated");
		}

		if let Some(x) = res.iter().find_map(|x| match x {
			Mutation::SetResourceValue { resource, .. } if !self.affected.contains(resource) => Some(resource),
			_ => None
		}) {
			bail!("Cannot modify resource {x:?} which is not marked as affected");
		}

		res
	}
}
