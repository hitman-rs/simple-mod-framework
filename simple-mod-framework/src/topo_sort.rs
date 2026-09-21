use std::cmp::Reverse;
use std::collections::BinaryHeap;

use color_eyre::eyre::{OptionExt, Result, bail};
use simple_mod_framework_types::HashMap;
use tryvial::try_fn;

/// Stable topological sort using Kahn's algorithm.
/// I have no idea how it works.
#[try_fn]
pub fn topological_sort<T>(items: Vec<(T, Vec<T>)>) -> Result<Vec<T>>
where
	T: Eq + std::hash::Hash + Clone
{
	let mut item_to_index = HashMap::new();
	let mut index_to_item = Vec::new();

	// Check for duplicate items and build mappings
	for (i, (item, _)) in items.iter().enumerate() {
		if item_to_index.contains_key(item) {
			bail!("Duplicate item found");
		}
		item_to_index.insert(item.clone(), i);
		index_to_item.push(item.clone());
	}

	let n = items.len();
	let mut adjacency = vec![vec![]; n];
	let mut in_degree = vec![0; n];

	// Process each item and its dependencies to build the graph
	for (current_item, deps) in items.into_iter() {
		let current_index = item_to_index[&current_item];

		for dep in deps {
			let dep_index = item_to_index.get(&dep).ok_or_eyre("Dependency not found in items")?;

			adjacency[*dep_index].push(current_index);
			in_degree[current_index] += 1;
		}
	}

	// Initialize priority queue with nodes of in_degree 0, ordered by original index
	let mut heap = BinaryHeap::new();
	for (index, &degree) in in_degree.iter().enumerate() {
		if degree == 0 {
			heap.push(Reverse(index));
		}
	}

	let mut result = Vec::new();
	while let Some(Reverse(index)) = heap.pop() {
		result.push(index_to_item[index].clone());

		// Decrement in_degree of neighbors and add to heap if in_degree becomes 0
		for &neighbor in &adjacency[index] {
			in_degree[neighbor] -= 1;
			if in_degree[neighbor] == 0 {
				heap.push(Reverse(neighbor));
			}
		}
	}

	if result.len() != n {
		bail!("Cycle detected")
	} else {
		result
	}
}
