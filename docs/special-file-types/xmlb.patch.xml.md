# XMLB.patch.xml

An [RFC7351](https://pike.lysator.liu.se/docs/ietf/rfc/73/rfc7351.xml) format XML patch for an XMLB resource, with the resource to patch specified by a `target` attribute. For example:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<p:patch xmlns:p="urn:ietf:rfc:7351" target="[assembly:/_knt/online/default/cloudstorage/resources/menusystem/aurora/global_components.xml].bechaml">
    <p:replace sel="doc/note/text()">Patched doc</p:replace>
    <p:remove sel="*/elem[@a='bar']/y:child" ws="both"/>
    <p:add sel="*/elem[@a='bar']" type="@b">new attr</p:add>
</p:patch>
```
