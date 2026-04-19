//! Type environment, class environment, and instance environment.
//!
//! The type environment maps every resolved global name and every
//! local in scope to a [`Scheme`]. Globals are keyed by fully
//! qualified `(module, name)` pairs so that re-exports and aliased
//! imports do not create duplicate slots.
//!
//! The class and instance environments implement the Haskell-98 class
//! system described in spec 07. Each class tracks its superclasses and
//! method signatures; each class-instance pair tracks the context and
//! dictionary implementation.

use std::collections::HashMap;

use super::ty::{Constraint, Scheme, Ty, TyVar};

/// A global identity used as a type-env key.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GlobalId {
    pub module: String,
    pub name: String,
}

impl GlobalId {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
        }
    }
}

/// Data constructor metadata.
#[derive(Debug, Clone)]
pub struct CtorInfo {
    pub type_name: String,
    /// The ctor's type scheme: `forall a b. τ₁ -> τ₂ -> ... -> T a b`.
    pub scheme: Scheme,
    /// Number of value arguments the ctor consumes (for arity checks).
    pub arity: usize,
}

/// Data-type metadata.
#[derive(Debug, Clone)]
pub struct DataInfo {
    pub name: String,
    /// Type-parameter names, in declaration order.
    pub params: Vec<String>,
    /// Each constructor's name (for coverage / lookup).
    pub ctor_names: Vec<String>,
}

/// A type-alias entry.
#[derive(Debug, Clone)]
pub struct AliasInfo {
    pub name: String,
    pub params: Vec<String>,
    /// The RHS type with param names as `Ty::Var(TyVar { name, id=0 })`
    /// — substituted at use sites.
    pub body: Ty,
}

/// Class metadata.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub type_var: String,
    /// Superclasses (`class Eq a => Ord a` records `["Eq"]` here).
    pub superclasses: Vec<String>,
    /// Method name → its class-relative scheme, WITHOUT the
    /// `C a =>` constraint (we add it on use).
    pub methods: HashMap<String, Scheme>,
    /// Methods that have default implementations (just the names;
    /// the implementation body is in the AST).
    pub defaults: Vec<String>,
}

/// One instance declaration.
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    pub class: String,
    pub context: Vec<Constraint>,
    /// The instance head: the type applied to the class.
    pub head: Ty,
    /// Free variables of `head` (fresh TyVar ids rewritten at match
    /// time).
    pub vars: Vec<TyVar>,
    /// Which methods the instance explicitly defines.
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ClassEnv {
    pub classes: HashMap<String, ClassInfo>,
    pub instances: Vec<InstanceInfo>,
}

impl ClassEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_class(&mut self, c: ClassInfo) {
        self.classes.insert(c.name.clone(), c);
    }

    pub fn register_instance(&mut self, i: InstanceInfo) {
        self.instances.push(i);
    }

    pub fn instances_for<'a>(&'a self, class: &str) -> impl Iterator<Item = &'a InstanceInfo> {
        self.instances.iter().filter(move |i| i.class == class)
    }

    /// Collect the transitive set of superclasses for `class`,
    /// including `class` itself.
    pub fn super_closure(&self, class: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![class.to_string()];
        while let Some(c) = stack.pop() {
            if out.iter().any(|x: &String| x == &c) {
                continue;
            }
            if let Some(info) = self.classes.get(&c) {
                for s in &info.superclasses {
                    stack.push(s.clone());
                }
            }
            out.push(c);
        }
        out
    }
}

/// The global + local type environment.
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    /// Global schemes keyed by `(module, name)`.
    pub globals: HashMap<GlobalId, Scheme>,
    /// Lexically-scoped locals (stacked via `push` / `pop`).
    pub locals: Vec<HashMap<String, Scheme>>,
    /// Data-type registry (by name).
    pub datas: HashMap<String, DataInfo>,
    /// Ctor registry (by name). Constructors are global per spec 08.
    pub ctors: HashMap<String, CtorInfo>,
    /// Type-alias registry (by name).
    pub aliases: HashMap<String, AliasInfo>,
    /// Type-level variable bindings (in a scheme's scope).
    pub type_locals: Vec<HashMap<String, TyVar>>,
    /// Type-constructor kinds (name → kind string, informational).
    pub type_kinds: HashMap<String, u32>, // arity
}

impl TypeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_locals(&mut self) {
        self.locals.push(HashMap::new());
    }

    pub fn pop_locals(&mut self) {
        self.locals.pop();
    }

    pub fn bind_local(&mut self, name: impl Into<String>, scheme: Scheme) {
        if let Some(top) = self.locals.last_mut() {
            top.insert(name.into(), scheme);
        } else {
            // No frame yet — open one.
            let mut m = HashMap::new();
            m.insert(name.into(), scheme);
            self.locals.push(m);
        }
    }

    pub fn lookup_local(&self, name: &str) -> Option<&Scheme> {
        for frame in self.locals.iter().rev() {
            if let Some(s) = frame.get(name) {
                return Some(s);
            }
        }
        None
    }

    pub fn push_type_locals(&mut self) {
        self.type_locals.push(HashMap::new());
    }

    pub fn pop_type_locals(&mut self) {
        self.type_locals.pop();
    }

    pub fn bind_type_local(&mut self, name: impl Into<String>, v: TyVar) {
        if let Some(top) = self.type_locals.last_mut() {
            top.insert(name.into(), v);
        }
    }

    pub fn lookup_type_local(&self, name: &str) -> Option<&TyVar> {
        for frame in self.type_locals.iter().rev() {
            if let Some(v) = frame.get(name) {
                return Some(v);
            }
        }
        None
    }
}
