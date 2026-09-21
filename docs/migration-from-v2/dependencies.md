# Dependencies

SMF now automatically ports any and all resources which are inaccessible from the place they are referenced. As such, SMF's auto-migration **does not** carry over the `dependencies` key into `portResources`, its modern equivalent, as the vast majority of use-cases for `dependencies` are now automatically handled with no need for manual specification.

If you used the `dependencies` key to port deleted resources, or to port resources in a way that affected other game behaviour (e.g. by porting an older version of a resource into a lower chunk), your mod may work differently or not at all on SMFv3. You should add back the minimal amount of these with the new `portResources` key.
