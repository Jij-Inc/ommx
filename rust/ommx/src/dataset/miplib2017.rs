use crate::{
    artifact::{ghcr, Artifact, InstanceAnnotations},
    v1::Instance,
};

use anyhow::{ensure, Result};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// CSV downloaded from [MIPLIB website](https://miplib.zib.de/tag_collection.html)
pub const MIPLIB2017_CSV: &str = include_str!("miplib2017.csv");

#[derive(Debug)]
enum ObjectiveValue {
    Infeasible,
    NotAvailable,
    Unbounded,
    Feasible(f64),
}

impl ObjectiveValue {
    fn try_to_string(&self) -> Option<String> {
        match self {
            ObjectiveValue::Infeasible => Some("Infeasible".to_string()),
            ObjectiveValue::NotAvailable => None,
            ObjectiveValue::Unbounded => Some("Unbounded".to_string()),
            ObjectiveValue::Feasible(value) => Some(value.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for ObjectiveValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let s = s.trim_end_matches("*");
        match s {
            "Infeasible" => Ok(ObjectiveValue::Infeasible),
            "NA" => Ok(ObjectiveValue::NotAvailable),
            "Unbounded" => Ok(ObjectiveValue::Unbounded),
            _ => s
                .parse::<f64>()
                .map(ObjectiveValue::Feasible)
                .map_err(serde::de::Error::custom),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(rename = "InstanceInst.")]
    instance: String,
    #[serde(rename = "StatusStat.")]
    status: String,
    #[serde(rename = "VariablesVari.")]
    variable: f64,
    #[serde(rename = "BinariesBina.")]
    binaries: f64,
    #[serde(rename = "IntegersInte.")]
    integers: f64,
    #[serde(rename = "ContinuousCont.")]
    continuous: f64,
    #[serde(rename = "ConstraintsCons.")]
    constraints: f64,
    #[serde(rename = "Nonz.Nonz.")]
    non_zero: f64,
    #[serde(rename = "SubmitterSubm.")]
    submitter: String,
    #[serde(rename = "GroupGrou.")]
    group: String,
    #[serde(rename = "ObjectiveObje.")]
    objective: ObjectiveValue,
    #[serde(rename = "TagsTags.")]
    tags: String,
}

impl RawEntry {
    fn as_annotation(&self) -> InstanceAnnotations {
        let mut annotation = InstanceAnnotations::default();
        annotation.set_title(self.instance.clone());
        annotation.set_created_now();
        annotation.set_authors(
            self.submitter
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        );
        if self.submitter.contains("Berk Ustun") {
            // Berk Ustun's submissions are licensed under "BSD", we assume it is "BSD-3-Clause"
            // https://git.zib.de/miplib2017/submissions/-/blob/master/Berk_Ustun/meta.yml?ref_type=heads
            annotation.set_license("BSD-3-Clause".to_string());
        } else {
            // Other submissions are licensed under the default MIPLIB license "CC-BY-SA-4.0"
            annotation.set_license("CC-BY-SA-4.0".to_string());
        }
        annotation.set_dataset("MIPLIB2017".to_string());
        annotation.set_variables(self.variable as usize);
        annotation.set_constraints(self.constraints as usize);

        // MIPLIB specific annotations
        for (key, value) in [
            ("binaries", self.binaries as usize),
            ("integers", self.integers as usize),
            ("continuous", self.continuous as usize),
            ("non_zero", self.non_zero as usize),
        ] {
            annotation.set_other(format!("org.ommx.miplib.{key}"), value.to_string());
        }
        annotation.set_other(
            "org.ommx.miplib.status".to_string(),
            self.status.to_string(),
        );
        if self.group != "-" {
            annotation.set_other("org.ommx.miplib.group".to_string(), self.group.to_string());
        }
        if let Some(objective) = self.objective.try_to_string() {
            annotation.set_other("org.ommx.miplib.objective".to_string(), objective);
        }
        let tags: Vec<_> = self.tags.split(' ').map(str::trim).collect();
        if !tags.is_empty() {
            annotation.set_other("org.ommx.miplib.tags".to_string(), tags.join(","));
        }
        annotation.set_other(
            "org.ommx.miplib.url".to_string(),
            format!(
                "https://miplib.zib.de/instance_details_{}.html",
                self.instance
            ),
        );
        annotation
    }
}

/// Convert [MIPLIB2017_CSV] as [InstanceAnnotations] dictionary
///
/// MIPLIB-specific annotations are stored in the `org.ommx.miplib.*` namespace.
///
/// ```rust
/// use ommx::dataset::miplib2017;
///
/// let annotations = miplib2017::instance_annotations().get("air05").unwrap().clone();
///
/// // Common annotations
/// assert_eq!(annotations.title().unwrap(), "air05");
/// assert_eq!(annotations.authors().unwrap().next(), Some("G. Astfalk"));
///
/// // MIPLIB specific annotations
/// assert_eq!(annotations.get("org.ommx.miplib.status").unwrap(), "easy");
/// assert_eq!(annotations.get("org.ommx.miplib.group").unwrap(), "air");
/// assert_eq!(annotations.get("org.ommx.miplib.binaries").unwrap(), "7195");
/// assert_eq!(annotations.get("org.ommx.miplib.integers").unwrap(), "0");
/// assert_eq!(annotations.get("org.ommx.miplib.continuous").unwrap(), "0");
/// assert_eq!(annotations.get("org.ommx.miplib.non_zero").unwrap(), "52121");
/// assert_eq!(annotations.get("org.ommx.miplib.objective").unwrap(), "26374");
/// assert_eq!(annotations.get("org.ommx.miplib.tags").unwrap(), "benchmark,binary,benchmark_suitable,set_partitioning");
/// assert_eq!(annotations.get("org.ommx.miplib.url").unwrap(), "https://miplib.zib.de/instance_details_air05.html");
/// ```
pub fn instance_annotations() -> HashMap<String, InstanceAnnotations> {
    let mut rdr = csv::Reader::from_reader(MIPLIB2017_CSV.as_bytes());
    let mut entries = HashMap::new();
    for result in rdr.deserialize() {
        let entry: RawEntry = result.expect("Invalid CSV for MIPLIB2017");
        entries.insert(entry.instance.clone(), entry.as_annotation());
    }
    entries
}

/// Instances which OMMX cannot load correctly
fn check_unsupported(name: &str) -> Result<()> {
    ensure!(
        ![
            "neos-933638",
            "neos-935769",
            "neos-983171",
            "neos-932721",
            "dsbmip",
            "neos-935234",
            "lrn",
            "neos-933966",
            "ivu52",
            "mad",
        ]
        .contains(&name),
        "Instance {name} is a multi-objective problem, which is not supported by OMMX."
    );
    ensure!(
        ![
            "supportcase27i",
            "supportcase21i",
            "supportcase28i",
            "mrcpspj30-17-10i",
            "gfd-schedulen25f5d20m10k3i",
            "fjspeasy01i",
            "splice1k1i",
            "elitserienhandball11i",
            "elitserienhandball13i",
            "mappingmesh3x3mpeg2i",
            "amaze22012-07-04i",
            "cvrpp-n16k8vrpi",
            "l2p2i",
            "mrcpspj30-15-5i",
            "mario-t-hard5i",
            "shipschedule6shipsmixi",
            "mrcpspj30-53-3i",
            "mspsphard01i",
            "gfd-schedulen55f2d50m30k3i",
            "amaze22012-03-15i",
            "elitserienhandball3i",
            "cvrpb-n45k5vrpi",
            "gfd-schedulen180f7d50m30k18-16i",
            "elitserienhandball14i",
            "cvrpa-n64k9vrpi",
            "k1mushroomi",
            "shipschedule8shipsmixuci",
            "amaze22012-06-28i",
            "pizza78i",
            "pizza27i",
            "rpp22falsei",
            "oocsp-racks030f7cci",
            "fillomino7x7-0i",
            "l2p1i",
            "stoch-vrpvrp-s5v2c8vrp-v2c8i",
            "shipschedule3shipsi",
            "cvrpsimple2i",
            "mspsphard03i",
            "oocsp-racks030e6cci",
            "ghoulomb4-9-10i",
        ]
        .contains(&name),
        "Instance {name} contains 'INDICATORS', which is not supported by OMMX."
    );
    ensure!(
        !["diameterc-msts-v40a100d5i", "diameterc-mstc-v20a190d5i"].contains(&name),
        "Instance {name} contains 'LAZYCONS', which is not supported by OMMX."
    );
    ensure!(
        name != "neos-5044663-wairoa",
        "Instance {name} looks broken MPS file."
    );
    Ok(())
}

/// Load an instance from the MIPLIB 2017 dataset
pub fn load(name: &str) -> Result<(Instance, InstanceAnnotations)> {
    let annotations = instance_annotations();
    ensure!(
        annotations.contains_key(name),
        "Given name '{name}' does not exist in MIPLIB 2017"
    );
    check_unsupported(name)?;

    let image_name = ghcr("Jij-Inc", "ommx", "miplib2017", name)?;
    let mut artifact = Artifact::from_remote(image_name)?.pull()?;
    let mut instances = artifact.get_instances()?;
    ensure!(
        instances.len() == 1,
        "MIPLIB2017 Artifact should contain exactly one instance"
    );
    let (desc, instance) = instances.pop().unwrap();
    Ok((
        instance,
        InstanceAnnotations::from(desc.annotations().clone().unwrap_or_default()),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_instance_annotations() {
        let annotations = super::instance_annotations();
        // Update this number if the CSV file is updated
        assert_eq!(annotations.len(), 1065);
    }
}
