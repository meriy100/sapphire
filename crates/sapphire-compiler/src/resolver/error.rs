//! Resolver errors.
//!
//! This module defines the ADT surfaced by the I5 name-resolution
//! pass. Each variant of [`ResolveErrorKind`] corresponds to one of
//! the static errors listed in spec 08 §Visibility and §Name
//! resolution: duplicate top-level names, undefined references,
//! ambiguous imports, private-type leaks, unknown imports, and so
//! on.
//!
//! The layering mirrors the parser's `ParseError` shape so that the
//! L2 diagnostic layer can pattern-match on a closed enum.

use std::fmt;

use sapphire_core::span::Span;

/// One name-resolution error, with a span into the source of the
/// *erroring* site (not necessarily the site that introduced the
/// conflicting name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    pub kind: ResolveErrorKind,
    pub span: Span,
}

impl ResolveError {
    pub const fn new(kind: ResolveErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The classification of resolver errors.
///
/// Kept as a closed enum so downstream diagnostic code can render
/// per-variant messages. String payloads carry the affected identifier
/// (and, where applicable, the names of the modules that brought
/// conflicting definitions into scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveErrorKind {
    /// A top-level declaration re-uses a name already bound at the
    /// module's top level. Spec 08 §Visibility: a module's top-level
    /// environment must have unique names per namespace.
    DuplicateTopLevel { name: String, namespace: Namespace },
    /// A bare identifier at a use site cannot be found in any scope
    /// (local, top-level, imported, or prelude).
    UndefinedName { name: String, namespace: Namespace },
    /// A `M.x` qualified reference names a module that is not in
    /// scope.
    UnknownModule { name: String },
    /// A `M.x` qualified reference names a module that is in scope,
    /// but that module does not export `x` (in the expected
    /// namespace).
    NotExported {
        module: String,
        name: String,
        namespace: Namespace,
    },
    /// Two unqualified imports expose the same name, and a use site
    /// references it bare.
    Ambiguous {
        name: String,
        namespace: Namespace,
        modules: Vec<String>,
    },
    /// The module header's export list refers to a name not defined
    /// in the module.
    ExportOfUnknown { name: String, namespace: Namespace },
    /// An `import M (x)` / `import M hiding (x)` item names something
    /// that `M` does not export.
    ImportOfUnknown {
        module: String,
        name: String,
        namespace: Namespace,
    },
    /// An `import M` refers to a module not present in the
    /// compilation.
    ImportOfUnknownModule { module: String },
    /// Spec 08 §Visibility: an exported top-level signature mentions
    /// a private type. `leak` is the private type's name; `public` is
    /// the public binding that leaks it.
    PrivateLeak {
        public: String,
        leak: String,
        namespace: Namespace,
    },
    /// Spec 08 §One module per file: a single-file script implicitly
    /// becomes `module Main`, but any file imported by another module
    /// must carry an explicit header.
    MainSugarNotImportable { module: String },
    /// Spec 08 §Cyclic imports: the import graph must be acyclic.
    /// `cycle` lists the modules on the detected cycle in
    /// declaration order.
    CyclicImports { cycle: Vec<String> },
    /// `import M (x)` and `import M hiding (y)` cannot be combined
    /// with other forms. This variant flags a malformed AST shape
    /// that should already have been rejected by the parser; kept
    /// here as a defence-in-depth.
    MalformedImport,
    /// A module references a name it does not have access to via any
    /// import path (qualified form with no matching `as` alias).
    QualifierNotInScope { qualifier: String },
}

/// Spec 08 names live in one of two namespaces. Record-field names
/// live in their own selection-syntax world and are deliberately not
/// represented here (04 §Design notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Namespace {
    /// Values, functions, value constructors, class methods,
    /// Ruby-embedded bindings.
    Value,
    /// Type constructors, type aliases, class names.
    Type,
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Namespace::Value => f.write_str("value"),
            Namespace::Type => f.write_str("type"),
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "resolve error at bytes {}..{}: {}",
            self.span.start, self.span.end, self.kind
        )
    }
}

impl fmt::Display for ResolveErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveErrorKind::DuplicateTopLevel { name, namespace } => {
                write!(f, "duplicate {namespace} binding `{name}` at top level")
            }
            ResolveErrorKind::UndefinedName { name, namespace } => {
                write!(f, "undefined {namespace} name `{name}`")
            }
            ResolveErrorKind::UnknownModule { name } => {
                write!(f, "no module named `{name}` is in scope")
            }
            ResolveErrorKind::NotExported {
                module,
                name,
                namespace,
            } => {
                write!(
                    f,
                    "module `{module}` does not export {namespace} name `{name}`"
                )
            }
            ResolveErrorKind::Ambiguous {
                name,
                namespace,
                modules,
            } => {
                write!(
                    f,
                    "{namespace} name `{name}` is ambiguous — imported from {}",
                    modules.join(", ")
                )
            }
            ResolveErrorKind::ExportOfUnknown { name, namespace } => {
                write!(
                    f,
                    "export list mentions {namespace} name `{name}` which is not declared in this module"
                )
            }
            ResolveErrorKind::ImportOfUnknown {
                module,
                name,
                namespace,
            } => {
                write!(
                    f,
                    "import list mentions {namespace} name `{name}` not exported by `{module}`"
                )
            }
            ResolveErrorKind::ImportOfUnknownModule { module } => {
                write!(f, "cannot import `{module}`: module not found")
            }
            ResolveErrorKind::PrivateLeak {
                public,
                leak,
                namespace,
            } => {
                write!(
                    f,
                    "exported binding `{public}` leaks private {namespace} `{leak}`"
                )
            }
            ResolveErrorKind::MainSugarNotImportable { module } => {
                write!(
                    f,
                    "`{module}` is a header-less script and cannot be imported (spec 08 §One module per file)"
                )
            }
            ResolveErrorKind::CyclicImports { cycle } => {
                write!(f, "cyclic imports: {}", cycle.join(" -> "))
            }
            ResolveErrorKind::MalformedImport => f.write_str("malformed import declaration"),
            ResolveErrorKind::QualifierNotInScope { qualifier } => {
                write!(f, "module qualifier `{qualifier}` is not in scope")
            }
        }
    }
}

impl std::error::Error for ResolveError {}
