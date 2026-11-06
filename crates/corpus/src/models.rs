//! Which concrete core models a row runs on.

use gbcli_core::Model;

use crate::manifest::{Family, ModelSpec};

/// One run of a row: the model to build and, when the row names revisions
/// the core lacks, the fact that this model stands in for them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub model: Model,
    /// The revisions the row asked for, when `model` is not one of them.
    pub substitute_for: Option<String>,
}

/// A model's name in the baseline, e.g. `dmg-B`, `cgb-E`.
pub fn model_name(model: Model) -> &'static str {
    match model {
        Model::DmgB => "dmg-B",
        Model::Mgb => "mgb",
        Model::Sgb => "sgb",
        Model::Sgb2 => "sgb2",
        Model::Cgb0 => "cgb-0",
        Model::CgbA => "cgb-A",
        Model::CgbB => "cgb-B",
        Model::CgbC => "cgb-C",
        Model::CgbD => "cgb-D",
        Model::CgbE => "cgb-E",
        Model::Agb => "agb-A",
    }
}

/// The models SameBoy has for each family, by revision letter, default first.
fn revisions(family: Family) -> &'static [(&'static str, Model)] {
    match family {
        Family::Dmg => &[("B", Model::DmgB)],
        Family::Mgb => &[("", Model::Mgb)],
        Family::Sgb => &[("", Model::Sgb)],
        Family::Sgb2 => &[("", Model::Sgb2)],
        Family::Cgb => &[
            ("E", Model::CgbE),
            ("0", Model::Cgb0),
            ("A", Model::CgbA),
            ("B", Model::CgbB),
            ("C", Model::CgbC),
            ("D", Model::CgbD),
        ],
        Family::Agb => &[("A", Model::Agb)],
    }
}

/// The runs for a row: one per listed revision the core has; where it has
/// none of them, one run on the family's default marked as a substitute.
pub fn targets(specs: &[ModelSpec]) -> Vec<Target> {
    let mut out = Vec::new();
    for spec in specs {
        let known = revisions(spec.family);
        match &spec.revisions {
            None => out.push(Target {
                model: known[0].1,
                substitute_for: None,
            }),
            Some(wanted) => {
                let mut matched: Vec<Model> = wanted
                    .iter()
                    .filter_map(|w| known.iter().find(|(r, _)| r == w).map(|&(_, m)| m))
                    .collect();
                matched.sort_by_key(|m| model_name(*m));
                if matched.is_empty() {
                    out.push(Target {
                        model: known[0].1,
                        substitute_for: Some(wanted.join("")),
                    });
                } else {
                    out.extend(matched.into_iter().map(|model| Target {
                        model,
                        substitute_for: None,
                    }));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(family: Family, revisions: Option<&[&str]>) -> ModelSpec {
        ModelSpec {
            family,
            revisions: revisions.map(|r| r.iter().map(|s| s.to_string()).collect()),
        }
    }

    fn plain(model: Model) -> Target {
        Target {
            model,
            substitute_for: None,
        }
    }

    #[test]
    fn a_family_without_revisions_runs_on_its_default() {
        let got = targets(&[spec(Family::Dmg, None), spec(Family::Cgb, None)]);
        assert_eq!(got, vec![plain(Model::DmgB), plain(Model::CgbE)]);
    }

    #[test]
    fn each_listed_revision_the_core_has_is_one_run() {
        let got = targets(&[spec(Family::Cgb, Some(&["B", "C", "E"]))]);
        assert_eq!(
            got,
            vec![plain(Model::CgbB), plain(Model::CgbC), plain(Model::CgbE)]
        );
    }

    #[test]
    fn a_revision_the_core_lacks_runs_the_default_as_a_substitute() {
        let got = targets(&[spec(Family::Dmg, Some(&["C"]))]);
        assert_eq!(
            got,
            vec![Target {
                model: Model::DmgB,
                substitute_for: Some("C".into())
            }]
        );
        assert_eq!(
            targets(&[spec(Family::Dmg, Some(&["A", "B", "C"]))]),
            vec![plain(Model::DmgB)]
        );
    }

    #[test]
    fn every_family_maps_to_a_model() {
        let got = targets(&[
            spec(Family::Mgb, None),
            spec(Family::Sgb, None),
            spec(Family::Sgb2, None),
            spec(Family::Agb, None),
        ]);
        assert_eq!(
            got,
            vec![
                plain(Model::Mgb),
                plain(Model::Sgb),
                plain(Model::Sgb2),
                plain(Model::Agb)
            ]
        );
    }
}
