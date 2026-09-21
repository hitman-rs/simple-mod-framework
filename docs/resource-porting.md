# Resource porting

The Glacier engine uses a system where resources are split across multiple partitions, often aligning with the selectable DLCs (e.g. each map in HITMAN 3 has its own partition). Resources in a partition can only access resources within the same partition, or any parent partitions.

If a resource is referenced which is inaccessible for the partition it is referenced from, SMF will automatically port the resource. This means that SMF will copy the referenced resource into the partition it was referenced from, making it available to your mod. As a result, you generally don't need to worry about partitions.

Sometimes, resources are marked as "deleted" by game patches. The data for these resources still exists in the files, but the resources are no longer in use by the game and are inaccessible by other resources. SMF will not automatically make these available, as often deleted resources are incompatible with the latest game version or have other issues. If you want to directly use a deleted resource as it existed before the update, you can add it to `portResources` manually, and SMF will handle it as normal.
