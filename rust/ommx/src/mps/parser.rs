use super::{is_gzipped, MpsParseError};
use derive_more::Deref;
use indexmap::IndexSet;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufRead, Read},
    path::Path,
    str::FromStr,
};

type Result<T> = std::result::Result<T, MpsParseError>;

/// A linear optimization problem loaded from MPS format
///
/// The data stored in a file of MPS format can be regarded as a sparse representation of the linear optimization problem:
///
/// $$
/// \begin{aligned}
/// \text{Minimize} \quad & c^T x \\\\
/// \text{subject to} \quad & Ax \circ b, \space l \le x \le u
/// \end{aligned}
/// $$
///
/// where $\circ$ is a vector of equality ($=$) or inequality ($\ge, \le$) operators.
/// The index of $x$ is the name of columns represented by [ColumnName].
/// $c$ and each row of $A$ also has the name represented by [RowName].
/// This parser converts any numerical variables into `f64`.
///
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mps {
    /// The name of the problem
    pub name: String,
    pub obj_sense: ObjSense,
    /// The name of the row corresponding to the objective function
    pub objective_name: RowName,
    /// The collection of all variables present -- useful for iterating over
    /// all variables in a problem.
    pub vars: IndexSet<ColumnName>,
    /// The coefficients of objective function, $c$
    pub c: HashMap<ColumnName, f64>,

    /// Constraint matrix, $A$
    pub a: HashMap<RowName, HashMap<ColumnName, f64>>,
    /// Right hand side of constraints, $b$
    pub b: HashMap<RowName, f64>,

    /// Upper bound for each column, $u$
    pub u: HashMap<ColumnName, f64>,
    /// Lower bound for each column, $l$
    pub l: HashMap<ColumnName, f64>,

    /// The column names of the variables that are required to be integer
    pub integer: HashSet<ColumnName>,
    /// The column names of the variables that are required to be binary
    pub binary: HashSet<ColumnName>,
    /// The column names which are not specified as integer or binary
    pub real: HashSet<ColumnName>,

    /// The row names of the constraints with equality
    pub eq: HashSet<RowName>,
    /// The row names of the constraints with inequality ($\ge$)
    pub ge: HashSet<RowName>,
    /// The row names of the constraints with inequality ($\le$)
    pub le: HashSet<RowName>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ObjSense {
    #[default]
    Min,
    Max,
}

impl FromStr for ObjSense {
    type Err = MpsParseError;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "MIN" => Ok(Self::Min),
            "MAX" => Ok(Self::Max),
            _ => Err(MpsParseError::InvalidObjSense(s.to_string())),
        }
    }
}

impl std::fmt::Display for ObjSense {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ObjSense::Min => "MIN",
            ObjSense::Max => "MAX",
        };
        write!(f, "{s}")
    }
}

/// A marker new type of `String` to distinguish row name and column name
#[derive(Debug, Deref, Clone, PartialEq, Eq, Hash, Default)]
pub struct RowName(pub String);

impl From<&str> for RowName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A marker new type of `String` to distinguish row name and column name
#[derive(Debug, Deref, Clone, PartialEq, Eq, Hash, Default)]
pub struct ColumnName(pub String);

impl From<&str> for ColumnName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Default)]
enum Cursor {
    #[default]
    Name,
    Rows,
    Columns,
    Rhs,
    Ranges,
    Bounds,
    End,
}

impl FromStr for Cursor {
    type Err = MpsParseError;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "ROWS" => Ok(Self::Rows),
            "COLUMNS" => Ok(Self::Columns),
            "RHS" => Ok(Self::Rhs),
            "RANGES" => Ok(Self::Ranges),
            "BOUNDS" => Ok(Self::Bounds),
            "ENDATA" => Ok(Self::End),
            _ => Err(MpsParseError::InvalidHeader(s.to_string())),
        }
    }
}

/// State machine for parsing MPS format
#[derive(Debug, Default)]
struct State {
    cursor: Cursor,
    is_integer_variable: bool,
    is_waiting_objsense_line: bool,
    mps: Mps,
}

impl State {
    fn read_header(&mut self, line: String) -> Result<()> {
        if let Some(name) = line.strip_prefix("NAME") {
            self.mps.name = name.trim().to_string();
        } else if let Some(sense) = line.strip_prefix("OBJSENSE") {
            if sense.trim().is_empty() {
                self.is_waiting_objsense_line = true;
                return Ok(());
            }
            self.mps.obj_sense = sense.trim().parse()?;
        } else {
            self.cursor = line.trim().parse()?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Field:    1           2          3         4         5         6
    // Columns:  2-3        5-12      15-22     25-36     40-47     50-61
    // ---------------------------------------------------------------------
    //           ROWS
    //            type     name
    fn read_row_field(&mut self, fields: Vec<&str>) -> Result<()> {
        assert_eq!(fields.len(), 2);
        let row_name = RowName(fields[1].to_string());
        match fields[0] {
            "N" => {
                if self.mps.objective_name.is_empty() {
                    self.mps.objective_name = row_name
                }
                // skip adding this row to `a` matrix
                return Ok(());
            }
            "E" => {
                self.mps.eq.insert(row_name.clone());
            }
            "G" => {
                self.mps.ge.insert(row_name.clone());
            }
            "L" => {
                self.mps.le.insert(row_name.clone());
            }
            _ => {
                return Err(MpsParseError::InvalidRowType(fields[0].to_string()));
            }
        }
        self.mps.a.insert(row_name, HashMap::new());
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Field:    1           2          3         4         5         6
    // Columns:  2-3        5-12      15-22     25-36     40-47     50-61
    // ---------------------------------------------------------------------
    //           COLUMNS
    //                    column       row       value     row      value
    //                     name        name                name
    fn read_column_field(&mut self, fields: Vec<&str>) -> Result<()> {
        assert!(fields.len() == 3 || fields.len() == 5);

        // G. A mixed integer program requires the specification of which variables
        //    are required to be integer.  Markers are used to indicate the start
        //    and end of a group of integer variables.  The start marker has its
        //    name in field 2, 'MARKER' in field 3, and 'INTORG' in field 5.  The
        //    end marker has its name in field 2, 'MARKER' in field 3, and 'INTEND'
        //    in field 5.  These markers are placed in the COLUMNS section.
        if fields[1] == "'MARKER'" {
            match fields[2] {
                "'INTORG'" => self.is_integer_variable = true,
                "'INTEND'" => self.is_integer_variable = false,
                _ => return Err(MpsParseError::InvalidMarker(fields[2].to_string())),
            }
            return Ok(());
        }

        let col_name = ColumnName(fields[0].to_string());
        self.mps.vars.insert(col_name.clone());
        if self.is_integer_variable {
            self.mps.integer.insert(col_name.clone());
        } else {
            self.mps.real.insert(col_name.clone());
        }

        for chunk in fields[1..].chunks(2) {
            let row_name = RowName(chunk[0].to_string());
            let coefficient = chunk[1].parse()?;
            if row_name == self.mps.objective_name {
                self.mps.c.insert(col_name.clone(), coefficient);
            } else {
                self.mps
                    .a
                    .get_mut(&row_name)
                    .ok_or(MpsParseError::UnknownRowName(row_name.0.clone()))?
                    .insert(col_name.clone(), coefficient);
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Field:    1           2          3         4         5         6
    // Columns:  2-3        5-12      15-22     25-36     40-47     50-61
    // ---------------------------------------------------------------------
    //           RHS
    //                     rhs         row       value     row      value
    //                     name        name                name
    fn read_rhs_field(&mut self, fields: Vec<&str>) -> Result<()> {
        assert!(fields.len() == 3 || fields.len() == 5);
        for chunk in fields[1..].chunks(2) {
            let row_name = RowName(chunk[0].to_string());
            let coefficient = chunk[1].parse()?;
            self.mps.b.insert(row_name, coefficient);
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Field:    1           2          3         4         5         6
    // Columns:  2-3        5-12      15-22     25-36     40-47     50-61
    // ---------------------------------------------------------------------
    //           RANGES
    //                     range       row       value     row      value
    //                     name        name                name
    //
    fn read_range_field(&mut self, fields: Vec<&str>) -> Result<()> {
        assert!(fields.len() == 3 || fields.len() == 5);
        for chunk in fields[1..].chunks(2) {
            let row_name = RowName(chunk[0].to_string());
            let range: f64 = chunk[1].parse()?;
            assert_ne!(range, 0.0, "RANGES with 0.0 is not supported");
            let mut new_row_name_candidate = RowName(format!("{}_", row_name.0));
            let new_row_name = loop {
                if !self.mps.a.contains_key(&new_row_name_candidate) {
                    break new_row_name_candidate;
                }
                new_row_name_candidate = RowName(format!("{}_", new_row_name_candidate.0));
            };
            let constraint = self
                .mps
                .a
                .get_mut(&row_name)
                .ok_or(MpsParseError::UnknownRowName(row_name.0.clone()))?
                .clone();
            self.mps.a.insert(new_row_name.clone(), constraint);
            // row type       sign of r       h          u
            // ----------------------------------------------
            //    G            + or -         b        b + |r|
            //    L            + or -       b - |r|      b
            //    E              +            b        b + |r|
            //    E              -          b - |r|      b
            let new_b = if self.mps.eq.contains(&row_name) {
                self.mps.eq.remove(&row_name);
                if range > 0.0 {
                    self.mps.ge.insert(row_name.clone());
                    self.mps.le.insert(new_row_name.clone());
                    self.mps.b.get(&row_name).unwrap_or(&0.0) + range.abs()
                } else {
                    self.mps.le.insert(row_name.clone());
                    self.mps.ge.insert(new_row_name.clone());
                    self.mps.b.get(&row_name).unwrap_or(&0.0) - range.abs()
                }
            } else if self.mps.ge.contains(&row_name) {
                self.mps.le.insert(new_row_name.clone());
                self.mps.b.get(&row_name).unwrap_or(&0.0) + range.abs()
            } else if self.mps.le.contains(&row_name) {
                self.mps.ge.insert(new_row_name.clone());
                self.mps.b.get(&row_name).unwrap_or(&0.0) - range.abs()
            } else {
                continue;
            };
            self.mps.b.insert(new_row_name, new_b);
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Field:    1           2          3         4         5         6
    // Columns:  2-3        5-12      15-22     25-36     40-47     50-61
    // ---------------------------------------------------------------------
    //           BOUNDS
    //            type     bound       column     value
    //                     name        name
    fn read_bound_field(&mut self, fields: Vec<&str>) -> Result<()> {
        match fields[0] {
            //  type            meaning
            // -----------------------------------
            //   LO    lower bound        b <= x
            "LO" => {
                self.mps
                    .l
                    .insert(ColumnName(fields[2].to_string()), fields[3].parse()?);
            }
            //   UP    upper bound        x <= b
            "UP" => {
                self.mps
                    .u
                    .insert(ColumnName(fields[2].to_string()), fields[3].parse()?);
            }
            //   FX    fixed variable     x = b
            "FX" => {
                let col_name = ColumnName(fields[2].to_string());
                let val = fields[3].parse()?;
                self.mps.l.insert(col_name.clone(), val);
                self.mps.u.insert(col_name, val);
            }
            //   MI    lower bound -inf   -inf < x
            "MI" => {
                self.mps
                    .l
                    .insert(ColumnName(fields[2].to_string()), f64::NEG_INFINITY);
            }
            //   BV    binary variable    x = 0 or 1
            "BV" => {
                let column_name = ColumnName(fields[2].to_string());
                self.mps.integer.remove(&column_name);
                self.mps.real.remove(&column_name);
                self.mps.binary.insert(column_name);
            }
            //   FR    free variable
            "FR" | "PL" => { /* do nothing */ }
            //   UI    upper (positive) integer
            "UI" => {
                let column_name = ColumnName(fields[2].to_string());
                let bound = fields[3].parse()?;
                self.mps.integer.insert(column_name.clone());
                self.mps.real.remove(&column_name);
                self.mps.u.insert(column_name, bound);
            }
            //   LI    lower (negative) integer
            "LI" => {
                let column_name = ColumnName(fields[2].to_string());
                let bound = fields[3].parse()?;
                self.mps.integer.insert(column_name.clone());
                self.mps.real.remove(&column_name);
                self.mps.l.insert(column_name, bound);
            }
            _ => {
                return Err(MpsParseError::InvalidBoundType(fields[0].to_string()));
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Mps {
        // If an integer variable `x` has a bound `0 <= x <= 1`,
        // regard it as a binary variable.
        for (name, u) in &self.mps.u {
            if *u == 1.0 {
                if let Some(l) = self.mps.l.get(name) {
                    if *l != 0.0 {
                        continue;
                    }
                }
                if let Some(name) = self.mps.integer.take(name) {
                    self.mps.binary.insert(name);
                }
            }
        }
        self.mps
    }
}

impl Mps {
    /// Read a MPS file from the given path.
    ///
    /// This function automatically detects if the file is gzipped or not.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let f = fs::File::open(&path)?;
        Self::parse(f)
    }

    pub fn parse(reader: impl Read) -> Result<Self> {
        let mut reader = io::BufReader::new(reader);
        let head = reader.fill_buf()?;
        if is_gzipped(head)? {
            let buf = flate2::read::GzDecoder::new(reader);
            let buf = io::BufReader::new(buf);
            Self::from_lines(buf.lines().map_while(|x| x.ok()))
        } else {
            let buf = io::BufReader::new(reader);
            Self::from_lines(buf.lines().map_while(|x| x.ok()))
        }
    }

    fn from_lines(lines: impl Iterator<Item = String>) -> Result<Self> {
        let mut state = State::default();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            // `*` is used as a comment in some files
            if line.starts_with('*') {
                continue;
            }

            // HEADER case
            if !line.starts_with(' ') {
                state.read_header(line)?;
                continue;
            }

            // FIELD case
            //
            // The original fixed format is designed as following:
            // ---------------------------------------------------------------------
            // Field:    1           2          3         4         5         6
            // Columns:  2-3        5-12      15-22     25-36     40-47     50-61
            // ---------------------------------------------------------------------
            //
            // But some data including the benchmark dataset in MIPLIB does not follow it.
            // Instead, we parse it as space-separated format.
            let fields = line.split_whitespace().collect::<Vec<_>>();

            if state.is_waiting_objsense_line {
                state.mps.obj_sense = fields[0].parse()?;
                state.is_waiting_objsense_line = false;
                continue;
            }

            match state.cursor {
                Cursor::Rows => state.read_row_field(fields)?,
                Cursor::Columns => state.read_column_field(fields)?,
                Cursor::Rhs => state.read_rhs_field(fields)?,
                Cursor::Ranges => state.read_range_field(fields)?,
                Cursor::Bounds => state.read_bound_field(fields)?,
                Cursor::Name => return Err(MpsParseError::InvalidHeader(line)),
                Cursor::End => break,
            }
        }
        Ok(state.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_eq_p() {
        // x = 1, range = 1.0 -> 1 <= x <= 2
        let mut state = State::default();
        state.read_row_field(vec!["E", "r1"]).unwrap(); // r1 is equality
        state.read_column_field(vec!["c1", "r1", "1.0"]).unwrap(); // a[c1, r1] = 1
        state.read_rhs_field(vec!["rhs", "r1", "1.0"]).unwrap(); // b[r1] = 1
        state.read_range_field(vec!["range", "r1", "1.0"]).unwrap();
        dbg!(&state);

        let r1: RowName = "r1".into();
        let new: RowName = "r1_".into();
        assert!(state.mps.eq.is_empty());
        // x >= 1, this row is original one
        assert_eq!(state.mps.ge, [r1.clone()].into());
        assert_eq!(state.mps.b.get(&r1), Some(&1.0));
        // x <= 2, this row is new one
        assert_eq!(state.mps.le, [new.clone()].into());
        assert_eq!(state.mps.b.get(&new), Some(&2.0));
    }

    #[test]
    fn range_eq_n() {
        // x = 2, range = -1 -> 1 <= x <= 2
        let mut state = State::default();
        state.read_row_field(vec!["E", "r1"]).unwrap(); // r1 is equality
        state.read_column_field(vec!["c1", "r1", "1.0"]).unwrap(); // a[c1, r1] = 1
        state.read_rhs_field(vec!["rhs", "r1", "2.0"]).unwrap(); // b[r1] = 2
        state.read_range_field(vec!["range", "r1", "-1.0"]).unwrap();
        dbg!(&state);

        let r1: RowName = "r1".into();
        let new: RowName = "r1_".into();
        assert!(state.mps.eq.is_empty());
        // x >= 1, this row is new one
        assert_eq!(state.mps.ge, [new.clone()].into());
        assert_eq!(state.mps.b.get(&new), Some(&1.0));
        // x <= 2, this row is original one
        assert_eq!(state.mps.le, [r1.clone()].into());
        assert_eq!(state.mps.b.get(&r1), Some(&2.0));
    }

    #[test]
    fn range_ge() {
        // x >= 1, range = 1 -> 1 <= x <= 2
        let mut state = State::default();
        state.read_row_field(vec!["G", "r1"]).unwrap(); // r1 is greater-than inequality
        state.read_column_field(vec!["c1", "r1", "1.0"]).unwrap(); // a[c1, r1] = 1
        state.read_rhs_field(vec!["rhs", "r1", "1.0"]).unwrap(); // b[r1] = 1
        state.read_range_field(vec!["range", "r1", "1.0"]).unwrap();
        dbg!(&state);

        let r1: RowName = "r1".into();
        let new: RowName = "r1_".into();
        assert!(state.mps.eq.is_empty());
        // x >= 1, this row is original one
        assert_eq!(state.mps.ge, [r1.clone()].into());
        assert_eq!(state.mps.b.get(&r1), Some(&1.0));
        // x <= 2, this row is new one
        assert_eq!(state.mps.le, [new.clone()].into());
        assert_eq!(state.mps.b.get(&new), Some(&2.0));
    }

    #[test]
    fn range_le() {
        // x <= 2, range = 1 -> 1 <= x <= 2
        let mut state = State::default();
        state.read_row_field(vec!["L", "r1"]).unwrap(); // r1 is greater-than inequality
        state.read_column_field(vec!["c1", "r1", "1.0"]).unwrap(); // a[c1, r1] = 1
        state.read_rhs_field(vec!["rhs", "r1", "2.0"]).unwrap(); // b[r1] = 2
        state.read_range_field(vec!["range", "r1", "1.0"]).unwrap();
        dbg!(&state);

        let r1: RowName = "r1".into();
        let new: RowName = "r1_".into();
        assert!(state.mps.eq.is_empty());
        // x >= 1, this row is new one
        assert_eq!(state.mps.ge, [new.clone()].into());
        assert_eq!(state.mps.b.get(&new), Some(&1.0));
        // x <= 2, this row is original one
        assert_eq!(state.mps.le, [r1.clone()].into());
        assert_eq!(state.mps.b.get(&r1), Some(&2.0));
    }

    #[test]
    fn as_binary() {
        // 0 <= x <= 1
        let mut state = State::default();
        let col: ColumnName = "x".into();
        state.mps.integer.insert(col.clone());
        state.mps.u.insert(col.clone(), 1.0);

        let mps = state.finish();
        assert!(mps.integer.is_empty());
        assert_eq!(mps.binary, [col].into());

        // -1 <= x <= 1
        let mut state = State::default();
        let col: ColumnName = "x".into();
        state.mps.integer.insert(col.clone());
        state.mps.u.insert(col.clone(), 1.0);
        state.mps.l.insert(col.clone(), -1.0);

        let mps = state.finish();
        assert_eq!(mps.integer, [col].into());
        assert!(mps.binary.is_empty());
    }

    #[test]
    fn objsense() {
        let input = indoc::indoc! {r#"
            NAME Problem
            OBJSENSE MAX
            ENDATA
        "#};
        let mps = Mps::from_lines(input.lines().map(|x| x.to_string())).unwrap();
        assert_eq!(mps.obj_sense, ObjSense::Max);

        // maybe separated line
        let input = indoc::indoc! {r#"
            NAME Problem
            OBJSENSE
             MAX
            ENDATA
        "#};
        let mps = Mps::from_lines(input.lines().map(|x| x.to_string())).unwrap();
        assert_eq!(mps.obj_sense, ObjSense::Max);

        // MIN and MAX are only allowed
        let input = indoc::indoc! {r#"
            NAME Problem
            OBJSENSE
             MINMAX
            ENDATA
        "#};
        assert!(Mps::from_lines(input.lines().map(|x| x.to_string())).is_err());

        // MAX must be field, not be header
        let input = indoc::indoc! {r#"
            NAME Problem
            OBJSENSE
            MAX
            ENDATA
        "#};
        assert!(Mps::from_lines(input.lines().map(|x| x.to_string())).is_err());
    }
}
