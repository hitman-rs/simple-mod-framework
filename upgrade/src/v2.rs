pub mod manifest {
	typify::import_types!(schema = "src/v2-manifest.json", map_type = indexmap::IndexMap);
}

pub mod material {
	typify::import_types!(schema = "src/rpkg-material.json", map_type = indexmap::IndexMap);
}
