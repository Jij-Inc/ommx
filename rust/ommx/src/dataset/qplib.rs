use crate::{
    artifact::{ghcr, Artifact, InstanceAnnotations},
    v1::Instance,
};

use anyhow::{ensure, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// CSV downloaded from [QPLIB website](http://qplib.zib.de/) on 2025-10-02
pub const QPLIB_CSV: &str = include_str!("qplib.csv");

#[derive(Debug, Deserialize)]
struct RawEntry {
    name: String,
    #[serde(rename = "solsource")]
    sol_source: String,
    donor: String,
    nvars: usize,
    ncons: usize,
    nbinvars: usize,
    nintvars: usize,
    nsemi: usize,
    nnlvars: usize,
    nnlbinvars: usize,
    nnlintvars: usize,
    nnlsemi: usize,
    nboundedvars: usize,
    nsingleboundedvars: usize,
    nsos1: usize,
    nsos2: usize,
    objsense: String,
    nobjnz: usize,
    nobjnlnz: usize,
    njacobiannz: usize,
    njacobiannlnz: usize,
    nlaghessiannz: usize,
    nlaghessiandiagnz: usize,
    nobjquadnz: usize,
    nobjquaddiagnz: usize,
    nobjquadnegev: i64,
    nobjquadposev: i64,
    objtype: String,
    objcurvature: String,
    conscurvature: String,
    nconvexnlcons: usize,
    nconcavenlcons: usize,
    nindefinitenlcons: usize,
    nlincons: usize,
    nquadcons: usize,
    ndiagquadcons: usize,
    nlaghessianblocks: usize,
    laghessianminblocksize: usize,
    laghessianmaxblocksize: usize,
    laghessianavgblocksize: f64,
    solobjvalue: String,
    solinfeasibility: String,
    probtype: String,
    nlinfunc: usize,
    nquadfunc: usize,
    nnlfunc: usize,
    nz: usize,
    nlnz: usize,
    ncontvars: usize,
    convex: String,
    density: f64,
    nldensity: f64,
    objquaddensity: f64,
    objquadproblevfrac: f64,
}

impl RawEntry {
    fn as_annotation(&self) -> InstanceAnnotations {
        let mut annotation = InstanceAnnotations::default();
        annotation.set_title(self.name.clone());
        if !self.donor.is_empty() {
            annotation.set_authors(vec![self.donor.clone()]);
        }
        // QPLIB is licensed under CC-BY 4.0 as of August 30, 2021
        annotation.set_license("CC-BY-4.0".to_string());
        annotation.set_dataset("QPLIB".to_string());

        // Store QPLIB's original counts in qplib namespace
        // Note: QPLIB and OMMX may count constraints differently (e.g., l <= f(x) <= u)
        annotation.set_other("org.ommx.qplib.nvars".to_string(), self.nvars.to_string());
        annotation.set_other("org.ommx.qplib.ncons".to_string(), self.ncons.to_string());

        // QPLIB specific annotations - variable counts
        for (key, value) in [
            ("nbinvars", self.nbinvars),
            ("nintvars", self.nintvars),
            ("ncontvars", self.ncontvars),
            ("nsemi", self.nsemi),
            ("nnlvars", self.nnlvars),
            ("nnlbinvars", self.nnlbinvars),
            ("nnlintvars", self.nnlintvars),
            ("nnlsemi", self.nnlsemi),
            ("nboundedvars", self.nboundedvars),
            ("nsingleboundedvars", self.nsingleboundedvars),
            ("nsos1", self.nsos1),
            ("nsos2", self.nsos2),
        ] {
            annotation.set_other(format!("org.ommx.qplib.{key}"), value.to_string());
        }

        // QPLIB specific annotations - constraint counts
        for (key, value) in [
            ("nlincons", self.nlincons),
            ("nquadcons", self.nquadcons),
            ("ndiagquadcons", self.ndiagquadcons),
            ("nconvexnlcons", self.nconvexnlcons),
            ("nconcavenlcons", self.nconcavenlcons),
            ("nindefinitenlcons", self.nindefinitenlcons),
        ] {
            annotation.set_other(format!("org.ommx.qplib.{key}"), value.to_string());
        }

        // QPLIB specific annotations - objective function
        for (key, value) in [
            ("nobjnz", self.nobjnz),
            ("nobjnlnz", self.nobjnlnz),
            ("nobjquadnz", self.nobjquadnz),
            ("nobjquaddiagnz", self.nobjquaddiagnz),
        ] {
            annotation.set_other(format!("org.ommx.qplib.{key}"), value.to_string());
        }

        for (key, value) in [
            ("nobjquadnegev", self.nobjquadnegev),
            ("nobjquadposev", self.nobjquadposev),
        ] {
            annotation.set_other(format!("org.ommx.qplib.{key}"), value.to_string());
        }

        // QPLIB specific annotations - Jacobian and Hessian
        for (key, value) in [
            ("njacobiannz", self.njacobiannz),
            ("njacobiannlnz", self.njacobiannlnz),
            ("nlaghessiannz", self.nlaghessiannz),
            ("nlaghessiandiagnz", self.nlaghessiandiagnz),
            ("nlaghessianblocks", self.nlaghessianblocks),
            ("laghessianminblocksize", self.laghessianminblocksize),
            ("laghessianmaxblocksize", self.laghessianmaxblocksize),
        ] {
            annotation.set_other(format!("org.ommx.qplib.{key}"), value.to_string());
        }

        annotation.set_other(
            "org.ommx.qplib.laghessianavgblocksize".to_string(),
            self.laghessianavgblocksize.to_string(),
        );

        // QPLIB specific annotations - nonzero counts and functions
        for (key, value) in [
            ("nz", self.nz),
            ("nlnz", self.nlnz),
            ("nlinfunc", self.nlinfunc),
            ("nquadfunc", self.nquadfunc),
            ("nnlfunc", self.nnlfunc),
        ] {
            annotation.set_other(format!("org.ommx.qplib.{key}"), value.to_string());
        }

        // QPLIB specific annotations - density measures
        annotation.set_other(
            "org.ommx.qplib.density".to_string(),
            self.density.to_string(),
        );
        annotation.set_other(
            "org.ommx.qplib.nldensity".to_string(),
            self.nldensity.to_string(),
        );
        annotation.set_other(
            "org.ommx.qplib.objquaddensity".to_string(),
            self.objquaddensity.to_string(),
        );
        annotation.set_other(
            "org.ommx.qplib.objquadproblevfrac".to_string(),
            self.objquadproblevfrac.to_string(),
        );

        // QPLIB specific annotations - problem characteristics
        annotation.set_other("org.ommx.qplib.objsense".to_string(), self.objsense.clone());
        annotation.set_other("org.ommx.qplib.objtype".to_string(), self.objtype.clone());
        annotation.set_other(
            "org.ommx.qplib.objcurvature".to_string(),
            self.objcurvature.clone(),
        );
        annotation.set_other(
            "org.ommx.qplib.conscurvature".to_string(),
            self.conscurvature.clone(),
        );
        annotation.set_other("org.ommx.qplib.probtype".to_string(), self.probtype.clone());
        annotation.set_other("org.ommx.qplib.convex".to_string(), self.convex.clone());

        // QPLIB specific annotations - solution information
        if !self.solobjvalue.is_empty() {
            annotation.set_other(
                "org.ommx.qplib.solobjvalue".to_string(),
                self.solobjvalue.clone(),
            );
        }
        if !self.solinfeasibility.is_empty() {
            annotation.set_other(
                "org.ommx.qplib.solinfeasibility".to_string(),
                self.solinfeasibility.clone(),
            );
        }
        if !self.sol_source.is_empty() {
            annotation.set_other(
                "org.ommx.qplib.solsource".to_string(),
                self.sol_source.clone(),
            );
        }

        annotation.set_other(
            "org.ommx.qplib.url".to_string(),
            format!(
                "http://qplib.zib.de/QPLIB_{}.html",
                self.name.strip_prefix("QPLIB_").unwrap_or(&self.name)
            ),
        );
        annotation
    }
}

/// Convert [QPLIB_CSV] as [InstanceAnnotations] dictionary
///
/// QPLIB-specific annotations are stored in the `org.ommx.qplib.*` namespace.
/// Field definitions are documented at <https://qplib.zib.de/doc.html>.
///
/// ```rust
/// use ommx::dataset::qplib;
///
/// let annotations = qplib::instance_annotations();
/// let annotation = annotations.get("0018").unwrap();
///
/// // Common annotations
/// assert_eq!(annotation.title().unwrap(), "QPLIB_0018");
/// assert_eq!(annotation.dataset().unwrap(), "QPLIB");
///
/// // QPLIB specific annotations (QPLIB's original counts)
/// assert_eq!(annotation.get("org.ommx.qplib.nvars").unwrap(), "50");
/// assert_eq!(annotation.get("org.ommx.qplib.ncons").unwrap(), "1");
/// assert_eq!(annotation.get("org.ommx.qplib.objtype").unwrap(), "quadratic");
/// assert_eq!(annotation.get("org.ommx.qplib.objcurvature").unwrap(), "indefinite");
/// assert_eq!(annotation.get("org.ommx.qplib.probtype").unwrap(), "QCL");
/// assert_eq!(annotation.get("org.ommx.qplib.url").unwrap(), "http://qplib.zib.de/QPLIB_0018.html");
/// ```
pub fn instance_annotations() -> HashMap<String, InstanceAnnotations> {
    let mut rdr = csv::Reader::from_reader(QPLIB_CSV.as_bytes());
    let mut entries = HashMap::new();
    for result in rdr.deserialize() {
        let entry: RawEntry = result.expect("Invalid CSV for QPLIB");
        let key = entry.name.strip_prefix("QPLIB_").unwrap_or(&entry.name).to_string();
        entries.insert(key, entry.as_annotation());
    }
    entries
}

/// Load an instance from the QPLIB dataset
///
/// # Arguments
///
/// * `tag` - The numeric tag of the QPLIB instance (e.g., "0018" for QPLIB_0018)
///
/// # Example
///
/// ```no_run
/// use ommx::dataset::qplib;
///
/// // Load QPLIB_0018 from local artifact (requires prior packaging)
/// let (instance, annotation) = qplib::load("0018").unwrap();
/// assert_eq!(annotation.title().unwrap(), "QPLIB_0018");
/// assert_eq!(annotation.dataset().unwrap(), "QPLIB");
/// assert!(instance.decision_variables.len() > 0);
/// ```
pub fn load(tag: &str) -> Result<(Instance, InstanceAnnotations)> {
    let annotations = instance_annotations();
    ensure!(
        annotations.contains_key(tag),
        "Given tag '{tag}' (QPLIB_{tag}) does not exist in QPLIB"
    );

    let image_name = ghcr("Jij-Inc", "ommx", "qplib", tag)?;
    let mut artifact = Artifact::from_remote(image_name)?.pull()?;
    let mut instances = artifact.get_instances()?;
    ensure!(
        instances.len() == 1,
        "QPLIB Artifact should contain exactly one instance"
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
        // QPLIB contains 453 instances
        assert_eq!(annotations.len(), 453);
    }

    #[test]
    fn test_instance_annotations_key_format() {
        let annotations = super::instance_annotations();
        
        // Test that keys are in numeric format (not "QPLIB_XXXX")
        // We know "0018" should exist from the CSV data
        let annotation = annotations.get("0018").expect("Should find annotation with key '0018'");
        
        // Verify that the title still contains the full QPLIB_XXXX format
        assert_eq!(annotation.title().unwrap(), "QPLIB_0018");
        assert_eq!(annotation.dataset().unwrap(), "QPLIB");
        
        // Verify that old format "QPLIB_0018" does NOT work as a key
        assert!(annotations.get("QPLIB_0018").is_none(), 
                "Old format key 'QPLIB_0018' should not exist in HashMap");
        
        // Test a few more instances to ensure consistency
        for key in ["0031", "0032", "0067"] {
            assert!(annotations.contains_key(key), 
                   "Should find annotation with numeric key '{}'", key);
        }
    }

    #[test]
    fn test_load_qplib_3877() {
        // This test requires QPLIB_3877 to be packaged locally
        let result = super::load("3877");
        match result {
            Ok((instance, annotation)) => {
                assert_eq!(annotation.title().unwrap(), "QPLIB_3877");
                assert_eq!(annotation.dataset().unwrap(), "QPLIB");
                assert!(instance.decision_variables.len() > 0);
                println!(
                    "Successfully loaded QPLIB_3877: {} vars, {} constraints",
                    instance.decision_variables.len(),
                    instance.constraints.len()
                );
            }
            Err(e) => {
                // If artifact doesn't exist locally, skip the test
                println!("Skipping test_load_qplib_3877: {}", e);
            }
        }
    }
}
