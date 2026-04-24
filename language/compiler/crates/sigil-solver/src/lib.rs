use std::collections::BTreeMap;

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum SymbolPathStep {
    Binding(String),
    Field(String),
    ListHead,
    ListTail,
    TupleIndex(usize),
    VariantField(usize),
    Length,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct SymbolPath(pub Vec<SymbolPathStep>);

impl SymbolPath {
    pub fn root(name: &str) -> Self {
        Self(vec![SymbolPathStep::Binding(name.to_string())])
    }

    pub fn field(&self, field: &str) -> Self {
        let mut parts = self.0.clone();
        parts.push(SymbolPathStep::Field(field.to_string()));
        Self(parts)
    }

    pub fn list_head(&self) -> Self {
        let mut parts = self.0.clone();
        parts.push(SymbolPathStep::ListHead);
        Self(parts)
    }

    pub fn list_tail(&self) -> Self {
        let mut parts = self.0.clone();
        parts.push(SymbolPathStep::ListTail);
        Self(parts)
    }

    pub fn tuple_index(&self, index: usize) -> Self {
        let mut parts = self.0.clone();
        parts.push(SymbolPathStep::TupleIndex(index));
        Self(parts)
    }

    pub fn variant_field(&self, index: usize) -> Self {
        let mut parts = self.0.clone();
        parts.push(SymbolPathStep::VariantField(index));
        Self(parts)
    }

    pub fn length(&self) -> Self {
        let mut parts = self.0.clone();
        parts.push(SymbolPathStep::Length);
        Self(parts)
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for (index, part) in self.0.iter().enumerate() {
            match part {
                SymbolPathStep::Binding(name) => out.push_str(name),
                SymbolPathStep::Field(name) => {
                    if index > 0 {
                        out.push('.');
                    }
                    out.push_str(name);
                }
                SymbolPathStep::ListHead => out.push_str(".__head"),
                SymbolPathStep::ListTail => out.push_str(".__tail"),
                SymbolPathStep::TupleIndex(value) => out.push_str(&format!(".__tuple{}", value)),
                SymbolPathStep::VariantField(value) => {
                    out.push_str(&format!(".__variant{}", value))
                }
                SymbolPathStep::Length => out.push_str(".__len"),
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct LinearForm {
    pub terms: BTreeMap<SymbolPath, i64>,
}

impl LinearForm {
    pub fn zero() -> Self {
        Self {
            terms: BTreeMap::new(),
        }
    }

    pub fn from_path(path: SymbolPath) -> Self {
        let mut terms = BTreeMap::new();
        terms.insert(path, 1);
        Self { terms }
    }

    pub fn add_scaled(&mut self, other: &Self, scale: i64) {
        for (path, coeff) in &other.terms {
            let next = self.terms.get(path).copied().unwrap_or(0) + (*coeff * scale);
            if next == 0 {
                self.terms.remove(path);
            } else {
                self.terms.insert(path.clone(), next);
            }
        }
    }

    pub fn single_term(&self) -> Option<(&SymbolPath, i64)> {
        if self.terms.len() == 1 {
            self.terms.iter().next().map(|(path, coeff)| (path, *coeff))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinearExpr {
    pub form: LinearForm,
    pub constant: i64,
}

impl LinearExpr {
    pub fn int(value: i64) -> Self {
        Self {
            form: LinearForm::zero(),
            constant: value,
        }
    }

    pub fn from_path(path: SymbolPath) -> Self {
        Self {
            form: LinearForm::from_path(path),
            constant: 0,
        }
    }

    pub fn add(&self, other: &Self) -> Self {
        let mut form = self.form.clone();
        form.add_scaled(&other.form, 1);
        Self {
            form,
            constant: self.constant + other.constant,
        }
    }

    pub fn subtract(&self, other: &Self) -> Self {
        let mut form = self.form.clone();
        form.add_scaled(&other.form, -1);
        Self {
            form,
            constant: self.constant - other.constant,
        }
    }

    pub fn negate(&self) -> Self {
        let mut form = LinearForm::zero();
        form.add_scaled(&self.form, -1);
        Self {
            form,
            constant: -self.constant,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Atom {
    IntCmp {
        form: LinearForm,
        op: ComparisonOp,
        rhs: i64,
    },
    BoolEq {
        path: SymbolPath,
        value: bool,
    },
    /// Protocol state equality: handle is in the given state (encoded as integer index).
    /// `state_index` is the 0-based index of the state in the protocol's sorted state list.
    StateEq {
        path: SymbolPath,
        state_index: i64,
        protocol: String,
    },
}

impl Atom {
    pub fn negate(&self) -> Self {
        match self {
            Self::IntCmp { form, op, rhs } => Self::IntCmp {
                form: form.clone(),
                op: match op {
                    ComparisonOp::Eq => ComparisonOp::Ne,
                    ComparisonOp::Ne => ComparisonOp::Eq,
                    ComparisonOp::Lt => ComparisonOp::Ge,
                    ComparisonOp::Le => ComparisonOp::Gt,
                    ComparisonOp::Gt => ComparisonOp::Le,
                    ComparisonOp::Ge => ComparisonOp::Lt,
                },
                rhs: *rhs,
            },
            Self::BoolEq { path, value } => Self::BoolEq {
                path: path.clone(),
                value: !value,
            },
            Self::StateEq {
                path,
                state_index,
                protocol: _,
            } => Self::IntCmp {
                form: LinearForm::from_path(path.clone()),
                op: ComparisonOp::Ne,
                rhs: *state_index,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Formula {
    True,
    False,
    Atom(Atom),
    And(Vec<Formula>),
    Or(Vec<Formula>),
    Not(Box<Formula>),
}

pub fn formula_and(parts: Vec<Formula>) -> Formula {
    let mut flattened = Vec::new();
    for part in parts {
        match part {
            Formula::True => {}
            Formula::False => return Formula::False,
            Formula::And(items) => flattened.extend(items),
            other => flattened.push(other),
        }
    }

    match flattened.len() {
        0 => Formula::True,
        1 => flattened.into_iter().next().unwrap(),
        _ => Formula::And(flattened),
    }
}

pub fn formula_or(parts: Vec<Formula>) -> Formula {
    let mut flattened = Vec::new();
    for part in parts {
        match part {
            Formula::False => {}
            Formula::True => return Formula::True,
            Formula::Or(items) => flattened.extend(items),
            other => flattened.push(other),
        }
    }

    match flattened.len() {
        0 => Formula::False,
        1 => flattened.into_iter().next().unwrap(),
        _ => Formula::Or(flattened),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SolverOutcome {
    Proved,
    Refuted {
        model: BTreeMap<String, serde_json::Value>,
    },
    Unknown {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProofCheck {
    pub assumptions: Vec<Formula>,
    pub goal: Formula,
    pub outcome: SolverOutcome,
}

pub fn prove_formula(assumptions: &[Formula], goal: &Formula) -> ProofCheck {
    let solver = z3::Solver::new();
    let registry = SymbolRegistry::from_formulas(assumptions, goal);

    for assumption in assumptions {
        solver.assert(lower_formula(assumption, &registry));
    }
    solver.assert(lower_formula(
        &Formula::Not(Box::new(goal.clone())),
        &registry,
    ));

    let outcome = match solver.check() {
        z3::SatResult::Unsat => SolverOutcome::Proved,
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .map(|model| registry.extract_model(&model))
                .unwrap_or_default();
            SolverOutcome::Refuted { model }
        }
        z3::SatResult::Unknown => SolverOutcome::Unknown {
            reason: solver
                .get_reason_unknown()
                .unwrap_or_else(|| "solver returned unknown".to_string()),
        },
    };

    ProofCheck {
        assumptions: assumptions.to_vec(),
        goal: goal.clone(),
        outcome,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymbolSort {
    Bool,
    Int,
}

struct SymbolRegistry {
    sorts: BTreeMap<SymbolPath, SymbolSort>,
}

impl SymbolRegistry {
    fn from_formulas(assumptions: &[Formula], goal: &Formula) -> Self {
        let mut sorts = BTreeMap::new();
        for formula in assumptions.iter().chain(std::iter::once(goal)) {
            collect_formula_sorts(formula, &mut sorts);
        }
        Self { sorts }
    }

    fn bool_const(&self, path: &SymbolPath) -> z3::ast::Bool {
        z3::ast::Bool::new_const(path.render())
    }

    fn int_const(&self, path: &SymbolPath) -> z3::ast::Int {
        z3::ast::Int::new_const(path.render())
    }

    fn extract_model(&self, model: &z3::Model) -> BTreeMap<String, serde_json::Value> {
        let mut values = BTreeMap::new();
        for (path, sort) in &self.sorts {
            let key = path.render();
            let value = match sort {
                SymbolSort::Bool => model
                    .eval(&self.bool_const(path), true)
                    .and_then(|value| value.as_bool())
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null),
                SymbolSort::Int => model
                    .eval(&self.int_const(path), true)
                    .and_then(|value| value.as_i64())
                    .map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
            };
            values.insert(key, value);
        }
        values
    }
}

fn collect_formula_sorts(formula: &Formula, out: &mut BTreeMap<SymbolPath, SymbolSort>) {
    match formula {
        Formula::True | Formula::False => {}
        Formula::Atom(atom) => collect_atom_sorts(atom, out),
        Formula::And(parts) | Formula::Or(parts) => {
            for part in parts {
                collect_formula_sorts(part, out);
            }
        }
        Formula::Not(inner) => collect_formula_sorts(inner, out),
    }
}

fn collect_atom_sorts(atom: &Atom, out: &mut BTreeMap<SymbolPath, SymbolSort>) {
    match atom {
        Atom::BoolEq { path, .. } => {
            out.insert(path.clone(), SymbolSort::Bool);
        }
        Atom::IntCmp { form, .. } => {
            for path in form.terms.keys() {
                out.insert(path.clone(), SymbolSort::Int);
            }
        }
        Atom::StateEq { path, .. } => {
            out.insert(path.clone(), SymbolSort::Int);
        }
    }
}

fn lower_formula(formula: &Formula, registry: &SymbolRegistry) -> z3::ast::Bool {
    match formula {
        Formula::True => z3::ast::Bool::from_bool(true),
        Formula::False => z3::ast::Bool::from_bool(false),
        Formula::Atom(atom) => lower_atom(atom, registry),
        Formula::And(parts) => {
            let lowered = parts
                .iter()
                .map(|part| lower_formula(part, registry))
                .collect::<Vec<_>>();
            let refs = lowered.iter().collect::<Vec<_>>();
            z3::ast::Bool::and(&refs)
        }
        Formula::Or(parts) => {
            let lowered = parts
                .iter()
                .map(|part| lower_formula(part, registry))
                .collect::<Vec<_>>();
            let refs = lowered.iter().collect::<Vec<_>>();
            z3::ast::Bool::or(&refs)
        }
        Formula::Not(inner) => lower_formula(inner, registry).not(),
    }
}

fn lower_atom(atom: &Atom, registry: &SymbolRegistry) -> z3::ast::Bool {
    match atom {
        Atom::BoolEq { path, value } => {
            let expr = registry.bool_const(path);
            if *value {
                expr
            } else {
                expr.not()
            }
        }
        Atom::IntCmp { form, op, rhs } => {
            let expr = lower_linear_form(form, registry);
            let rhs_expr = z3::ast::Int::from_i64(*rhs);
            match op {
                ComparisonOp::Eq => expr.eq(&rhs_expr),
                ComparisonOp::Ne => expr.eq(&rhs_expr).not(),
                ComparisonOp::Lt => expr.lt(&rhs_expr),
                ComparisonOp::Le => expr.le(&rhs_expr),
                ComparisonOp::Gt => expr.gt(&rhs_expr),
                ComparisonOp::Ge => expr.ge(&rhs_expr),
            }
        }
        Atom::StateEq { path, state_index, .. } => {
            let expr = registry.int_const(path);
            let rhs_expr = z3::ast::Int::from_i64(*state_index);
            expr.eq(&rhs_expr)
        }
    }
}

fn lower_linear_form(form: &LinearForm, registry: &SymbolRegistry) -> z3::ast::Int {
    let mut terms = Vec::new();
    for (path, coeff) in &form.terms {
        let variable = registry.int_const(path);
        let term = if *coeff == 1 {
            variable
        } else {
            z3::ast::Int::from_i64(*coeff) * variable
        };
        terms.push(term);
    }

    if terms.is_empty() {
        z3::ast::Int::from_i64(0)
    } else {
        let refs = terms.iter().collect::<Vec<_>>();
        z3::ast::Int::add(&refs)
    }
}
