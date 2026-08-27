//! Generated SBOLInventory Profile 0.2 rule catalog.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConformanceClass {
    Reader,
    Writer,
    Validator,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleStrength {
    Required,
    Recommended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileRule {
    pub id: &'static str,
    pub section: &'static str,
    pub classes: &'static [ConformanceClass],
    pub strength: RuleStrength,
    pub machine_checkable: bool,
    pub shacl_core: bool,
    pub fixture: Option<&'static str>,
    pub statement: &'static str,
}

impl ProfileRule {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: &'static str,
        section: &'static str,
        classes: &'static [ConformanceClass],
        strength: RuleStrength,
        machine_checkable: bool,
        shacl_core: bool,
        fixture: Option<&'static str>,
        statement: &'static str,
    ) -> Self {
        Self {
            id,
            section,
            classes,
            strength,
            machine_checkable,
            shacl_core,
            fixture,
            statement,
        }
    }

    pub fn applies_to(self, class: ConformanceClass) -> bool {
        self.classes.contains(&class)
    }
}

include!(concat!(env!("OUT_DIR"), "/profile_rule_catalog.rs"));

pub fn profile_rules() -> &'static [ProfileRule] {
    PROFILE_RULES
}

pub fn profile_rule(id: &str) -> Option<&'static ProfileRule> {
    PROFILE_RULES.iter().find(|rule| rule.id == id)
}
