use std::{collections::VecDeque, fmt::Debug, hash::Hash, sync::Arc, time::UNIX_EPOCH};

use color_eyre::eyre::{OptionExt, Result, WrapErr};
use ecow::{EcoString, eco_format};
use enum_dispatch::enum_dispatch;
use facet::Facet;
use futures::{StreamExt, stream::FuturesUnordered};
use glacier_commons::game::GlacierGame;
use indexmap::IndexMap;
use itertools::Itertools;
use ranked_semaphore::RankedSemaphore;
use relative_path::RelativePathBuf;
use serde::{Deserialize, Serialize};
use simple_mod_framework_core::{
	diagnostics::DiagnosticTarget,
	game::{NominalPartition, REPO_ID, ResourceSpecifier, UNLOCKABLES_ID_FL, UNLOCKABLES_ID_WOA},
	resource_in,
	utils::ResultExt
};
use simple_mod_framework_types::{HashMap, ModID, VersionPlatform};
use specta::Type;
use tracing::{Instrument, instrument};
use tryvial::try_fn;
use uuid::Uuid;
use xxhash_rust::xxh3::Xxh3Default;

use crate::{
	cache::{Cache, OperationMetadata},
	deploy::apply_graph_node,
	state::{Mutation, State},
	world::{Mods, Progress, World}
};

pub mod entity;
pub mod json;
pub mod localisation;
pub mod misc;
pub mod scripts;

pub use entity::*;
pub use json::*;
pub use localisation::*;
pub use misc::*;
pub use scripts::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type, better_rune_derive::Any)]
#[serde(rename_all = "camelCase")]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT, PARTIAL_EQ, EQ, CLONE)]
pub struct Attribution {
	/// The source of this node; the mod's ID.
	#[rune(get, set)]
	pub mod_id: ModID,

	/// The file which produced this node, relative to the mod root.
	#[specta(type = String)]
	pub source: RelativePathBuf,

	/// If this node was produced by a script, a unique identifier for the operation distinguishing it from others from the same script.
	pub script_identifier: Option<String>
}

impl Attribution {
	pub fn to_diagnostic_target(&self) -> DiagnosticTarget {
		DiagnosticTarget::Operation {
			mod_id: self.mod_id.to_owned(),
			source: self.source.to_owned(),
			script_identifier: self.script_identifier.to_owned()
		}
	}

	/// Get a hash for quickly comparing the underlying source file without reading it from disk.
	/// Will not return the same value as Operation::get_hash (which "deeply" hashes a parsed operation), as if available file times will be used instead for performance.
	#[try_fn]
	pub fn unit_hash(&self, world: &impl Mods) -> Result<Option<u64>> {
		if self.script_identifier.is_some() {
			None
		} else if let Some(mod_root) = world.get_mod_root(&self.mod_id) {
			let file_path = self.source.to_logical_path(mod_root);
			let metadata = std::fs::metadata(&file_path)?;

			let mut hasher = Xxh3Default::new();

			metadata.len().hash(&mut hasher);
			if let Ok(modified) = metadata.modified() {
				modified.duration_since(UNIX_EPOCH)?.as_secs().hash(&mut hasher);
			} else {
				return Ok(None);
			}

			let meta_path = file_path.with_added_extension("metadata.json");
			if meta_path.exists() {
				// Also hash the associated metadata file
				let metadata = std::fs::metadata(meta_path)?;

				metadata.len().hash(&mut hasher);
				if let Ok(modified) = metadata.modified() {
					modified.duration_since(UNIX_EPOCH)?.as_secs().hash(&mut hasher);
				} else {
					return Ok(None);
				}
			}

			if self
				.source
				.file_name()
				.ok_or_eyre("No file name")?
				.ends_with(".texture.json")
			{
				// Also hash the associated image file
				let metadata = std::fs::metadata(
					["dds", "png", "tga"]
						.into_iter()
						.map(|ext| file_path.with_extension(ext))
						.find(|x| x.exists())
						.ok_or_eyre("Couldn't find an associated image file")
						.intentional()?
				)?;

				metadata.len().hash(&mut hasher);
				if let Ok(modified) = metadata.modified() {
					modified.duration_since(UNIX_EPOCH)?.as_secs().hash(&mut hasher);
				} else {
					return Ok(None);
				}
			}

			Some(hasher.digest())
		} else {
			None
		}
	}

	pub fn unit(&self) -> String {
		format!("{}|{}", self.mod_id.as_str(), self.source)
	}
}

pub struct GraphNode {
	/// Information about where the node came from.
	pub attribution: Attribution,

	/// The graph dependencies of this node at the time of insertion.
	pub deps: Vec<EcoString>,

	/// The operation that will be performed, or metadata about the operation if it is available in the cache.
	pub operation: NodeOperation
}

pub enum NodeOperation {
	Cached(OperationMetadata),
	Uncached(Box<Operation>)
}

impl NodeOperation {
	pub fn required(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		match self {
			NodeOperation::Cached(meta) => meta.required.to_owned(),
			NodeOperation::Uncached(op) => op.get_required(platform)
		}
	}

	pub fn affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		match self {
			NodeOperation::Cached(meta) => meta.affected.to_owned(),
			NodeOperation::Uncached(op) => op.get_affected(platform)
		}
	}

	/// The union of required and affected resources.
	pub fn resource_deps(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier> {
		// To make sure deploy order is applied, we consider everything the operation uses or affects as dependencies
		// This avoids situations like
		// -> OverwriteTexture (resource A)
		// -> OverwriteTexture (resource A)
		// where since the second OverwriteTexture doesn't depend on the first, it could be executed in any order (incorrect)
		self.required(platform)
			.into_iter()
			.chain(self.affected(platform))
			.collect()
	}

	pub fn warn_on_identical(&self) -> bool {
		match self {
			NodeOperation::Cached(meta) => meta.warn_on_identical,
			NodeOperation::Uncached(op) => op.warn_on_identical()
		}
	}

	pub fn get_hash(&self) -> u64 {
		match self {
			NodeOperation::Cached(meta) => meta.hash,
			NodeOperation::Uncached(op) => op.get_hash()
		}
	}

	pub fn should_cache(&self) -> bool {
		match self {
			NodeOperation::Cached(_) => true,
			NodeOperation::Uncached(op) => op.should_cache()
		}
	}
}

impl GraphNode {
	/// Get the node's result, either by running its operation (and subsequently caching the result) or loading from cache.
	#[try_fn]
	pub async fn evaluate(
		self,
		cache: &Cache,
		state: &Arc<State<impl World + Mods + Progress>>,
		id: &EcoString
	) -> Result<Vec<Mutation>> {
		if let NodeOperation::Uncached(operation) = self.operation {
			let should_cache = operation.should_cache();

			let progress = should_cache
				.then(|| {
					state.world.start_spinner(
						"Applying",
						Some(
							state
								.world
								.get_mod_manifest(&self.attribution.mod_id)?
								.name
								.loc(&state.config.ui_locale)
						),
						if let Some(identifier) = self.attribution.script_identifier.as_deref() {
							identifier
						} else {
							self.attribution.source.as_str()
						}
					)
				})
				.transpose()?;

			let unit = self.attribution.unit();

			let metadata = OperationMetadata {
				hash: operation.get_hash(),
				required: operation.get_required((&*state.game).into()),
				affected: operation.get_affected((&*state.game).into()),
				warn_on_identical: operation.warn_on_identical()
			};

			let operation_type = facet::Peek::new(&*operation)
				.into_enum()
				.unwrap()
				.variant_name_active()
				.unwrap();

			let result = operation
				.evaluate(self.attribution, state.clone())
				.await
				.wrap_err_with(|| format!("Failed to evaluate {operation_type} operation"))?;

			if let Some(progress) = progress {
				state.world.finish_progress(progress)?;
			}

			if should_cache {
				cache.insert_node(
					id.into(),
					unit,
					metadata,
					self.deps.into_iter().map(|x| x.into()).collect(),
					&result
				)?;
			}

			result
		} else {
			cache
				.get_node_result(id)?
				.ok_or_eyre("No cached blob found for cached operation")?
		}
	}
}

pub struct DeployGraph {
	platform: VersionPlatform,
	nodes: IndexMap<EcoString, GraphNode>,
	deps: HashMap<EcoString, Vec<EcoString>>,
	reverse_deps: HashMap<EcoString, Vec<EcoString>>,
	ready: VecDeque<EcoString>
}

impl DeployGraph {
	pub fn new(platform: impl Into<VersionPlatform>) -> Self {
		Self {
			platform: platform.into(),
			nodes: IndexMap::new(),
			deps: Default::default(),
			reverse_deps: Default::default(),
			ready: VecDeque::new()
		}
	}

	fn make_id(&self, attribution: &Attribution, uniqueness: Option<usize>) -> EcoString {
		if let Some(uniqueness) = uniqueness {
			eco_format!("{}|{}|{}", attribution.mod_id.as_str(), attribution.source, uniqueness)
		} else {
			let candidate = eco_format!("{}|{}|0", attribution.mod_id.as_str(), attribution.source);
			if self.nodes.contains_key(&candidate) {
				eco_format!(
					"{}|{}|{}",
					attribution.mod_id.as_str(),
					attribution.source,
					self.nodes
						.keys()
						.filter(|x| x.starts_with(&format!("{}|{}|", attribution.mod_id.as_str(), attribution.source)))
						.count()
				)
			} else {
				candidate
			}
		}
	}

	/// Insert a marker node into the graph, returning its ID. The node must be in the cache, else this will panic.
	///
	/// Will automatically validate the cached dependencies and invalidate this marker if necessary. Returns true if invalidation was necessary.
	pub(crate) fn insert_marker(&mut self, cache: &Cache, attribution: Attribution, id: EcoString) -> bool {
		let cached = cache.get_node(&id).expect("Attempted to add marker for uncached node");
		let operation = NodeOperation::Cached(cached.metadata);

		let mut need_propagate = false;

		let resources = operation.resource_deps(self.platform);
		if cached.deps
			!= self
				.nodes
				.iter()
				.filter(|(_, node)| {
					node.operation
						.resource_deps(self.platform)
						.iter()
						.any(|x| resources.contains(x))
				})
				.map(|(id, _)| id.to_string())
				.collect_vec()
		{
			cache.invalidate(std::slice::from_ref(&id));
			need_propagate = true;
		}

		self.insert_node(attribution, id, operation);

		need_propagate
	}

	/// Insert a node into the graph without automatically calling [`Cache::propagate_invalidations`]. Will still call [`Cache::invalidate`] if necessary.
	pub(crate) fn insert_deferring(
		&mut self,
		cache: &Cache,
		attribution: Attribution,
		uniqueness: Option<usize>,
		operation: Operation
	) -> EcoString {
		let id = self.make_id(&attribution, uniqueness);

		self.insert_node(
			attribution,
			id.to_owned(),
			if operation.should_cache()
				&& let Some(cached) = cache.get_node(&id)
			{
				if cached.metadata.hash == operation.get_hash() {
					let cached_operation = NodeOperation::Cached(cached.metadata);
					let resources = cached_operation.resource_deps(self.platform);

					if cached.deps
						== self
							.nodes
							.iter()
							.filter(|(_, node)| {
								node.operation
									.resource_deps(self.platform)
									.iter()
									.any(|x| resources.contains(x))
							})
							.map(|(id, _)| id.to_string())
							.collect_vec()
					{
						cached_operation
					} else {
						cache.invalidate(std::slice::from_ref(&id));
						NodeOperation::Uncached(operation.into())
					}
				} else {
					cache.invalidate(std::slice::from_ref(&id));
					NodeOperation::Uncached(operation.into())
				}
			} else {
				NodeOperation::Uncached(operation.into())
			}
		);

		id
	}

	/// Insert a node into the graph, returning its ID. Will discard the operation and store a marker if valid cached data is available.
	pub fn insert(
		&mut self,
		cache: &Cache,
		attribution: Attribution,
		uniqueness: Option<usize>,
		operation: Operation
	) -> EcoString {
		let id = self.make_id(&attribution, uniqueness);

		self.insert_node(
			attribution,
			id.to_owned(),
			if operation.should_cache()
				&& let Some(cached) = cache.get_node(&id)
			{
				if cached.metadata.hash == operation.get_hash() {
					let cached_operation = NodeOperation::Cached(cached.metadata);
					let resources = cached_operation.resource_deps(self.platform);

					if cached.deps
						== self
							.nodes
							.iter()
							.filter(|(_, node)| {
								node.operation
									.resource_deps(self.platform)
									.iter()
									.any(|x| resources.contains(x))
							})
							.map(|(id, _)| id.to_string())
							.collect_vec()
					{
						cached_operation
					} else {
						cache.invalidate(std::slice::from_ref(&id));
						cache.propagate_invalidations();
						NodeOperation::Uncached(operation.into())
					}
				} else {
					cache.invalidate(std::slice::from_ref(&id));
					cache.propagate_invalidations();
					NodeOperation::Uncached(operation.into())
				}
			} else {
				NodeOperation::Uncached(operation.into())
			}
		);

		id
	}

	/// Alter the operation of an existing node if it exists and its hash is different.
	pub fn modify(&mut self, cache: &Cache, attribution: &Attribution, uniqueness: usize, operation: Operation) {
		let id = self.make_id(attribution, Some(uniqueness));
		if let Some(node) = self.nodes.get_mut(&id)
			&& (!cache.is_node_cached(&id) || node.operation.get_hash() != operation.get_hash())
		{
			node.operation = NodeOperation::Uncached(operation.into());
			cache.invalidate(std::slice::from_ref(&id));
			cache.propagate_invalidations();
		}
	}

	fn insert_node(&mut self, attribution: Attribution, id: EcoString, operation: NodeOperation) {
		let resources = operation.resource_deps(self.platform);

		let deps = self
			.nodes
			.iter()
			.filter(|(_, node)| {
				node.operation
					.resource_deps(self.platform)
					.iter()
					.any(|x| resources.contains(x))
			})
			.map(|(id, _)| id)
			.cloned()
			.collect_vec();

		self.nodes.insert(
			id.to_owned(),
			GraphNode {
				attribution,
				operation,
				deps: deps.to_owned()
			}
		);

		if deps.is_empty() {
			self.ready.push_back(id.to_owned());
		}

		for dep in &deps {
			self.reverse_deps.get_mut(dep).unwrap().push(id.to_owned());
		}

		self.deps.insert(id.to_owned(), deps);
		self.reverse_deps.insert(id, vec![]);
	}

	pub fn contains(&self, id: &str) -> bool {
		self.nodes.contains_key(id)
	}

	pub fn get(&self, id: &str) -> Option<&GraphNode> {
		self.nodes.get(id)
	}

	pub fn get_dependencies(&self, id: &str) -> Option<&Vec<EcoString>> {
		self.deps.get(id)
	}

	pub fn get_reverse_dependencies(&self, id: &str) -> Option<&Vec<EcoString>> {
		self.reverse_deps.get(id)
	}

	/// Remove a node from the graph altogether, updating the dependencies of all other nodes.
	/// Equivalent to calling [`Self::complete`] and [`Self::take`] (order is inconsequential).
	pub fn remove(&mut self, id: &str) -> Option<GraphNode> {
		self.complete(id);
		self.take(id)
	}

	/// Remove the node with the given ID from the graph. Returns the removed node if it existed.
	/// This will update reverse dependencies but NOT (forward) dependencies of other nodes! Use [`Self::complete`] before or after this to do so.
	///
	/// Coupled with [`Self::complete`], this can be used to execute nodes in their graph order:
	/// ```
	/// # use glacier_commons::game::GlacierGame;
	/// # use simple_mod_framework::graph::{AddContractToORES, Attribution, DeployGraph, GraphNode, Operation};
	/// # use simple_mod_framework_types::{ModID, Platform, VersionPlatform};
	/// # let mut graph = DeployGraph::new(VersionPlatform { version: GlacierGame::H3, platform: Platform::Epic });
	/// # graph.insert_node(GraphNode { attribution: Attribution { mod_id: ModID::try_from("Some.Mod".into()).unwrap(), mod_friendly: "Some Mod", source: "Op1", is_script: false }, operation: AddContractToORES { id: "abc".into() }.into() });
	/// # graph.insert_node(GraphNode { attribution: Attribution { mod_id: ModID::try_from("Some.Mod".into()).unwrap(), mod_friendly: "Some Mod", source: "Op2", is_script: false }, operation: AddContractToORES { id: "def".into() }.into() });
	/// while let Some(id) = graph.ready_mut().pop_front() {
	///     let node = graph.take(&id).unwrap();
	///     // Evaluate the node here, e.g. node.evaluate(state).await;
	///     graph.complete(&id);
	/// }
	/// ```
	pub fn take(&mut self, id: &str) -> Option<GraphNode> {
		if let Some(deps) = self.deps.remove(id) {
			for dep in &deps {
				if let Some(rdeps) = self.reverse_deps.get_mut(dep) {
					rdeps.retain(|x| x != id);
					if rdeps.is_empty() {
						self.reverse_deps.remove(dep);
					}
				}
			}

			Some(self.nodes.shift_remove(id).unwrap())
		} else {
			None
		}
	}

	/// Signal a node as having been completed. The node may have been removed already.
	/// This will update dependencies of other nodes to remove this one and add any to the ready queue if they no longer have dependencies.
	pub fn complete(&mut self, id: &str) {
		if let Some(reverse_deps) = self.reverse_deps.remove(id) {
			for reverse_dep in reverse_deps {
				if let Some(deps) = self.deps.get_mut(&reverse_dep) {
					deps.retain(|x| x != id);

					if deps.is_empty() {
						self.ready.push_back(reverse_dep);
					}
				}
			}
		}
	}

	pub fn ready(&self) -> &VecDeque<EcoString> {
		&self.ready
	}

	pub fn ready_mut(&mut self) -> &mut VecDeque<EcoString> {
		&mut self.ready
	}

	pub fn platform(&self) -> VersionPlatform {
		self.platform
	}

	pub fn nodes(&self) -> &IndexMap<EcoString, GraphNode> {
		&self.nodes
	}

	/// Evaluate the graph to completion in parallel, applying operations to the given State.
	#[try_fn]
	#[instrument(skip_all)]
	pub async fn evaluate(
		mut self,
		cache: &Arc<Cache>,
		state: &Arc<State<impl World + Mods + Progress>>
	) -> Result<()> {
		let progress = state
			.world
			.start_progress("Evaluating graph", self.nodes().len() as u64)?;

		let mut active_tasks = FuturesUnordered::new();

		// Allows scheduling operations with soft ordering, so that long-running operations can get started first
		let concurrency = Arc::new(RankedSemaphore::new_fifo(
			std::env::var("SMF_GRAPH_PARALLELISM")
				.ok()
				.and_then(|x| x.parse().ok())
				.unwrap_or(std::thread::available_parallelism()?.get())
		));

		loop {
			while let Some(ready_id) = self.ready_mut().pop_front() {
				let ready_node = self.take(&ready_id).unwrap();

				let reverse_dependencies = self
					.get_reverse_dependencies(&ready_id)
					.unwrap()
					.iter()
					.map(|id| self.get(id).unwrap().operation.resource_deps(self.platform))
					.collect_vec();

				let cache = cache.clone();
				let state = state.clone();
				let concurrency = concurrency.clone();

				active_tasks.push(tokio::spawn(
					async move {
						let _permit = concurrency
							.acquire_with_priority(reverse_dependencies.len() as isize)
							.await?;

						apply_graph_node(cache, state, ready_id, ready_node, reverse_dependencies).await
					}
					.instrument(tracing::info_span!("Evaluating node"))
				));
			}

			if active_tasks.is_empty() && self.nodes().is_empty() {
				break;
			}

			self.complete(&active_tasks.next().await.unwrap()??);
			state.world.advance_progress(progress, 1)?;
		}

		state.world.finish_progress(progress)?;

		// Finalise unlockables
		if let Some(res) = state.deployment.resource_states.get(&ResourceSpecifier {
			id: if state.game.version == GlacierGame::FL {
				UNLOCKABLES_ID_FL
			} else {
				UNLOCKABLES_ID_WOA
			},
			partition: NominalPartition("super".into()).real_candidates(&*state.game).unwrap()[0].to_owned()
		}) && let Some((_, res)) = res.lock().await.front()
		{
			let unlockables_data =
				glacier_bin1::deserialize::<EcoString>(&res.data).wrap_err("Couldn't parse unlockables")?;
			let asset_id = Uuid::new_v4();
			state
				.deployment
				.server_side_assets
				.lock()
				.insert(asset_id, unlockables_data.as_bytes().into());
			state.deployment.server_side_data.lock().unlockables = Some(asset_id);
		}

		// Finalise repository
		if let Some(res) = state.deployment.resource_states.get(&ResourceSpecifier {
			id: REPO_ID,
			partition: NominalPartition("super".into()).real_candidates(&*state.game).unwrap()[0].to_owned()
		}) && let Some((_, res)) = res.lock().await.front()
		{
			if state.config.auto_disable_dynres {
				state.deployment.server_side_data.lock().dynamic_resources_disabled = true;
			}

			let asset_id = Uuid::new_v4();
			state
				.deployment
				.server_side_assets
				.lock()
				.insert(asset_id, res.data.to_owned());
			state.deployment.server_side_data.lock().repository = Some(asset_id);
		}

		// Finalise world map metadata
		let world_map_metadata = state.deployment.world_map_metadata.lock().to_owned();
		for res in world_map_metadata {
			if state.deployment.all_relevant_resources.contains_key(&res)
				&& let Some((_, res)) = state.deployment.resource_states.iter().find(|(k, _)| k.id == res)
				&& let Some((_, res)) = res.lock().await.front()
			{
				let scene = serde_json::from_slice::<serde_json::Value>(&res.data)
					.wrap_err("World map metadata file was invalid JSON")?
					.get("scene")
					.ok_or_eyre("World map metadata file had no scene property")?
					.as_str()
					.ok_or_eyre("World map metadata file scene property was not string")?
					.to_owned();

				let asset_id = Uuid::new_v4();
				state
					.deployment
					.server_side_assets
					.lock()
					.insert(asset_id, res.data.to_owned());
				state
					.deployment
					.server_side_data
					.lock()
					.world_map_metadata
					.insert(scene, asset_id);
			}
		}

		// Finalise storyconfig
		if let Some(res) = state.deployment.resource_states.get(&resource_in!(
			"[assembly:/_pro/online/default/cloudstorage/resources/storyconfig.json].pc_json",
			"super",
			&*state.game
		)) && let Some((_, res)) = res.lock().await.front()
		{
			let asset_id = Uuid::new_v4();
			state
				.deployment
				.server_side_assets
				.lock()
				.insert(asset_id, res.data.to_owned());
			state.deployment.server_side_data.lock().story_config = Some(asset_id);
		}
	}
}

/// An operation to perform.
/// Operations should be split as granularly as possible to enhance concurrency.
/// For example, contract adding is split because writing contract JSON files can be done concurrently, while writing the ORES requires ordering.
#[allow(async_fn_in_trait)]
#[enum_dispatch]
pub trait GraphOperation: Send + Sync {
	/// Whether the cache should be considered at all for this operation.
	/// If false, the operation will always be run - only to be used for cases where the cache would add more overhead than it saves.
	fn should_cache(&self) -> bool {
		true
	}

	/// Whether this operation should warn if any produced mutation has no effect (e.g. if the resource is already identical).
	fn warn_on_identical(&self) -> bool {
		true
	}

	/// Get the resources which this operation needs, used to load all resources in before evaluating the graph.
	fn get_required(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier>;

	/// Get the resources which are modified by this operation.
	fn get_affected(&self, platform: VersionPlatform) -> Vec<ResourceSpecifier>;

	/// Return a hash for quickly comparing the contents of this operation. Only for caching.
	/// This is used in favour of hashing file contents to allow for caching of script-created graph nodes.
	fn get_hash(&self) -> u64;

	/// Evaluate the operation, returning a list of mutations to apply to the state.
	async fn evaluate(self, attribution: Attribution, state: Arc<State<impl World>>) -> Result<Vec<Mutation>>;
}

#[derive(Facet, Debug, better_rune_derive::Any)]
#[rune(item = ::simple_mod_framework)]
#[rune_derive(DEBUG_FMT)]
#[repr(u8)]
#[enum_dispatch(GraphOperation)]
pub enum Operation {
	ApplyQuickEntityPatch(#[facet(opaque)] ApplyQuickEntityPatch),
	OverwriteEntity(#[facet(opaque)] OverwriteEntity),
	ApplyJSONMergePatch(#[facet(opaque)] ApplyJSONMergePatch),
	ApplyJSONPatch(#[facet(opaque)] ApplyJSONPatch),
	AddContractToORES(#[facet(opaque)] AddContractToORES),
	WriteContractJSON(#[facet(opaque)] WriteContractJSON),
	AddContractToDestinations(#[facet(opaque)] AddContractToDestinations),
	OverwriteMaterial(#[facet(opaque)] OverwriteMaterial),
	OverwriteMaterialEntity(#[facet(opaque)] OverwriteMaterialEntity),
	OverwriteTexture(#[facet(opaque)] OverwriteTexture),
	OverwriteSFX(#[facet(opaque)] OverwriteSFX),
	OverwriteLanguageFile(#[facet(opaque)] OverwriteLanguageFile),
	AddBlobsToORES(#[facet(opaque)] AddBlobsToORES),
	AddLocalisation(#[facet(opaque)] AddLocalisation),
	OverrideLocalisation(#[facet(opaque)] OverrideLocalisation),
	WriteLocalisedLine(#[facet(opaque)] WriteLocalisedLine),
	AddPackageDefinitionEntry(#[facet(opaque)] AddPackageDefinitionEntry),
	OverwriteRawResource(#[facet(opaque)] OverwriteRawResource),
	OverwriteSoundDefinitions(#[facet(opaque)] OverwriteSoundDefinitions),
	PatchSoundDefinitions(#[facet(opaque)] PatchSoundDefinitions),
	ScriptOperation(#[facet(opaque)] ScriptOperation),
	OverwriteBehaviorTree(#[facet(opaque)] OverwriteBehaviorTree),
	OverwriteAspectEntity(#[facet(opaque)] OverwriteAspectEntity)
}

pub trait RuneOperation: Send + Sync {
	/// Install this operation into the SMF module.
	fn install(&self, module: &mut rune::Module) -> Result<(), rune::ContextError>;

	/// Get the name of this operation's type for debugging purposes.
	fn type_name(&self) -> String;

	/// Get the Rune type hash for this operation.
	fn type_hash(&self) -> rune::Hash;

	/// Attempt to downcast a rune::Value into this operation's specific type and then convert it to Operation.
	fn downcast(&self, value: rune::Value) -> Result<Operation, rune::runtime::RuntimeError>;
}

#[macro_export]
macro_rules! register_operation {
	($op:ident) => {
		mident::mident! {
			#[linkme::distributed_slice(RUNE_OPERATIONS)]
			static #concat(REGISTER_ $op): &dyn RuneOperation =
				&rune_operation::<$op>() as &dyn RuneOperation;
		}
	};
}

#[linkme::distributed_slice]
pub static RUNE_OPERATIONS: [&'static dyn RuneOperation];

pub struct RegisteredRuneOperation<T>(std::marker::PhantomData<T>)
where
	T: GraphOperation
		+ Into<Operation>
		+ rune::__priv::AnyMarker
		+ rune::runtime::TypeOf
		+ rune::runtime::MaybeTypeOf
		+ rune::module::InstallWith;

impl<T> RuneOperation for RegisteredRuneOperation<T>
where
	T: GraphOperation
		+ Into<Operation>
		+ rune::__priv::AnyMarker
		+ rune::runtime::TypeOf
		+ rune::runtime::MaybeTypeOf
		+ rune::module::InstallWith
{
	#[try_fn]
	fn install(&self, module: &mut rune::Module) -> Result<(), rune::ContextError> {
		module.ty::<T>()?;
	}

	fn type_name(&self) -> String {
		T::ITEM.base_name().unwrap().into()
	}

	fn type_hash(&self) -> rune::Hash {
		T::HASH
	}

	fn downcast(&self, value: rune::Value) -> Result<Operation, rune::runtime::RuntimeError> {
		rune::from_value::<T>(value).map(Into::into)
	}
}

pub const fn rune_operation<T>() -> RegisteredRuneOperation<T>
where
	T: GraphOperation
		+ Into<Operation>
		+ rune::__priv::AnyMarker
		+ rune::runtime::TypeOf
		+ rune::runtime::MaybeTypeOf
		+ rune::module::InstallWith
{
	RegisteredRuneOperation(std::marker::PhantomData)
}
