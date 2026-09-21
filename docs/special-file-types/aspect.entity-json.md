# aspect.entity.json

An aspect entity in JSON format:

```json
{
    "factory": "[assembly:/templates/aspectdummy.aspect]([assembly:/_glacier/geometry/mockup.wl2?/mockup_box_1x1x1_a.prim].entitytype,[modules:/zshadowlodaspect.class].entitytype).pc_entitytype",
    "blueprint": "[assembly:/templates/aspectdummy.aspect]([assembly:/templates/geometrytemplatestaticcoll.template?/geomentity01.entitytemplate].entitytype,[modules:/zshadowlodaspect.class].entitytype).pc_entityblueprint",
    "resources": [
        {
            "factory": "[assembly:/_glacier/geometry/mockup.wl2?/mockup_box_1x1x1_a.prim].pc_entitytype",
            "blueprint": "[assembly:/templates/geometrytemplatestaticcoll.template?/geomentity01.entitytemplate].pc_entityblueprint"
        },
        {
            "factory": "[modules:/zshadowlodaspect.class].pc_entitytype",
            "blueprint": "[modules:/zshadowlodaspect.class].pc_entityblueprint"
        }
    ]
}
```