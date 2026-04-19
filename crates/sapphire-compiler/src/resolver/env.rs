//! Resolved-reference and module-environment types.
//!
//! This module fixes the *shape* of resolution results. A parsed
//! identifier lands in one of three buckets:
//!
//! 1. A **local** binding — introduced by a `\x -> ...`, `let`,
//!    pattern, or `do` bind. Locals have no home module; they are
//!    scoped to the enclosing expression.
//! 2. A **top-level** binding in some module — either the same
//!    module or one imported. Represented by [`ResolvedRef`].
//! 3. A **built-in**. Spec 09 `Int` / `String` / the `Ruby` type
//!    carrier are modelled as prelude exports so the table lookup
//!    path handles them uniformly; no extra case is needed here.
//!
//! See `docs/impl/15-resolver.md` for the rationale on keeping the
//! original AST untouched and emitting resolution information in
//! side tables keyed on [`ResolvedRef::span`].

use std::collections::HashMap;

use sapphire_core::span::Span;

use super::error::Namespace;

/// A fully-qualified module identity. Identical layout to
/// `ast::ModName` but without the span — modules are compared by
/// dotted segment chain, never by where they were written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId {
    pub segments: Vec<String>,
}

impl ModuleId {
    pub fn from_segments(segments: &[String]) -> Self {
        Self {
            segments: segments.to_vec(),
        }
    }

    pub fn display(&self) -> String {
        self.segments.join(".")
    }
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

/// A resolved reference: the module that defines the name, the
/// name itself, and which namespace it was looked up in.
///
/// Two [`ResolvedRef`]s are equal iff they point at the same
/// definition site — this is the payoff of the spec-08 rule that
/// every binding has exactly one home module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedRef {
    pub module: ModuleId,
    pub name: String,
    pub namespace: Namespace,
}

impl ResolvedRef {
    pub fn new(module: ModuleId, name: impl Into<String>, namespace: Namespace) -> Self {
        Self {
            module,
            name: name.into(),
            namespace,
        }
    }
}

/// How a definition is visible outside the defining module.
///
/// For `data T = C1 | C2` with export item `T(..)`, the type `T`
/// carries `Visibility::Exported` and every constructor does too.
/// For a bare `T` export, `T` is exported but `C1` / `C2` stay
/// `Private`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Exported,
    Private,
}

/// A single top-level definition discovered in a module, kept
/// indexed by namespace.
///
/// `data T = C a` registers the type `T` (namespace `Type`) and the
/// constructor `C` (namespace `Value`). Class declarations register
/// the class name (`Type`) and any methods exported via `class C(..)`
/// (`Value`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelDef {
    pub name: String,
    pub namespace: Namespace,
    pub visibility: Visibility,
    pub kind: DefKind,
    /// Span of the declaration's header (e.g. the `data` keyword
    /// position). Used for duplicate-error reporting.
    pub span: Span,
}

/// What flavour of declaration produced this top-level definition.
///
/// Keeps a little more structure than a plain "it's a value"
/// so that visibility checks know, e.g., which value names are
/// constructors (needed for 08 §Visibility "bare `Maybe` exports the
/// type but not the constructors").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefKind {
    /// A value binding (`foo x = ...`) or signature (`foo : ...`).
    Value,
    /// A value constructor of a `data` declaration.
    Ctor { parent_type: String },
    /// A class method (ordinary or declared via `class C where m : ...`).
    ClassMethod { parent_class: String },
    /// A Ruby-embedded binding (`name x := "..."`).
    RubyEmbed,
    /// A `data` type constructor.
    DataType,
    /// A `type T = τ` transparent alias.
    TypeAlias,
    /// A type-class name.
    Class,
}

/// A module's export surface as computed by the resolver.
///
/// `values` and `types` are keyed by the exported name (unqualified).
/// The [`ResolvedRef`] points at the binding's home module, which may
/// be the module itself (ordinary export) or — once selective
/// re-export (08-OQ3) lands — some other module.
#[derive(Debug, Clone, Default)]
pub struct Exports {
    pub values: HashMap<String, ResolvedRef>,
    pub types: HashMap<String, ResolvedRef>,
    /// Ctor name → parent data-type name, for all *exported* ctors.
    /// Populated by `compute_exports`. Used to filter `T(..)` imports
    /// so that `import M (T(..))` brings only `T`'s constructors —
    /// not unrelated values from `M` (must-fix from I5 review iter 1).
    pub ctor_parents: HashMap<String, String>,
    /// Method name → parent class name, for all *exported* methods.
    /// Symmetric to `ctor_parents` for class methods.
    pub method_parents: HashMap<String, String>,
}

impl Exports {
    pub fn lookup(&self, name: &str, ns: Namespace) -> Option<&ResolvedRef> {
        match ns {
            Namespace::Value => self.values.get(name),
            Namespace::Type => self.types.get(name),
        }
    }

    pub fn insert(&mut self, name: String, ns: Namespace, r: ResolvedRef) {
        match ns {
            Namespace::Value => {
                self.values.insert(name, r);
            }
            Namespace::Type => {
                self.types.insert(name, r);
            }
        }
    }
}

/// A module's full internal environment — every top-level name
/// (exported or private) with its resolved identity.
///
/// The resolver builds this in two phases: first a `data` / `class`
/// / `value` / `type` scan populates [`ModuleEnv::top_level`], then
/// the import pass populates [`ModuleEnv::unqualified`] and
/// [`ModuleEnv::qualified_aliases`] based on what each `import`
/// brings in.
#[derive(Debug, Clone)]
pub struct ModuleEnv {
    pub id: ModuleId,
    /// All top-level definitions in this module.
    pub top_level: Vec<TopLevelDef>,
    /// Quick lookup for top-level names by `(name, namespace)`.
    pub top_level_index: HashMap<(String, Namespace), usize>,
    /// Names available unqualified in this module: locals shadow
    /// these, but the base lookup goes through here. A single name
    /// may resolve to multiple definitions when two imports expose
    /// it — the resolver emits an `Ambiguous` error at the use site.
    pub unqualified: HashMap<(String, Namespace), Vec<ResolvedRef>>,
    /// `import X as Y` or `import X` — every qualifier string that
    /// resolves to an actual module's exports. A module is always
    /// in scope under its full dotted name.
    pub qualified_aliases: HashMap<String, ModuleId>,
    /// The exports this module provides to the rest of the program.
    pub exports: Exports,
}

impl ModuleEnv {
    pub fn new(id: ModuleId) -> Self {
        Self {
            id,
            top_level: Vec::new(),
            top_level_index: HashMap::new(),
            unqualified: HashMap::new(),
            qualified_aliases: HashMap::new(),
            exports: Exports::default(),
        }
    }

    /// Look up a top-level definition in the defining module.
    pub fn top_lookup(&self, name: &str, ns: Namespace) -> Option<&TopLevelDef> {
        self.top_level_index
            .get(&(name.to_string(), ns))
            .map(|i| &self.top_level[*i])
    }
}
