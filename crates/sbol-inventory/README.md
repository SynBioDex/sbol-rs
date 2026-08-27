# sbol-inventory

Native Rust support for SBOLInventory Profile 0.2, layered on `sbol3`.

The crate treats serialized SBOL 3 RDF as the interoperability boundary. It exposes typed, read-only views of facilities, zones, assets, capability offerings, property values, and material lots while preserving the complete underlying `sbol3::Document` and unknown extension triples.

Profile validation, candidate queries, builders, and run-ledger helpers are part of the same crate. SBOLInventory remains a profile rather than a change to the SBOL 3 core object model.
