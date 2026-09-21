use std::{
	fs,
	path::{Path, PathBuf}
};

use color_eyre::{Result, eyre::Context};
use ecow::EcoString;
use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions, PersistMode};
use itertools::Itertools;
use parking_lot::RwLock;
use rkyv::{Archive, Deserialize, Serialize, rancor};
use simple_mod_framework_core::game::ResourceSpecifier;
use simple_mod_framework_types::HashMap;
use tracing::instrument;
use tryvial::try_fn;
use xxhash_rust::xxh3::xxh3_64;

use crate::{APP_VERSION, graph::DeployGraph, state::Mutation};

#[derive(Archive, Serialize, Deserialize)]
pub struct CacheMetadata {
	pub framework_version: String,
	pub game_hash: String,
	pub units: HashMap<String, CachedUnit>,
	pub nodes: HashMap<String, CachedNode>
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct CachedUnit {
	pub hash: u64,
	pub nodes: Vec<String>
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct OperationMetadata {
	pub hash: u64,

	/// All resources this operation will use.
	pub required: Vec<ResourceSpecifier>,

	/// All resources this operation will modify.
	pub affected: Vec<ResourceSpecifier>,

	pub warn_on_identical: bool
}

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct CachedNode {
	/// The unit this node belongs to.
	pub unit: String,

	pub metadata: OperationMetadata,

	/// The ID in storage of the cached results of this node.
	pub result: u64,

	/// The graph dependencies of this node.
	pub deps: Vec<String>
}

pub struct Cache {
	pub root: PathBuf,
	pub metadata: RwLock<CacheMetadata>,
	pub storage: Database,
	pub blobs: Keyspace
}

impl Cache {
	/// Open or create a cache at the given path for the given game hash.
	#[try_fn]
	#[instrument(skip_all)]
	pub fn new(root: impl AsRef<Path>, game_hash: &str) -> Result<Self> {
		let root = root.as_ref();

		fs::create_dir_all(root).wrap_err("Couldn't create cache folder")?;

		let metadata = fs::read(root.join("cache.meta"))
			.ok()
			.and_then(|x| {
				rkyv::from_bytes::<_, rancor::BoxedError>(&x)
					.inspect_err(|e| {
						log::trace!("Couldn't deserialise cache: {e}");
					})
					.ok()
			})
			.and_then(|x: CacheMetadata| {
				if x.framework_version != APP_VERSION.to_string() {
					log::trace!(
						"Throwing away cache due to version mismatch: {}/{}",
						x.framework_version,
						*APP_VERSION
					);
					None
				} else if x.game_hash != game_hash {
					log::trace!(
						"Throwing away cache due to game hash mismatch: {}/{}",
						x.game_hash,
						game_hash
					);
					None
				} else {
					Some(x)
				}
			})
			.unwrap_or_else(|| CacheMetadata {
				framework_version: APP_VERSION.to_string(),
				game_hash: game_hash.into(),
				units: Default::default(),
				nodes: Default::default()
			});

		log::trace!("Opening cache storage");

		let storage = fjall::Database::builder("cache")
			.open()
			.wrap_err("Failed to open cache storage")?;

		let blobs = storage
			.keyspace("blobs", || {
				KeyspaceCreateOptions::default().with_kv_separation(Some(KvSeparationOptions::default()))
			})
			.wrap_err("Failed to open cache storage partition")?;

		Cache {
			root: root.into(),
			metadata: metadata.into(),
			storage,
			blobs
		}
	}

	/// Remove any cached units or nodes which are no longer present in the given graph.
	pub fn gc(&self, graph: &DeployGraph) {
		let mut metadata = self.metadata.write();

		metadata
			.units
			.retain(|id, _| graph.nodes().values().any(|x| x.attribution.unit() == *id));

		metadata.nodes.retain(|id, _| graph.nodes().contains_key(id.as_str()));
	}

	/// Save the cache metadata to disk and clean up any unused blobs.
	#[try_fn]
	#[instrument(skip_all)]
	pub fn persist(&self) -> Result<()> {
		let metadata = self.metadata.read();

		fs::write(
			self.root.join("cache.meta"),
			rkyv::to_bytes::<rancor::BoxedError>(&*metadata)?
		)?;

		// Remove any mutations no longer in the cache
		for guard in self.blobs.iter() {
			let key = guard.key()?;

			let blob = u64::from_be_bytes(key.as_ref().try_into()?);
			if !metadata.nodes.values().any(|x| x.result == blob) {
				self.blobs.remove(key)?;
			}
		}

		self.storage.persist(PersistMode::SyncAll)?;
	}

	/// Compare the given hash with the unit's cached metadata and remove it from the cache if there is a mismatch.
	/// Returns whether the cache was invalidated and the unit must be re-analysed.
	pub fn unit_invalidated(&self, id: &str, hash: u64) -> bool {
		let mut metadata = self.metadata.write();

		if let Some(info) = metadata.units.get(id) {
			if info.hash != hash {
				// Changed
				log::trace!("Unit {id} was modified, re-analysing");
				metadata.units.remove(id);
				true
			} else if info.nodes.iter().any(|x| !metadata.nodes.contains_key(x)) {
				// A node is uncached
				log::trace!("Unit {id} has uncached node, re-analysing");
				metadata.units.remove(id);
				true
			} else {
				// Unchanged
				false
			}
		} else {
			// New
			true
		}
	}

	/// Invalidate the given nodes.
	/// This will not propagate to dependent nodes automatically! Call [`Self::propagate_invalidations`] afterwards.
	pub fn invalidate(&self, roots: &[EcoString]) {
		let mut metadata = self.metadata.write();

		for id in roots {
			if let Some(node) = metadata.nodes.remove(id.as_str()) {
				log::trace!("Invalidating cached node {} with unit {}", id, node.unit);
				metadata.units.remove(&node.unit);
			}
		}
	}

	/// Propagate any invalidations previously done with [`Self::invalidate`] to nodes depending on now-uncached nodes.
	pub fn propagate_invalidations(&self) -> Vec<String> {
		let mut invalidated_units = vec![];

		let mut metadata = self.metadata.write();

		let mut reverse_dependencies = HashMap::default();

		for (id, node) in &metadata.nodes {
			for dep in &node.deps {
				reverse_dependencies
					.entry(dep.to_owned())
					.or_insert_with(Vec::new)
					.push(id.to_owned());
			}
		}

		fn uncache_reverse_dependencies(
			invalidated_units: &mut Vec<String>,
			metadata: &mut CacheMetadata,
			reverse_dependencies: &mut HashMap<String, Vec<String>>,
			id: String
		) {
			if let Some(reverse_deps) = reverse_dependencies.remove(&id) {
				for id in reverse_deps {
					if let Some(node) = metadata.nodes.remove(&id) {
						log::trace!("Propagated invalidation to node {}, unit {}", id, node.unit);
						metadata.units.remove(&node.unit);
						invalidated_units.push(node.unit);
					}
					uncache_reverse_dependencies(invalidated_units, metadata, reverse_dependencies, id);
				}
			}
		}

		for id in reverse_dependencies
			.keys()
			.filter(|&id| !metadata.nodes.contains_key(id))
			.cloned()
			.collect_vec()
		{
			// Anything that depends on something which is uncached is also uncached
			uncache_reverse_dependencies(&mut invalidated_units, &mut metadata, &mut reverse_dependencies, id);
		}

		invalidated_units
	}

	pub fn get_unit(&self, id: &str) -> Option<CachedUnit> {
		self.metadata.read().units.get(id).cloned()
	}

	pub fn insert_unit(&self, id: String, hash: u64, nodes: Vec<String>) {
		self.metadata
			.write()
			.units
			.insert(id.to_owned(), CachedUnit { hash, nodes });
	}

	pub fn is_unit_cached(&self, unit: &str) -> bool {
		self.metadata.read().units.contains_key(unit)
	}

	pub fn is_node_cached(&self, node: &str) -> bool {
		self.metadata.read().nodes.contains_key(node)
	}

	pub fn get_node(&self, node: &str) -> Option<CachedNode> {
		self.metadata.read().nodes.get(node).cloned()
	}

	#[try_fn]
	#[instrument(skip_all)]
	pub fn insert_node(
		&self,
		node: String,
		unit: String,
		metadata: OperationMetadata,
		deps: Vec<String>,
		mutations: &Vec<Mutation>
	) -> Result<()> {
		let mutations = rkyv::to_bytes::<rancor::BoxedError>(mutations)?;
		let mutations_hash = xxh3_64(&mutations);

		self.blobs.insert(mutations_hash.to_be_bytes(), mutations.as_slice())?;

		self.metadata.write().nodes.insert(
			node.to_owned(),
			CachedNode {
				unit,
				metadata,
				result: mutations_hash,
				deps
			}
		);
	}

	#[try_fn]
	#[instrument(skip_all)]
	pub fn get_node_result(&self, node: &str) -> Result<Option<Vec<Mutation>>> {
		let metadata = self.metadata.read();

		if let Some(info) = metadata.nodes.get(node)
			&& let Some(data) = self.blobs.get(info.result.to_be_bytes())?
		{
			#[expect(
				clippy::unnecessary_to_owned,
				reason = "rkyv requires alignment which is not guaranteed by fjall"
			)]
			let mutations = rkyv::from_bytes::<_, rancor::BoxedError>(&data.to_vec())
				.wrap_err("Couldn't deserialise cached data")?;

			Some(mutations)
		} else {
			None
		}
	}
}
