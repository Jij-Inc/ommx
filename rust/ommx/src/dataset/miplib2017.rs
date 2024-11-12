use crate::{
    artifact::{ghcr, Artifact, InstanceAnnotations},
    v1::Instance,
};

use anyhow::{ensure, Result};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// CSV file downloaded from [MIPLIB website](https://miplib.zib.de/tag_collection.html)
const MIPLIB_CSV: &str = include_str!("miplib2017.csv");

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
            annotation.set_other(format!("org.ommx.miplib.{}", key), value.to_string());
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

pub fn instance_annotations() -> HashMap<String, InstanceAnnotations> {
    let mut rdr = csv::Reader::from_reader(MIPLIB_CSV.as_bytes());
    let mut entries = HashMap::new();
    for result in rdr.deserialize() {
        let entry: RawEntry = result.expect("Invalid CSV for MIPLIB2017");
        entries.insert(entry.instance.clone(), entry.as_annotation());
    }
    entries
}

pub fn load_instance(name: &str) -> Result<(Instance, InstanceAnnotations)> {
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
