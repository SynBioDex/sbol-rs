# Downstream compatibility errata

This directory is based on SBOLInventory commit `aa4d111b5fdd3199066140a4a647885f9a1fb8c0` and carries two explicit corrections required for its valid fixtures to conform to SBOL 3.1.0.

Material lineage uses `fac:derivedFromMaterial` rather than `prov:wasDerivedFrom`. SBOL rules `sbol3-12301` and `sbol3-12302` require every `prov:wasDerivedFrom` value on an `sbol:Implementation` to identify a Component, so an Implementation-to-Implementation lineage edge cannot use that predicate while remaining valid SBOL.

The material-run fixture uses the standard `sbol:hasAttachment` property rather than the undefined `sbol:attachment` spelling.

These corrections do not relax SBOL core validation. They are recorded in `SOURCE.toml` so a future upstream profile revision can replace this downstream patch without ambiguity.
