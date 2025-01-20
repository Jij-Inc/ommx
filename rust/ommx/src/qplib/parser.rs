use super::{ParseErrorReason, QplibParseError};
use std::collections::HashMap;
use std::{
    fmt::Display,
    fs,
    io::{self, BufRead, Read},
    path::Path,
    str::FromStr,
};

use anyhow::{Context, Result};

#[derive(Default, Debug)]
pub struct QplibFile {
    pub name: String,
    pub problem_type: ProblemType,
    pub sense: ObjSense,
    pub num_vars: usize,
    pub num_constraints: usize,
    pub var_types: Vec<VarType>,

    // Q^0. More specifically, "the non-zeroes in the lower triangle of Q^0"
    pub q0_non_zeroes: HashMap<(usize, usize), f64>,
    pub b0_non_defaults: HashMap<usize, f64>,
    pub obj_constant: f64,

    pub qs_non_zeroes: Vec<HashMap<(usize, usize), f64>>,
    pub bs_non_zeroes: Vec<HashMap<usize, f64>>,
    pub constr_lower_cs: Vec<f64>,
    pub constr_upper_cs: Vec<f64>,
    pub lower_bounds: Vec<f64>,
    pub upper_bounds: Vec<f64>,

    pub infinity_threshold: f64,

    pub default_b0: f64,

    // jij tooling doesn't support these values, but they are loaded
    // anyway in case the user wants them.
    pub default_starting_x: f64,
    pub starting_x: HashMap<usize, f64>,
    pub default_starting_y: f64,
    pub starting_y: HashMap<usize, f64>,
    pub default_starting_z: f64,
    pub starting_z: HashMap<usize, f64>,
    pub var_names: HashMap<usize, String>,
    pub constr_names: HashMap<usize, String>,
}

impl QplibFile {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let f = fs::File::open(path)
            .with_context(|| format!("Failed to read file {}", path.display()))?;
        Self::from_reader(f)
    }

    pub fn from_reader(reader: impl Read) -> Result<Self> {
        // let buf = flate2::read::GzDecoder::new(reader);
        let buf = io::BufReader::new(reader);
        Self::from_lines(buf.lines().map_while(|x| x.ok()))
    }

    pub fn from_lines(lines: impl Iterator<Item = String>) -> Result<Self> {
        use ProbConstrKind as C;
        use ProbObjKind as O;
        use ProbVarKind as V;
        let mut cursor = FileCursor::new(lines);
        let name = cursor
            .expect_next()?
            // take only first word
            .split_whitespace()
            .next()
            .map(|s| s.to_string())
            .ok_or(QplibParseError::invalid_line(cursor.line_num))?;
        let ProblemType(okind, vkind, ckind) = cursor.next_parse()?;
        let sense = cursor.next_parse()?;
        let num_vars = cursor.next_parse()?;
        let num_constraints = match ckind {
            C::Box | C::None => 0,
            _ => cursor.next_parse()?,
        };

        let q0_non_zeroes = match okind {
            O::Linear => Default::default(),
            _ => cursor.collect_ij_val()?,
        };
        let default_b0 = cursor.next_parse()?;
        let b0_non_defaults = cursor.collect_i_val()?;
        let obj_constant = cursor.next_parse()?;

        // non-zero quadratic and non-quadratic coefficient
        let (qs_non_zeroes, bs_non_zeroes) = match ckind {
            // skip all constraint coefficients
            C::None | C::Box => (Vec::new(), Vec::new()),
            // skip reading only quadratic matrix
            C::Linear => (Vec::new(), cursor.collect_list_of_i_val(num_constraints)?),
            _ => (
                cursor.collect_list_of_ij_val(num_constraints)?,
                cursor.collect_list_of_i_val(num_constraints)?,
            ),
        };

        let infinity_threshold = cursor.next_parse()?;

        let (constr_lower_cs, constr_upper_cs) = match ckind {
            C::None | C::Box => (vec![], vec![]),
            _ => {
                let lower_cs = cursor.collect_list(num_constraints)?;
                let upper_cs = cursor.collect_list(num_constraints)?;
                (lower_cs, upper_cs)
            }
        };

        let (lower_bounds, upper_bounds) = match vkind {
            V::Binary => (vec![0.; num_vars], vec![1.; num_vars]),
            _ => {
                let lower_bounds = cursor.collect_list(num_vars)?;
                let upper_bounds = cursor.collect_list(num_vars)?;
                (lower_bounds, upper_bounds)
            }
        };

        let var_types = match vkind {
            V::Continuous => vec![VarType::Continuous; num_vars],
            V::Binary => vec![VarType::Binary; num_vars],
            V::Integer => {
                let types = vec![VarType::Integer; num_vars];
                // find all the variables which have
                // lower bound = 0 and upper bound = 1.
                integer_to_binary(types, &lower_bounds, &upper_bounds)
            }
            _ => {
                let types = cursor.collect_list(num_vars)?;
                integer_to_binary(types, &lower_bounds, &upper_bounds)
            }
        };

        // We don't currently support these sections in a meaningful way. These
        // are checked just so callers can issue warnings in case they are
        // defined.
        let default_starting_x = cursor.next_parse()?;
        let starting_x = cursor.collect_i_val()?;
        let (default_starting_y, starting_y) = match ckind {
            C::None | C::Box => (0., Default::default()),
            _ => (cursor.next_parse()?, cursor.collect_i_val()?),
        };
        let default_starting_z = cursor.next_parse()?;
        let starting_z = cursor.collect_i_val()?;
        let var_names = cursor.collect_i_val()?;
        let constr_names = cursor.collect_i_val()?;

        Ok(QplibFile {
            name,
            problem_type: ProblemType(okind, vkind, ckind),
            sense,
            num_vars,
            num_constraints,
            var_types,
            q0_non_zeroes,
            b0_non_defaults,
            obj_constant,

            qs_non_zeroes,
            bs_non_zeroes,
            constr_lower_cs,
            constr_upper_cs,
            lower_bounds,
            upper_bounds,
            infinity_threshold,
            default_b0,

            default_starting_x,
            starting_x,
            default_starting_y,
            starting_y,
            default_starting_z,
            starting_z,
            var_names,
            constr_names,
        })
    }

    pub fn apply_infinity_threshold(&mut self) {
        let threshold = self.infinity_threshold;
        let apply = |val: &mut f64, inf| {
            if val.abs() >= threshold {
                *val = inf;
            }
        };
        let apply_pos = |val| apply(val, f64::INFINITY);
        let apply_neg = |val| apply(val, f64::NEG_INFINITY);

        self.lower_bounds.iter_mut().for_each(apply_neg);
        self.constr_lower_cs.iter_mut().for_each(apply_neg);

        self.upper_bounds.iter_mut().for_each(apply_pos);
        self.constr_upper_cs.iter_mut().for_each(apply_pos);
    }
}

#[derive(Default, Debug)]
pub struct ProblemType(ProbObjKind, ProbVarKind, ProbConstrKind);

impl Display for ProblemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}{}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum ProbObjKind {
    Linear,
    DiagonalC,       // convex if minimization; concave if maximization
    ConcaveOrConvex, // convex if minimization; concave if maximization
    #[default]
    Quadratic, // generic case
}

impl Display for ProbObjKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            ProbObjKind::Linear => 'L',
            ProbObjKind::DiagonalC => 'D',
            ProbObjKind::ConcaveOrConvex => 'C',
            ProbObjKind::Quadratic => 'Q',
        };
        write!(f, "{c}")
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum ProbVarKind {
    Continuous,
    Binary,
    Mixed, // binary & continuous
    Integer,
    #[default]
    General,
}

impl Display for ProbVarKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            ProbVarKind::Continuous => 'C',
            ProbVarKind::Binary => 'B',
            ProbVarKind::Mixed => 'M',
            ProbVarKind::Integer => 'I',
            ProbVarKind::General => 'G',
        };
        write!(f, "{c}")
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum ProbConstrKind {
    None,
    Box,
    Linear,
    DiagonalConvex,
    Convex,
    #[default]
    Quadratic,
}

impl Display for ProbConstrKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            ProbConstrKind::None => 'N',
            ProbConstrKind::Box => 'B',
            ProbConstrKind::Linear => 'L',
            ProbConstrKind::DiagonalConvex => 'D',
            ProbConstrKind::Convex => 'C',
            ProbConstrKind::Quadratic => 'Q',
        };
        write!(f, "{c}")
    }
}

impl FromStr for ProblemType {
    type Err = ParseErrorReason;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_out = || ParseErrorReason::InvalidProblemType(s.to_owned());
        let mut chars = s.chars();
        let ((o, v), c) = chars
            .next()
            .zip(chars.next())
            .zip(chars.next())
            .ok_or_else(err_out)?;
        let o = match o.to_ascii_uppercase() {
            'L' => ProbObjKind::Linear,
            'D' => ProbObjKind::DiagonalC,
            'C' => ProbObjKind::ConcaveOrConvex,
            'Q' => ProbObjKind::Quadratic,
            _ => return Err(err_out()),
        };
        let v = match v.to_ascii_uppercase() {
            'C' => ProbVarKind::Continuous,
            'B' => ProbVarKind::Binary,
            'M' => ProbVarKind::Mixed,
            'I' => ProbVarKind::Integer,
            'G' => ProbVarKind::General,
            _ => return Err(err_out()),
        };
        let c = match c.to_ascii_uppercase() {
            'N' => ProbConstrKind::None,
            'B' => ProbConstrKind::Box,
            'L' => ProbConstrKind::Linear,
            'D' => ProbConstrKind::DiagonalConvex,
            'C' => ProbConstrKind::Convex,
            'Q' => ProbConstrKind::Quadratic,
            _ => return Err(err_out()),
        };
        Ok(ProblemType(o, v, c))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ObjSense {
    #[default]
    Minimize,
    Maximize,
}

impl FromStr for ObjSense {
    type Err = ParseErrorReason;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "minimize" => Ok(Self::Minimize),
            "maximize" => Ok(Self::Maximize),
            _ => Err(ParseErrorReason::InvalidObjSense(s.to_owned())),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarType {
    #[default]
    Continuous,
    Integer,
    Binary,
}

impl FromStr for VarType {
    type Err = ParseErrorReason;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(VarType::Continuous),
            "1" => Ok(VarType::Integer),
            "2" => Ok(VarType::Binary),
            _ => Err(ParseErrorReason::InvalidVarType(s.to_owned())),
        }
    }
}

/// Checks the bounds of all integer variables in `vars`, and transforms them
/// into Binary variables if the bounds are (0, 1)
fn integer_to_binary(
    mut vars: Vec<VarType>,
    lower_bounds: &[f64],
    upper_bounds: &[f64],
) -> Vec<VarType> {
    let bounds = lower_bounds.iter().zip(upper_bounds.iter());
    for (var_type, bounds) in vars.iter_mut().zip(bounds) {
        if *var_type == VarType::Integer {
            // The paper describes the only way to fix a binary variable being
            // to define it as an integer and setting lower == upper == 1 or 0.
            // This seems inconsequential, but the metadata in qplib accounts
            // for this and our numbers of variables of each kind don't match
            // the qplib metadata for exactly _one_ instance (QPLIB_3510) if we
            // don't account for this.
            if bounds == (&0., &1.) || bounds == (&1., &1.) || bounds == (&0., &0.) {
                *var_type = VarType::Binary;
            }
        }
    }
    vars
}

struct FileCursor<T: Iterator<Item = String>> {
    inner: T,
    line_num: usize,
}

fn is_comment(s: &str) -> bool {
    s.trim().starts_with(['!', '%', '#'])
}

impl<It> FileCursor<It>
where
    It: Iterator<Item = String>,
{
    fn new(inner: It) -> Self {
        Self { inner, line_num: 0 }
    }

    fn expect_next(&mut self) -> Result<String> {
        // ignores comments & blank lines
        for s in &mut self.inner {
            self.line_num += 1;
            if !s.trim().is_empty() && !is_comment(&s) {
                return Ok(s);
            }
        }
        Err(QplibParseError::unexpected_eof(self.line_num).into())
    }

    fn parse_or_err_with_line<T, E>(&self, raw: &str) -> Result<T>
    where
        T: FromStr<Err = E>,
        E: Into<ParseErrorReason>,
    {
        raw.parse::<T>()
            .map_err(|e| e.into().with_line(self.line_num).into())
    }

    /// Consumes the next line and tries to parse the first value
    /// in it (determined by whitespace).
    fn next_parse<T, E>(&mut self) -> Result<T>
    where
        T: FromStr<Err = E>,
        E: Into<ParseErrorReason>,
    {
        let line = self.expect_next()?;
        let val = line
            .split_whitespace()
            .next()
            .ok_or(QplibParseError::invalid_line(self.line_num))?;
        self.parse_or_err_with_line(val)
    }

    fn next_split_n(&mut self, n: usize) -> Result<Vec<String>> {
        let line = self.expect_next()?;
        let parts = line
            .splitn(n, |c: char| c.is_ascii_whitespace())
            .take_while(|part| !is_comment(part))
            .map(|s| s.to_string())
            .collect();
        Ok(parts)
    }

    /// Consumes the next line, parsing it as an integer `n`, then consumes the
    /// following `n` lines as space-separated values according to `f`,
    /// then collect into a map.
    ///
    /// `f` is a function that receives the space-separated strings in the line
    /// and returns a key-value pair, or errors.
    fn consume_map<K, V, E>(
        &mut self,
        // number of "segments" to split line into.
        segments: usize,
        f: impl Fn(Vec<String>) -> Result<(K, V), ParseErrorReason>,
    ) -> Result<HashMap<K, V>>
    where
        K: Eq + std::hash::Hash,
        V: FromStr<Err = E>,
        ParseErrorReason: From<E>,
    {
        let num = self.next_parse()?;
        let mut out = HashMap::with_capacity(num);
        for _ in 0..num {
            // we add one so that comments are left a the end of the line.
            let parts = self.next_split_n(segments + 1)?;
            let (key, val) = f(parts).map_err(|e| e.with_line(self.line_num))?;
            out.insert(key, val);
        }
        Ok(out)
    }

    /// Method for reading the "non-defaults in b^0" section.
    ///
    /// Note that all `i`s are subtracted by 1 because we want things to be
    /// 0-indexed.
    fn collect_i_val<V, E>(&mut self) -> Result<HashMap<usize, V>>
    where
        V: FromStr<Err = E>,
        ParseErrorReason: From<E>,
    {
        self.consume_map(2, |parts| {
            let key = parts[0].parse::<usize>()? - 1;
            let val: V = parts[1].parse()?;
            Ok((key, val))
        })
    }

    /// Method for reading the "non-zeroes in Q^0" section.
    ///
    /// Note that all `i`s and `j`s are subtracted by 1 because we want things
    /// to be 0-indexed.
    fn collect_ij_val(&mut self) -> Result<HashMap<(usize, usize), f64>> {
        self.consume_map(3, |parts| {
            let key = (
                parts[0].parse::<usize>()? - 1,
                parts[1].parse::<usize>()? - 1,
            );
            let val = parts[2].parse()?;
            Ok((key, val))
        })
    }

    /// Consumes the next line, parsing it as an integer `n`. then consumes the
    /// following `n` lines as space-separated values according to `f`, to
    /// collect them into a `Vec<HashMap<_, _>>`
    ///
    /// `f` is a function that receives the space-separated strings in the line
    /// and returns a index-key-value triplet, or errors. "Index" is the position
    /// in the resulting vector which should be updated by that line.
    fn consume_list_of_maps<K>(
        &mut self,
        size: usize,
        // number of "segments" to split line into.
        segments: usize,
        f: impl Fn(Vec<String>) -> Result<(usize, K, f64), ParseErrorReason>,
    ) -> Result<Vec<HashMap<K, f64>>>
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let num = self.next_parse()?;
        let mut out = vec![HashMap::default(); size];
        for _ in 0..num {
            let parts = self.next_split_n(segments + 1)?;
            let (m, key, val) = f(parts).map_err(|e| e.with_line(self.line_num))?;
            out[m].insert(key, val);
        }
        Ok(out)
    }

    /// Method for reading the "non-zeroes in b^i" section.
    ///
    /// Note that indices are subtracted by 1 because we want everything to be
    /// 0-indexed.
    fn collect_list_of_i_val(&mut self, size: usize) -> Result<Vec<HashMap<usize, f64>>> {
        self.consume_list_of_maps(size, 3, |parts| {
            let m = parts[0].parse::<usize>()? - 1;
            let key = parts[1].parse::<usize>()? - 1;
            let val = parts[2].parse()?;
            Ok((m, key, val))
        })
    }

    /// Method for reading the "non-zeroes in Q^i" section".
    ///
    /// Note that indices are subtracted by 1 because we want everything to be
    /// 0-indexed.
    fn collect_list_of_ij_val(&mut self, size: usize) -> Result<Vec<HashMap<(usize, usize), f64>>> {
        self.consume_list_of_maps(size, 4, |parts| {
            let m = parts[0].parse::<usize>()? - 1;
            let key = (
                parts[1].parse::<usize>()? - 1,
                parts[2].parse::<usize>()? - 1,
            );
            let val = parts[3].parse()?;
            Ok((m, key, val))
        })
    }

    /// Method for reading basic "default / num / non_defaults..." sections.
    ///
    /// Consumes the next line, parsing it as a default value, then the
    /// following line as an integer `n`. Then consumes the following `n`
    /// lines as space-separated `(i, val)` pairs.
    ///
    /// Returns this as a vector, where positions `i-1` are filled with `val`,
    /// and all others have the default value.
    fn collect_list<V, E>(&mut self, size: usize) -> Result<Vec<V>>
    where
        V: FromStr<Err = E> + Clone,
        E: Into<ParseErrorReason>,
    {
        let default: V = self.next_parse()?;
        let mut out = vec![default; size];
        let num = self.next_parse()?;
        for _ in 0..num {
            let parts = self.next_split_n(3)?; // this is 3 because we ignore anything beyond the first 2
            let (i, val): (usize, V) = (
                self.parse_or_err_with_line(&parts[0])?,
                self.parse_or_err_with_line(&parts[1])?,
            );
            out[i - 1] = val;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::*;

    #[test]
    fn cursor_collect() -> Result<()> {
        let file = r#"!
! collect i val
2
1 1.0
2 2.0 this is ignored
! collect ij val
2
3 4 3.0
5 6 4.0 this is ignored
! collect list of i val
4
1 2 5.0
1 3 6.0
2 1 7.0
2 2 8.0
! collect list of ij val
4
1 1 1 1.0
1 1 2 2.0
2 2 1 3.0
2 2 2 4.0
! collect list
3.0 # default value
2
2 0.0
3 1.0"#;
        let mut cursor = FileCursor::new(file.lines().map(|s| s.to_owned()));
        assert_eq!(
            cursor.collect_i_val()?,
            hashmap! { 0 => 1.0, 1 => 2.0 },
            "collect_i_val"
        );
        assert_eq!(
            cursor.collect_ij_val()?,
            hashmap! { (2, 3) => 3.0, (4, 5) => 4.0 },
            "collect_ij_val"
        );
        assert_eq!(
            cursor.collect_list_of_i_val(2)?,
            vec![
                hashmap! { 1 => 5.0, 2 => 6.0 },
                hashmap! { 0 => 7.0, 1 => 8.0 },
            ],
            "collect_list_of_i_val"
        );

        assert_eq!(
            cursor.collect_list_of_ij_val(2)?,
            vec![
                hashmap! { (0, 0) => 1.0, (0, 1) => 2.0 },
                hashmap! { (1, 0) => 3.0, (1, 1) => 4.0 },
            ],
            "collect_list_of_ij_val"
        );
        assert_eq!(cursor.collect_list::<f64, _>(4)?, vec![3.0, 0.0, 1.0, 3.0]);
        Ok(())
    }

    #[test]
    fn example_problem() -> Result<()> {
        // example given in the paper
        // Furini, Fabio, et al.
        // "QPLIB: a library of quadratic programming instances."
        // Mathematical Programming Computation 11 (2019): 237-265
        // pages 42 & 43
        // https://link.springer.com/article/10.1007/s12532-018-0147-4
        let file = r#"
! ---------------
! example problem
! ---------------
MIPBAND # problem name
QML # problem is a mixed-integer quadratic program
Minimize # minimize the objective function
3 # variables
2 # general linear constraints
5 # nonzeros in lower triangle of Q^0
1 1 2.0 5 lines row & column index & value of nonzero in lower triangle Q^0
2 1 -1.0 |
2 2 2.0 |
3 2 -1.0 |
3 3 2.0 |
-0.2 default value for entries in b_0
1 # non default entries in b_0
2 -0.4 1 line of index & value of non-default values in b_0
0.0 value of q^0
4 # nonzeros in vectors b^i (i=1,...,m)
1 1 1.0 4 lines constraint, index & value of nonzero in b^i (i=1,...,m)
1 2 1.0 |
2 1 1.0 |
2 3 1.0 |
1.0E+20 infinity
1.0 default value for entries in c_l
0 # non default entries in c_l
1.0E+20 default value for entries in c_u
0 # non default entries in c_u
0.0 default value for entries in l
0 # non default entries in l
1.0 default value for entries in u
1 # non default entries in u
2 2.0 1 line of non-default indices and values in u
0 default variable type is continuous
1 # non default variable types
3 2 variable 3 is binary
1.0 default value for initial values for x
0 # non default entries in x
0.0 default value for initial values for y
0 # non default entries in y
0.0 default value for initial values for z
0 # non default entries in z
0 # non default names for variables
0 # non default names for constraints"#;
        let parsed = QplibFile::from_lines(file.lines().map(|s| s.to_string()))?;
        assert_eq!(parsed.name, "MIPBAND");
        assert_eq!(parsed.problem_type.0, ProbObjKind::Quadratic);
        assert_eq!(parsed.problem_type.1, ProbVarKind::Mixed);
        assert_eq!(parsed.problem_type.2, ProbConstrKind::Linear);
        assert_eq!(parsed.sense, ObjSense::Minimize);
        assert_eq!(parsed.num_vars, 3);
        assert_eq!(parsed.num_constraints, 2);

        // all indices (keys) should be 1 less than what is present in the file

        assert_eq!(
            parsed.q0_non_zeroes,
            hashmap! {
                (0, 0) => 2.,
                (1, 0) =>-1.,
                (1, 1) =>2.,
                (2, 1) =>-1.,
                (2, 2) =>2.,
            },
            "q0_non_zeroes"
        );
        assert_eq!(
            parsed.b0_non_defaults,
            hashmap! { 1 => -0.4 },
            "b0_non_zeroes"
        );
        assert_eq!(parsed.default_b0, -0.2, "default_b0");
        assert_eq!(parsed.obj_constant, 0.0, "obj_constant");

        // ProbConstrType is Linear, qs is skipped but bs has values
        assert_eq!(parsed.qs_non_zeroes, vec![], "qs_non_zeroes");
        assert_eq!(
            parsed.bs_non_zeroes,
            vec![hashmap! { 0 => 1., 1 => 1. }, hashmap! { 0 => 1., 2 => 1. }],
            "bs_non_zeroes"
        );

        assert_eq!(parsed.infinity_threshold, 1.0E+20_f64);

        assert_eq!(parsed.constr_lower_cs, vec![1., 1.], "constr_lower_cs");
        assert_eq!(
            parsed.constr_upper_cs,
            vec![parsed.infinity_threshold, parsed.infinity_threshold],
            "constr_upper_cs"
        );

        assert_eq!(parsed.lower_bounds, vec![0., 0., 0.], "lower_bounds");
        assert_eq!(parsed.upper_bounds, vec![1., 2., 1.], "upper_bounds");
        assert_eq!(
            parsed.var_types,
            vec![VarType::Continuous, VarType::Continuous, VarType::Binary]
        );

        Ok(())
    }
}
