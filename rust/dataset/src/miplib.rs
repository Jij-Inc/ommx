use anyhow::Result;
use ommx::artifact::InstanceAnnotations;
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fs, path::Path};
use zip::ZipArchive;

/// CSV file downloaded from [MIPLIB website](https://miplib.zib.de/tag_collection.html)
const MIPLIB_CSV: &str = include_str!("../miplib.csv");

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
struct MiplibEntry {
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

fn miplib_entries() -> Result<HashMap<String, MiplibEntry>> {
    let mut rdr = csv::Reader::from_reader(MIPLIB_CSV.as_bytes());
    let mut entries = HashMap::new();
    dbg!(rdr.headers()?);
    for result in rdr.deserialize() {
        let entry: MiplibEntry = result?;
        entries.insert(entry.instance.clone(), entry);
    }
    Ok(entries)
}

pub fn package(path: &Path) -> Result<()> {
    let entries = miplib_entries()?;
    println!("Input Archive: {}", path.display());
    let f = fs::File::open(path)?;
    let mut ar = ZipArchive::new(f)?;

    for i in 0..ar.len() {
        let file = ar.by_index(i)?;
        let Some(name) = file.name().strip_suffix(".mps.gz").map(str::to_string) else {
            continue;
        };
        let Some(entry) = entries.get(&name) else {
            eprintln!("No metadata found for '{}'", name);
            continue;
        };
        println!("Loading: {}", name);
        let _instance = match ommx::mps::load_reader(file) {
            Ok(instance) => instance,
            Err(err) => {
                eprintln!("Failed to load '{name}' with error: {err}");
                continue;
            }
        };
        let mut annotation = InstanceAnnotations::default();
        annotation.set_title(name);
        annotation.set_authors(
            entry
                .submitter
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        );
        if entry.submitter.contains("Berk Ustun") {
            // Berk Ustun's submissions are licensed under "BSD", we assume it is "BSD-3-Clause"
            // https://git.zib.de/miplib2017/submissions/-/blob/master/Berk_Ustun/meta.yml?ref_type=heads
            annotation.set_license("BSD-3-Clause".to_string());
        } else {
            // Other submissions are licensed under the default MIPLIB license "CC-BY-SA-4.0"
            annotation.set_license("CC-BY-SA-4.0".to_string());
        }
        annotation.set_dataset("MIPLIB2017".to_string());
        annotation.set_variables(entry.variable as usize);
        annotation.set_constraints(entry.constraints as usize);

        // MIPLIB specific annotations
        for (key, value) in [
            ("binaries", entry.binaries as usize),
            ("integers", entry.integers as usize),
            ("continuous", entry.continuous as usize),
            ("non_zero", entry.non_zero as usize),
        ] {
            annotation.set_other(format!("org.ommx.miplib.{}", key), value.to_string());
        }
        annotation.set_other(
            "org.ommx.miplib.status".to_string(),
            entry.status.to_string(),
        );
        if entry.group != "-" {
            annotation.set_other("org.ommx.miplib.group".to_string(), entry.group.to_string());
        }
        if let Some(objective) = entry.objective.try_to_string() {
            annotation.set_other("org.ommx.miplib.objective".to_string(), objective);
        }
        let tags: Vec<_> = entry.tags.split(' ').map(str::trim).collect();
        if !tags.is_empty() {
            annotation.set_other("org.ommx.miplib.tags".to_string(), tags.join(","));
        }
        dbg!(annotation);
    }
    Ok(())
}
