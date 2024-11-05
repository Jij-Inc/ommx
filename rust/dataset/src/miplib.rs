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

impl<'de> Deserialize<'de> for ObjectiveValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let s = s.trim_end_matches("*");
        match s {
            "Infeasible" => return Ok(ObjectiveValue::Infeasible),
            "NA" => return Ok(ObjectiveValue::NotAvailable),
            "Unbounded" => return Ok(ObjectiveValue::Unbounded),
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

        dbg!(annotation);
    }
    Ok(())
}
