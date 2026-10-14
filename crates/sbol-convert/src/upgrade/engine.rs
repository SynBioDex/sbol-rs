//! The SBOL 2 → SBOL 3 conversion engine: preflight scanning and the
//! per-triple type/predicate rewriting pass. The `Engine` struct and the
//! shared helpers live in the parent module; the emission and synthesis
//! half of the impl is in [`super::emit`].

use super::*;

impl<'a> Engine<'a> {
    pub(super) fn new(input: &'a Graph, options: UpgradeOptions) -> Self {
        let identity = IdentityMap::build(input);
        Self {
            input,
            options,
            identity,
            typed_subjects: HashMap::new(),
            cd_sequences: HashMap::new(),
            location_to_sa: HashMap::new(),
            sa_to_cd: HashMap::new(),
            sa_to_subcomponent: HashMap::new(),
            display_ids: HashMap::new(),
            owned_by: HashMap::new(),
            mapsto_info: HashMap::new(),
            fc_directions: HashMap::new(),
            fc_direction_none: HashSet::new(),
            fc_public_access: HashSet::new(),
            output_triples: Vec::new(),
            namespaced_subjects: HashSet::new(),
            used_iris: HashSet::new(),
            location_display_id_overrides: HashMap::new(),
            generic_top_levels: HashSet::new(),
            report: UpgradeReport::default(),
        }
    }

    pub(super) fn preflight(&mut self) -> Result<(), UpgradeError> {
        // Seed the used-IRI pool with every input subject IRI in its
        // canonical (post-`identity.rewrite_iri`) form. Every synthesis
        // site downstream (SA-collapse Location rewrite,
        // ComponentReference/Constraint allocation for MapsTo
        // decomposition, synthesized Interface IRI) routes its
        // candidate IRI through `next_available_*` against this pool,
        // so no synthesized SBOL 3 IRI silently lands on an existing
        // subject. (Mirrors the same invariant the downgrade engine
        // enforces.)
        //
        // The same pass groups input subjects by canonical IRI so we
        // can surface identity collisions: two distinct input
        // subjects whose version-stripped (or persistentIdentity-keyed)
        // canonical forms coincide. The conversion still runs and the
        // merged subject keeps every input triple, but the result is a
        // chimeric SBOL 3 entity that almost certainly isn't what the
        // author intended; the warning lets callers audit.
        let mut input_subjects: HashSet<String> = HashSet::new();
        for triple in self.input.triples() {
            if let Some(iri) = triple.subject.as_iri() {
                input_subjects.insert(iri.as_str().to_owned());
            }
        }
        let mut canonical_sources: HashMap<String, Vec<String>> = HashMap::new();
        for subject in input_subjects {
            let canonical = self.identity.rewrite_iri(&subject).to_owned();
            self.used_iris.insert(canonical.clone());
            canonical_sources
                .entry(canonical)
                .or_default()
                .push(subject);
        }
        let mut collisions: Vec<(String, Vec<String>)> = canonical_sources
            .into_iter()
            .filter(|(_, sources)| sources.len() > 1)
            .collect();
        collisions.sort_by(|a, b| a.0.cmp(&b.0));
        for (canonical, mut sources) in collisions {
            sources.sort();
            self.report
                .push(UpgradeWarning::IdentityCollision { canonical, sources });
        }
        // A document is SBOL 2 if it carries ANY predicate in the SBOL 2
        // namespace (persistentIdentity / displayId / version / the domain
        // properties), even when no subject's rdf:type is SBOL 2-namespaced.
        // GenericTopLevel (custom rdf:type), PROV-only, and OM-only documents
        // are all valid SBOL 2 with this shape.
        let mut saw_sbol2_predicate = false;
        // Subjects carrying a non-SBOL2/PROV/OM rdf:type together with SBOL 2
        // identity properties, keyed to whether they also carry a recognized
        // SBOL 2 / PROV type (those go through the ordinary top-level path).
        let mut custom_typed: HashSet<String> = HashSet::new();
        let mut recognized_typed: HashSet<String> = HashSet::new();
        let mut sbol2_identified: HashSet<String> = HashSet::new();

        for triple in self.input.triples() {
            let predicate = triple.predicate.as_str();

            if predicate.starts_with(v2::SBOL2_NS) {
                saw_sbol2_predicate = true;
                if (predicate == v2::SBOL2_PERSISTENT_IDENTITY || predicate == v2::SBOL2_DISPLAY_ID)
                    && let Some(subject_iri) = triple.subject.as_iri()
                {
                    sbol2_identified.insert(subject_iri.as_str().to_owned());
                }
            }

            if predicate == v3::RDF_TYPE {
                if let Some(iri) = triple.object.as_iri()
                    && let Some(subject_iri) = triple.subject.as_iri()
                {
                    let object = iri.as_str();
                    let subject = subject_iri.as_str().to_owned();

                    // Split subjects by whether their rdf:type is a
                    // recognized SBOL 2 / PROV / OM class. A subject with
                    // only a custom (foreign-namespace) type is a
                    // GenericTopLevel candidate.
                    if object.starts_with(v2::SBOL2_NS)
                        || object.starts_with(v3::PROV_NS)
                        || object.starts_with(v3::OM_NS)
                    {
                        recognized_typed.insert(subject.clone());
                    } else {
                        custom_typed.insert(subject.clone());
                    }

                    // Capture SBOL 2 types directly, plus the PROV top-level
                    // classes (Activity / Agent / Plan) that SBOL 2 documents
                    // commonly carry alongside SBOL 2 typed objects (e.g.
                    // SynBioHub's igem2sbol conversion provenance). PROV
                    // top-levels are normative top-levels in SBOL 3 too and
                    // need `hasNamespace` synthesized.
                    if object.starts_with(v2::SBOL2_NS)
                        || object == v3::PROV_ACTIVITY
                        || object == v3::PROV_AGENT_CLASS
                        || object == v3::PROV_PLAN
                    {
                        let should_replace =
                            self.typed_subjects.get(&subject).is_none_or(|existing| {
                                type_precedence(object) > type_precedence(existing)
                            });
                        if should_replace {
                            self.typed_subjects.insert(subject, object.to_owned());
                        }
                    }
                }
                continue;
            }

            let subject = match triple.subject.as_iri() {
                Some(iri) => iri.as_str().to_owned(),
                None => continue,
            };

            if predicate == v2::SBOL2_DISPLAY_ID
                && let Some(lit) = triple.object.as_literal()
            {
                self.display_ids
                    .entry(subject.clone())
                    .or_insert_with(|| lit.value().to_owned());
            }

            let object = match triple.object.as_iri() {
                Some(iri) => iri.as_str().to_owned(),
                None => continue,
            };

            match predicate {
                v2::SBOL2_SEQUENCE_PROP => {
                    self.cd_sequences
                        .entry(subject.clone())
                        .or_default()
                        .push(object.clone());
                }
                v2::SBOL2_SEQUENCE_ANNOTATION_PROP => {
                    self.sa_to_cd.insert(object.clone(), subject.clone());
                    self.owned_by.insert(object, subject);
                }
                v2::SBOL2_LOCATION_PROP => {
                    self.location_to_sa.insert(object.clone(), subject.clone());
                    self.owned_by.insert(object, subject);
                }
                v2::SBOL2_COMPONENT_PROP
                | v2::SBOL2_FUNCTIONAL_COMPONENT_PROP
                | v2::SBOL2_MODULE_PROP
                | v2::SBOL2_INTERACTION_PROP
                | v2::SBOL2_PARTICIPATION_PROP
                | v2::SBOL2_MAPS_TO_PROP
                | v2::SBOL2_VARIABLE_COMPONENT_PROP => {
                    self.owned_by.insert(object, subject);
                }
                _ => {}
            }
        }

        // Reject only when the document has NEITHER an SBOL 2 predicate NOR a
        // recognized SBOL 2 / PROV / OM rdf:type. Genuine SBOL 3 and non-SBOL
        // input carry no SBOL 2 predicates and land here.
        // An empty graph is trivially a valid (empty) SBOL 2 document, and a
        // valid empty SBOL 3 document, so it upgrades to empty rather than
        // being rejected. Only a graph that carries triples yet none of the
        // SBOL 2 markers (i.e. genuine SBOL 3 or non-SBOL input) is rejected.
        let has_recognized_type = self
            .typed_subjects
            .values()
            .any(|t| t.starts_with(v2::SBOL2_NS) || is_prov_top_level(t))
            || !recognized_typed.is_empty();
        let is_empty = self.input.triples().is_empty();
        if !is_empty && !saw_sbol2_predicate && !has_recognized_type {
            return Err(UpgradeError::NotSbol2);
        }

        // Custom-typed subjects that carry SBOL 2 identity properties but no
        // recognized SBOL 2 / PROV / OM type are GenericTopLevels: SBOL 3 has
        // no GenericTopLevel class, so the custom rdf:type is retained and the
        // subject is treated as an SBOL 3 top-level (it needs `hasNamespace`).
        // A subject nested under another identified subject is a child, not a
        // top-level, so it is excluded.
        let identity_prefixes: HashSet<String> = sbol2_identified
            .iter()
            .map(|iri| format!("{}/", self.identity.rewrite_iri(iri)))
            .collect();
        for subject in &sbol2_identified {
            if !custom_typed.contains(subject) || recognized_typed.contains(subject) {
                continue;
            }
            let canonical = self.identity.rewrite_iri(subject).to_owned();
            // A subject never starts with its own `{iri}/` prefix, so this
            // flags only subjects genuinely nested under a different one.
            let is_child = identity_prefixes
                .iter()
                .any(|prefix| canonical.starts_with(prefix.as_str()));
            if !is_child {
                self.generic_top_levels.insert(canonical);
            }
        }

        // Second pass to resolve SA-with-component: `sbol2:component` is
        // overloaded across CDs and SAs, so we need the rdf:type map from
        // the first pass to know which subjects are SAs.
        for triple in self.input.triples() {
            if triple.predicate.as_str() != v2::SBOL2_COMPONENT_PROP {
                continue;
            }
            let subject = match triple.subject.as_iri() {
                Some(iri) => iri.as_str().to_owned(),
                None => continue,
            };
            if self.typed_subjects.get(&subject).map(String::as_str)
                != Some(v2::SBOL2_SEQUENCE_ANNOTATION)
            {
                continue;
            }
            let target = match triple.object.as_iri() {
                Some(iri) => iri.as_str().to_owned(),
                None => continue,
            };
            self.sa_to_subcomponent.insert(subject, target);
        }

        // Locations under collapsed SAs must migrate to be children of their
        // new SubComponent parent so SBOL 3 IRI compliance (sbol3-10104)
        // holds. The new IRI is `{SubComponent}/{displayId or last segment}`.
        //
        // Allocation routes through `next_available_child_iri` against
        // `self.used_iris` so two SAs both pointing at the same
        // SubComponent and carrying Locations of the same displayId
        // produce DISTINCT new Location IRIs (one disambiguated with a
        // `_N` suffix) rather than silently merging both Locations
        // onto a single subject. When disambiguation fires the
        // emitted `sbol3:displayId` value must also pick up the
        // suffix so sbol3-10204 still holds. That override is
        // recorded in `location_display_id_overrides` and consulted by
        // `handle_sbol2_predicate` when it emits the rewritten
        // `sbol3:displayId` triple.
        //
        // Sorted to keep the disambiguation index deterministic across
        // runs (HashMap iteration order is unstable).
        let mut location_owners: Vec<(String, String)> = self
            .location_to_sa
            .iter()
            .filter_map(|(loc, sa)| {
                self.sa_to_subcomponent
                    .get(sa)
                    .map(|sc| (loc.clone(), sc.clone()))
            })
            .collect();
        location_owners.sort();
        for (loc_iri, subcomponent_iri) in location_owners {
            let subcomponent_canonical = self.identity.rewrite_iri(&subcomponent_iri).to_owned();
            let base_display_id = self.display_ids.get(&loc_iri).cloned().unwrap_or_else(|| {
                last_path_segment(self.identity.rewrite_iri(&loc_iri)).to_owned()
            });
            // Release the Location's existing canonical IRI from the
            // pool before allocating. We're about to re-map this
            // identity onto its new SubComponent-relative slot, so its
            // old slot no longer represents a distinct subject. Without
            // this, an upgrade-downgrade-upgrade round-trip where the
            // downgrade emitted the Location at its post-collapse IRI
            // (because that's what `backport:sbol2persistentIdentity`
            // preserved) would see the Location's own IRI in
            // `used_iris` and disambiguate the re-collapse target to
            // `_2`, drifting the round-trip.
            let loc_canonical = self.identity.rewrite_iri(&loc_iri).to_owned();
            self.used_iris.remove(&loc_canonical);
            let (chosen_display_id, new_iri) = next_available_child_iri(
                &subcomponent_canonical,
                &base_display_id,
                &mut self.used_iris,
            );
            if chosen_display_id != base_display_id {
                self.location_display_id_overrides
                    .insert(loc_iri.clone(), chosen_display_id);
            }
            self.identity.add_rewrite(loc_iri, new_iri);
        }

        // Capture FunctionalComponent directions so we can synthesize an
        // SBOL 3 Interface per Component. Per SBOL 3.1.0 section 10.2,
        // public SBOL 2 FunctionalComponents with direction none also belong
        // in the SBOL 3 Interface as nondirectional features; we collect the
        // two required signals in separate scans and merge them below.
        for triple in self.input.triples() {
            if triple.predicate.as_str() != v2::SBOL2_DIRECTION {
                continue;
            }
            let subject = match triple.subject.as_iri() {
                Some(iri) => iri.as_str(),
                None => continue,
            };
            if self.typed_subjects.get(subject).map(String::as_str)
                != Some(v2::SBOL2_FUNCTIONAL_COMPONENT)
            {
                continue;
            }
            let dir_iri = match triple.object.as_iri() {
                Some(iri) => iri.as_str(),
                None => continue,
            };
            let direction = match dir_iri {
                v2::SBOL2_DIRECTION_IN => FcDirection::Input,
                v2::SBOL2_DIRECTION_OUT => FcDirection::Output,
                v2::SBOL2_DIRECTION_INOUT => FcDirection::Inout,
                v2::SBOL2_DIRECTION_NONE => {
                    self.fc_direction_none.insert(subject.to_owned());
                    continue;
                }
                _ => continue,
            };
            self.fc_directions.insert(subject.to_owned(), direction);
        }
        for triple in self.input.triples() {
            if triple.predicate.as_str() != v2::SBOL2_ACCESS {
                continue;
            }
            let subject = match triple.subject.as_iri() {
                Some(iri) => iri.as_str(),
                None => continue,
            };
            if triple.object.as_iri().map(|i| i.as_str()) != Some(v2::SBOL2_ACCESS_PUBLIC) {
                continue;
            }
            match self.typed_subjects.get(subject).map(String::as_str) {
                Some(v2::SBOL2_FUNCTIONAL_COMPONENT) => {
                    self.fc_public_access.insert(subject.to_owned());
                }
                Some(v2::SBOL2_COMPONENT) => {
                    self.fc_directions
                        .entry(subject.to_owned())
                        .or_insert(FcDirection::Inout);
                }
                _ => {}
            }
        }
        for fc_iri in self
            .fc_direction_none
            .intersection(&self.fc_public_access)
            .cloned()
            .collect::<Vec<_>>()
        {
            self.fc_directions
                .entry(fc_iri)
                .or_insert(FcDirection::Inout);
        }

        // Third pass to populate per-MapsTo details (local / remote /
        // refinement / displayId), keyed by the original MapsTo IRI.
        for triple in self.input.triples() {
            let subject = match triple.subject.as_iri() {
                Some(iri) => iri.as_str(),
                None => continue,
            };
            if self.typed_subjects.get(subject).map(String::as_str) != Some(v2::SBOL2_MAPS_TO) {
                continue;
            }
            let info = self.mapsto_info.entry(subject.to_owned()).or_default();
            match triple.predicate.as_str() {
                v2::SBOL2_LOCAL => {
                    if let Some(iri) = triple.object.as_iri() {
                        info.local = Some(iri.as_str().to_owned());
                    }
                }
                v2::SBOL2_REMOTE => {
                    if let Some(iri) = triple.object.as_iri() {
                        info.remote = Some(iri.as_str().to_owned());
                    }
                }
                v2::SBOL2_REFINEMENT => {
                    if let Some(iri) = triple.object.as_iri() {
                        info.refinement = Some(iri.as_str().to_owned());
                    }
                }
                v2::SBOL2_DISPLAY_ID => {
                    if let Some(lit) = triple.object.as_literal() {
                        info.display_id = Some(lit.value().to_owned());
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Walks the `owned_by` chain up from `subject` until reaching a
    /// top-level subject (one with no owner). Returns the top-level subject's
    /// IRI string, or `subject` itself if it is already top-level.
    pub(super) fn owning_top_level(&self, subject: &str) -> String {
        let mut current = subject.to_owned();
        while let Some(parent) = self.owned_by.get(&current) {
            current = parent.clone();
        }
        current
    }

    pub(super) fn convert(&mut self) {
        // Iterate by index to avoid cloning the entire triple slice.
        // `handle_triple` only reads its argument; the &mut self is
        // needed for `self.output_triples` and the report. The input
        // slice is bounded by the loop's stored length, so growing
        // `self.output_triples` mid-loop is safe.
        let n = self.input.triples().len();
        for i in 0..n {
            let triple = self.input.triples()[i].clone();
            self.handle_triple(&triple);
        }
        self.emit_synthesized_triples();
    }

    pub(super) fn handle_triple(&mut self, triple: &Triple) {
        let predicate = triple.predicate.as_str();

        // Metadata on a collapsed SA cannot pass through on the SA
        // subject because the SA shell no longer exists in SBOL 3. Archive
        // it on the replacement SubComponent under encoded backport
        // predicates so the downgrade can rebuild the original SA wrapper.
        if !predicate.starts_with(v2::SBOL2_NS)
            && let Some(subject_iri) = triple.subject.as_iri()
            && let Some(target) = self.sa_to_subcomponent.get(subject_iri.as_str()).cloned()
            && (predicate != v3::RDF_TYPE
                || triple.object.as_iri().map(|i| i.as_str())
                    != Some(v2::SBOL2_SEQUENCE_ANNOTATION))
        {
            self.preserve_collapsed_sa_metadata(triple, &target);
            return;
        }

        if predicate == v3::RDF_TYPE {
            self.handle_type_triple(triple);
            return;
        }

        // Predicates in the SBOL 2 namespace get rewritten or dropped.
        if predicate.starts_with(v2::SBOL2_NS) {
            self.handle_sbol2_predicate(triple);
            return;
        }

        // dcterms:title / dcterms:description are the SBOL 2 spelling of
        // name / description. The reference converts them to sbol3:name /
        // sbol3:description and drops the dcterms form.
        if predicate == v2::DCTERMS_TITLE || predicate == v2::DCTERMS_DESCRIPTION {
            let target = if predicate == v2::DCTERMS_TITLE {
                v3::SBOL_NAME
            } else {
                v3::SBOL_DESCRIPTION
            };
            let rewritten = self.identity.rewrite_triple(triple);
            self.output_triples.push(Triple {
                subject: rewritten.subject,
                predicate: Iri::from_static(target),
                object: rewritten.object,
            });
            return;
        }

        // Everything else (PROV, custom annotations) passes through with
        // identity rewriting on subject and object.
        self.output_triples
            .push(self.identity.rewrite_triple(triple));
    }

    pub(super) fn handle_type_triple(&mut self, triple: &Triple) {
        let object_iri = match triple.object.as_iri() {
            Some(iri) => iri.as_str(),
            None => {
                self.output_triples
                    .push(self.identity.rewrite_triple(triple));
                return;
            }
        };

        // SAs that reference a SubComponent collapse into that SubComponent
        // (their locations migrate via the location-rewrite path). Suppress
        // the SequenceFeature type triple entirely.
        if object_iri == v2::SBOL2_SEQUENCE_ANNOTATION
            && let Some(subject_iri) = triple.subject.as_iri()
            && self.sa_to_subcomponent.contains_key(subject_iri.as_str())
        {
            self.report
                .push(UpgradeWarning::SequenceAnnotationWithComponent {
                    annotation: subject_iri.as_str().to_owned(),
                });
            self.report.counts.sequence_annotations_collapsed += 1;
            return;
        }

        // MapsTo is decomposed into ComponentReference + Constraint at the
        // emit-synthesized-triples stage. Suppress the original type triple.
        if object_iri == v2::SBOL2_MAPS_TO {
            return;
        }

        let subject = self.identity.rewrite_resource(&triple.subject);
        let original_iri = triple.subject.as_iri().map(|iri| iri.as_str().to_owned());

        let v3_type: Option<&'static str> = match object_iri {
            v2::SBOL2_COMPONENT_DEFINITION | v2::SBOL2_MODULE_DEFINITION => {
                Some(v3::SBOL_COMPONENT_CLASS)
            }
            v2::SBOL2_COMPONENT | v2::SBOL2_MODULE | v2::SBOL2_FUNCTIONAL_COMPONENT => {
                Some(v3::SBOL_SUB_COMPONENT_CLASS)
            }
            v2::SBOL2_SEQUENCE_ANNOTATION => Some(v3::SBOL_SEQUENCE_FEATURE_CLASS),
            v2::SBOL2_SEQUENCE_CONSTRAINT => Some(v3::SBOL_CONSTRAINT_CLASS),
            v2::SBOL2_SEQUENCE => Some(v3::SBOL_SEQUENCE_CLASS),
            v2::SBOL2_MODEL => Some(v3::SBOL_MODEL_CLASS),
            v2::SBOL2_INTERACTION => Some(v3::SBOL_INTERACTION_CLASS),
            v2::SBOL2_PARTICIPATION => Some(v3::SBOL_PARTICIPATION_CLASS),
            v2::SBOL2_COLLECTION => Some(v3::SBOL_COLLECTION_CLASS),
            v2::SBOL2_IMPLEMENTATION => Some(v3::SBOL_IMPLEMENTATION_CLASS),
            v2::SBOL2_ATTACHMENT => Some(v3::SBOL_ATTACHMENT_CLASS),
            v2::SBOL2_EXPERIMENT => Some(v3::SBOL_EXPERIMENT_CLASS),
            v2::SBOL2_EXPERIMENTAL_DATA => Some(v3::SBOL_EXPERIMENTAL_DATA_CLASS),
            v2::SBOL2_COMBINATORIAL_DERIVATION => Some(v3::SBOL_COMBINATORIAL_DERIVATION_CLASS),
            v2::SBOL2_VARIABLE_COMPONENT => Some(v3::SBOL_VARIABLE_FEATURE_CLASS),
            v2::SBOL2_RANGE => Some(v3::SBOL_RANGE_CLASS),
            v2::SBOL2_CUT => Some(v3::SBOL_CUT_CLASS),
            v2::SBOL2_GENERIC_LOCATION => Some(v3::SBOL_LOCATION_CLASS),
            other if other.starts_with(v2::SBOL2_NS) => {
                if let Some(iri) = triple.subject.as_iri() {
                    self.report.push(UpgradeWarning::UnknownSbol2Type {
                        subject: iri.as_str().to_owned(),
                        sbol2_type: other.to_owned(),
                    });
                }
                return;
            }
            _ => {
                // Non-SBOL2 type (e.g. BioPAX or custom): keep, possibly
                // rewriting BioPAX → SBO terms.
                let mapped = values::map_biopax_type(object_iri).unwrap_or(object_iri);
                self.output_triples.push(Triple {
                    subject,
                    predicate: triple.predicate.clone(),
                    object: Term::Resource(Resource::Iri(Iri::new_unchecked(mapped))),
                });
                return;
            }
        };

        if let Some(target) = v3_type {
            self.output_triples.push(Triple {
                subject: subject.clone(),
                predicate: triple.predicate.clone(),
                object: Term::Resource(Resource::Iri(Iri::new_unchecked(target))),
            });

            match object_iri {
                v2::SBOL2_COMPONENT_DEFINITION => {
                    self.report.counts.component_definitions += 1;
                }
                v2::SBOL2_MODULE_DEFINITION => {
                    self.report.counts.module_definitions += 1;
                    // SBOL 2 ModuleDefinitions don't carry a `type`, but
                    // the SBOL 3 Component they become requires at least
                    // one. Inject `SBO:functionalEntity` so the converted
                    // document satisfies sbol3-10110 cardinality without
                    // overriding any types the user explicitly authored.
                    self.output_triples.push(Triple {
                        subject: subject.clone(),
                        predicate: Iri::from_static(v3::SBOL_TYPE),
                        object: Term::Resource(Resource::Iri(Iri::new_unchecked(
                            "https://identifiers.org/SBO:0000241",
                        ))),
                    });
                }
                v2::SBOL2_MODULE => {
                    self.report.counts.sub_components += 1;
                    // A SubComponent derived from an SBOL 2 Module carries a
                    // marker so the downgrade restores it as a Module rather
                    // than a FunctionalComponent.
                    if self.options.preserve_backport {
                        self.output_triples.push(Triple {
                            subject: subject.clone(),
                            predicate: Iri::from_static(v2::BACKPORT_SBOL2_ORIGINATES_FROM_MODULE),
                            object: Term::Literal(sbol_rdf::Literal::simple("true")),
                        });
                    }
                }
                v2::SBOL2_COMPONENT | v2::SBOL2_FUNCTIONAL_COMPONENT => {
                    self.report.counts.sub_components += 1;
                }
                v2::SBOL2_SEQUENCE_ANNOTATION => {
                    // SAs that get *collapsed* never reach this branch (we
                    // short-circuited at the top of handle_type_triple), so
                    // any SA here is becoming a standalone SequenceFeature.
                    self.report.counts.sequence_features += 1;
                }
                _ => {}
            }

            // Stamp the original SBOL 2 identity on every converted entity so
            // external tooling can trace it back to its source object. A
            // SubComponent derived from an SBOL 2 Module is the exception: the
            // reference converter copies no identified metadata onto it, so it
            // carries no `sbol2OriginalURI`.
            if self.options.preserve_backport
                && object_iri != v2::SBOL2_MODULE
                && let Some(original) = original_iri.as_ref()
            {
                self.output_triples.push(Triple {
                    subject,
                    predicate: Iri::from_static(v2::BACKPORT_SBOL2_ORIGINAL_URI),
                    object: Term::Resource(Resource::Iri(Iri::new_unchecked(original.clone()))),
                });
            }
        }
    }

    pub(super) fn handle_sbol2_predicate(&mut self, triple: &Triple) {
        let predicate = triple.predicate.as_str();

        if self.try_collapse_sa_triple(triple) {
            return;
        }

        // Drop the parent's `sbol2:mapsTo` triple. The MapsTo shell is
        // replaced by ComponentReference + Constraint in
        // emit_synthesized_triples().
        if predicate == v2::SBOL2_MAPS_TO_PROP
            && let Some(target_iri) = triple.object.as_iri()
            && self.mapsto_info.contains_key(target_iri.as_str())
        {
            return;
        }

        // Drop every triple whose subject is a MapsTo. The decomposition
        // emits the equivalent SBOL 3 triples explicitly.
        if let Some(subject_iri) = triple.subject.as_iri()
            && self.mapsto_info.contains_key(subject_iri.as_str())
        {
            return;
        }

        let subject = self.identity.rewrite_resource(&triple.subject);
        let object = self.identity.rewrite_term(&triple.object);
        // The original SBOL 2 subject IRI keys `typed_subjects`, which
        // disambiguates ontology conversion for shared predicates.
        let subject_key = triple
            .subject
            .as_iri()
            .map(|iri| iri.as_str().to_owned())
            .unwrap_or_default();

        // Single-predicate rename cases.
        let renamed: Option<&'static str> = match predicate {
            v2::SBOL2_DISPLAY_ID => Some(v3::SBOL_DISPLAY_ID),
            v2::SBOL2_BUILT => Some(v3::SBOL_BUILT),
            v2::SBOL2_TYPE => Some(v3::SBOL_TYPE),
            v2::SBOL2_ROLE => Some(v3::SBOL_ROLE),
            v2::SBOL2_ROLE_INTEGRATION => Some(v3::SBOL_ROLE_INTEGRATION),
            v2::SBOL2_ELEMENTS => Some(v3::SBOL_ELEMENTS),
            v2::SBOL2_SOURCE => Some(v3::SBOL_SOURCE),
            v2::SBOL2_FORMAT => Some(v3::SBOL_FORMAT),
            v2::SBOL2_SIZE => Some(v3::SBOL_SIZE),
            v2::SBOL2_HASH => Some(v3::SBOL_HASH),
            v2::SBOL2_HASH_ALGORITHM => Some(v3::SBOL_HASH_ALGORITHM),
            v2::SBOL2_LANGUAGE => Some(v3::SBOL_LANGUAGE),
            v2::SBOL2_FRAMEWORK => Some(v3::SBOL_FRAMEWORK),
            v2::SBOL2_START => Some(v3::SBOL_START),
            v2::SBOL2_END => Some(v3::SBOL_END),
            v2::SBOL2_AT => Some(v3::SBOL_AT),
            v2::SBOL2_SEQUENCE_PROP => Some(v3::SBOL_HAS_SEQUENCE),
            v2::SBOL2_SEQUENCE_ANNOTATION_PROP => Some(v3::SBOL_HAS_FEATURE),
            v2::SBOL2_SEQUENCE_CONSTRAINT_PROP => Some(v3::SBOL_HAS_CONSTRAINT),
            v2::SBOL2_COMPONENT_PROP => Some(v3::SBOL_HAS_FEATURE),
            v2::SBOL2_FUNCTIONAL_COMPONENT_PROP => Some(v3::SBOL_HAS_FEATURE),
            v2::SBOL2_MODULE_PROP => Some(v3::SBOL_HAS_FEATURE),
            v2::SBOL2_INTERACTION_PROP => Some(v3::SBOL_HAS_INTERACTION),
            v2::SBOL2_PARTICIPATION_PROP => Some(v3::SBOL_HAS_PARTICIPATION),
            v2::SBOL2_LOCATION_PROP => Some(v3::SBOL_HAS_LOCATION),
            v2::SBOL2_DEFINITION => Some(v3::SBOL_INSTANCE_OF),
            v2::SBOL2_VARIABLE_COMPONENT_PROP => Some(v3::SBOL_HAS_VARIABLE_FEATURE),
            v2::SBOL2_OPERATOR => Some(v3::SBOL_CARDINALITY),
            v2::SBOL2_VARIABLE => Some(v3::SBOL_VARIABLE),
            v2::SBOL2_VARIANT => Some(v3::SBOL_VARIANT),
            v2::SBOL2_VARIANT_COLLECTION => Some(v3::SBOL_VARIANT_COLLECTION),
            v2::SBOL2_VARIANT_DERIVATION => Some(v3::SBOL_VARIANT_DERIVATION),
            v2::SBOL2_MODEL_PROP => Some(v3::SBOL_HAS_MODEL),
            v2::SBOL2_ATTACHMENT_PROP => Some(v3::SBOL_HAS_ATTACHMENT),
            v2::SBOL2_RESTRICTION => Some(v3::SBOL_RESTRICTION),
            v2::SBOL2_SUBJECT => Some(v3::SBOL_SUBJECT),
            v2::SBOL2_OBJECT => Some(v3::SBOL_OBJECT),
            v2::SBOL2_PARTICIPANT => Some(v3::SBOL_PARTICIPANT),
            v2::SBOL2_STRATEGY => Some(v3::SBOL_STRATEGY),
            v2::SBOL2_TEMPLATE => Some(v3::SBOL_TEMPLATE),
            v2::SBOL2_MEMBER => Some(v3::SBOL_MEMBER),
            v2::SBOL2_EXPERIMENTAL_DATA_PROP => Some(v3::SBOL_MEMBER),
            _ => None,
        };

        if let Some(target) = renamed {
            let mut object_with_value_rewrites =
                self.rewrite_value(&subject_key, triple.predicate.as_str(), &object);

            // When a Location's IRI got disambiguated during the
            // SA-collapse rewrite, override the displayId literal so it
            // matches the new IRI's last segment. Otherwise the
            // rewritten Location would emit `<...range1_2> sbol3:displayId
            // "range1"` and fail sbol3-10204.
            if predicate == v2::SBOL2_DISPLAY_ID
                && let Some(subject_iri) = triple.subject.as_iri()
                && let Some(override_did) =
                    self.location_display_id_overrides.get(subject_iri.as_str())
            {
                object_with_value_rewrites =
                    Term::Literal(sbol_rdf::Literal::simple(override_did.clone()));
            }

            // Location and Cut positions are written as plain literals, as the
            // SBOL-Converter reference writes them. The SBOL 2 input may carry
            // them typed (`xsd:integer`, which `sbol2::Document` assigns while
            // reading) or untyped (raw RDF/XML); either way the SBOL 3 output
            // is the same.
            if matches!(predicate, v2::SBOL2_START | v2::SBOL2_END | v2::SBOL2_AT)
                && let Term::Literal(literal) = &object_with_value_rewrites
            {
                object_with_value_rewrites =
                    Term::Literal(sbol_rdf::Literal::simple(literal.value().to_owned()));
            }

            self.output_triples.push(Triple {
                subject,
                predicate: Iri::from_static(target),
                object: object_with_value_rewrites,
            });
            return;
        }

        // Encoding gets a value rewrite as part of the predicate rewrite.
        if predicate == v2::SBOL2_ENCODING {
            let rewritten = self.rewrite_value(&subject_key, predicate, &object);
            self.output_triples.push(Triple {
                subject,
                predicate: Iri::from_static(v3::SBOL_ENCODING),
                object: rewritten,
            });
            return;
        }

        // Orientation is renamed and value-rewritten.
        if predicate == v2::SBOL2_ORIENTATION {
            let rewritten = self.rewrite_value(&subject_key, predicate, &object);
            self.output_triples.push(Triple {
                subject,
                predicate: Iri::from_static(v3::SBOL_ORIENTATION),
                object: rewritten,
            });
            return;
        }

        // Every SBOL 2 predicate reaching this point has no SBOL 3 rename and
        // no structural successor: `persistentIdentity` and `version` (both
        // consumed by identity derivation), plus any unmapped SBOL 2 predicate
        // such as `access` or `direction`. These are dropped — SBOL 2-only
        // data with no SBOL 3 home is not carried across. `subject` and
        // `object` are computed for the rewrite paths above; here they simply
        // go unused.
        let _ = (subject, object);
    }
}
