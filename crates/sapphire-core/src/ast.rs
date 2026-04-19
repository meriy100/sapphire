//! Sapphire surface AST.
//!
//! The concrete syntax described by `docs/spec/01-core-expressions.md`
//! through `docs/spec/10-ruby-interop.md` is parsed by
//! `sapphire-compiler::parser` into the tree in this module. The AST
//! lives under `sapphire-core` (rather than in the compiler crate
//! directly) so that `sapphire-lsp` and any other downstream consumer
//! can share the same nodes without pulling in the compiler
//! pipeline. I-OQ2 (parser strategy) and the decision to house the
//! AST here are recorded in `docs/impl/13-parser.md`.
//!
//! ## What this module does *not* encode
//!
//! - **Name resolution / scoping.** Identifiers are kept as raw
//!   strings with the lexical split between `lower_ident` (values,
//!   field names, type variables) and `upper_ident` (type / value
//!   constructors, module segments). Which one a particular name
//!   refers to is I5's job.
//! - **Types.** `Type` carries the parsed shape only; kind checking
//!   and any structural equivalence live downstream (I6).
//! - **Desugaring.** Surface sugar such as list-literal expressions
//!   (`[x, y, z]`), `if ... then ... else ...`, and the `do`
//!   notation is preserved verbatim so the downstream pipeline sees
//!   the same surface the programmer wrote. The parser does not
//!   unfold `[x, y, z]` into `Cons x (Cons y Nil)` — that is the
//!   elaboration stage's choice.
//! - **Diagnostics.** Parse errors live in the compiler crate; this
//!   module just holds the successfully-parsed tree.
//!
//! Every node carries a [`Span`] pointing back into the original
//! source. Synthetic nodes introduced by later passes may carry
//! empty spans ([`Span::empty`]).

use crate::span::Span;

// ===================================================================
//  Module header and top-level declarations
// ===================================================================

/// A complete parsed Sapphire source file.
///
/// `header` is `None` when the file is a single-file script (no
/// `module Foo where` at the top). The spec 08 rule that such files
/// desugar to `module Main where` is a resolution-layer concern, not
/// a parse-layer one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub header: Option<ModuleHeader>,
    pub imports: Vec<ImportDecl>,
    pub decls: Vec<Decl>,
    pub span: Span,
}

/// `module Foo.Bar (exports) where` — the optional top-of-file
/// header. The export list is represented as `None` when absent (the
/// "everything is exported" default from spec 08 §Visibility), and
/// as `Some(vec)` when present; an explicit empty export list
/// (`module M () where ...`) is `Some(vec![])`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleHeader {
    pub name: ModName,
    pub exports: Option<Vec<ExportItem>>,
    pub span: Span,
}

/// A dotted module name `Foo.Bar.Baz`. Each segment is an
/// `upper_ident` per spec 02 / 08.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModName {
    pub segments: Vec<String>,
    pub span: Span,
}

/// One entry in a module export list (spec 08 §Abstract syntax).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportItem {
    /// `foo` — export a value-level binding (`lower_ident`) or a
    /// parenthesised operator (`(+)`, `(>>=)`). The `operator` flag
    /// distinguishes the two surface forms.
    Value {
        name: String,
        operator: bool,
        span: Span,
    },
    /// `Maybe` — export a type name (but not its constructors).
    Type { name: String, span: Span },
    /// `Maybe(..)` — export a type with all its constructors.
    TypeAll { name: String, span: Span },
    /// `Maybe(Just, Nothing)` — export a type with selected
    /// constructors.
    TypeWith {
        name: String,
        ctors: Vec<String>,
        span: Span,
    },
    /// `class Eq` — export a class name, but not its methods.
    Class { name: String, span: Span },
    /// `class Eq(..)` — export a class and all its methods.
    ClassAll { name: String, span: Span },
    /// `module Foo.Bar` — re-export a whole module.
    ReExport { name: ModName, span: Span },
}

/// `import Foo.Bar (items)` / `import qualified Foo.Bar as B` etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDecl {
    pub name: ModName,
    pub qualified: bool,
    pub alias: Option<ModName>,
    pub items: ImportItems,
    pub span: Span,
}

/// What names an `import` brings into unqualified scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportItems {
    /// No parenthesised list and no `hiding` — default "everything
    /// exported by the target module" (spec 08).
    All,
    /// `import M (foo, Bar(..))` — restricted list.
    Only(Vec<ImportItem>),
    /// `import M hiding (foo, bar)` — everything except these.
    Hiding(Vec<ImportItem>),
}

/// One entry in an `import`'s explicit item list. Mirrors
/// [`ExportItem`] minus the module re-export form (which is not
/// admitted inside an import list per spec 08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportItem {
    Value {
        name: String,
        operator: bool,
        span: Span,
    },
    Type {
        name: String,
        span: Span,
    },
    TypeAll {
        name: String,
        span: Span,
    },
    TypeWith {
        name: String,
        ctors: Vec<String>,
        span: Span,
    },
    Class {
        name: String,
        span: Span,
    },
    ClassAll {
        name: String,
        span: Span,
    },
}

// ===================================================================
//  Top-level declarations
// ===================================================================

/// Top-level declarations other than `import` (which is lifted into
/// [`Module::imports`] for convenience even though spec 08 places
/// `import_decl` within `decl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `name : scheme` — top-level type signature.
    Signature {
        name: String,
        /// `true` when the name was written as `(+)` etc.
        operator: bool,
        scheme: Scheme,
        span: Span,
    },
    /// `name pat... = expr` — value definition, possibly with
    /// argument patterns (spec 07's `clause` form generalised to
    /// top-level, see 07 §Abstract syntax).
    Value(ValueClause),
    /// `data T a b = C1 τ... | C2 τ...`.
    Data(DataDecl),
    /// `type T a b = τ` — transparent alias (spec 09 §Type aliases).
    TypeAlias(TypeAlias),
    /// `class C a where ...`.
    Class(ClassDecl),
    /// `instance C T where ...`.
    Instance(InstanceDecl),
    /// `name pat... := ruby_string` — Ruby embedding (spec 10).
    RubyEmbed(RubyEmbedDecl),
}

/// A single `name pat... = expr` clause. Multiple `ValueClause`
/// decls with the same `name` in sequence are equivalent to a
/// multi-clause function definition; the parser does not merge
/// them (that is an elaboration step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueClause {
    pub name: String,
    /// `true` for `(op) x y = ...` / infix-LHS `x op y = ...`
    /// clauses. The elaborator uses this to recognise operator
    /// methods.
    pub operator: bool,
    pub params: Vec<Pattern>,
    pub body: Expr,
    pub span: Span,
}

/// `data T a₁ ... aₙ = C₁ τ₁₁ ... | ... | Cₘ ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDecl {
    pub name: String,
    pub type_params: Vec<String>,
    pub ctors: Vec<DataCtor>,
    pub span: Span,
}

/// One `|`-separated alternative of a `data` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCtor {
    pub name: String,
    /// Each argument is a spec-03 `atype` — parenthesise for
    /// arrows / applied constructors.
    pub args: Vec<Type>,
    pub span: Span,
}

/// `type T a b = τ` — transparent alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    pub name: String,
    pub type_params: Vec<String>,
    pub body: Type,
    pub span: Span,
}

/// `class Ctx => Name tvar where body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDecl {
    /// Superclass constraints (the `Ctx` before `=>`).
    pub context: Vec<Constraint>,
    pub name: String,
    pub type_var: String,
    /// Method signatures and optional default bodies.
    pub items: Vec<ClassItem>,
    pub span: Span,
}

/// One entry inside `class ... where { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassItem {
    /// `methodName : scheme`.
    Signature {
        name: String,
        operator: bool,
        scheme: Scheme,
        span: Span,
    },
    /// `methodName pat... = expr` — default method body.
    Default(ValueClause),
}

/// `instance Ctx => Name τ where body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceDecl {
    pub context: Vec<Constraint>,
    pub name: String,
    /// The spec 07 §Instance declarations "head type". Restricted
    /// at kind-checking time to ground types or saturated
    /// constructors on distinct variables; parsed here without
    /// constraint-layer enforcement.
    pub head: Type,
    pub items: Vec<ValueClause>,
    pub span: Span,
}

/// `name p₁ ... pₙ := "ruby source"` — Ruby embedding, spec 10.
///
/// Parameters are restricted to plain `lower_ident`s by spec 10
/// §Embedding form; destructuring is not admitted. The signature
/// requirement (every `:=` binding must have a type annotation in
/// scope) is enforced at resolution time, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubyEmbedDecl {
    pub name: String,
    pub params: Vec<Param>,
    /// The Ruby source, after the lexer has decoded string
    /// escapes. For triple-quoted snippets this is the raw content
    /// between the `"""` delimiters (with escape processing per
    /// spec 10 §Triple-quoted string literals).
    pub source: String,
    pub span: Span,
}

/// A simple parameter name with span — used for Ruby-embedded
/// bindings (where only plain identifiers are admitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub span: Span,
}

// ===================================================================
//  Types
// ===================================================================

/// A surface type scheme `forall a b. (Ctx) => τ`. Both prefixes
/// are optional; a plain `τ` is a scheme with no quantifiers and
/// no constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    /// Explicit `forall a b.` variables. Empty when the scheme is
    /// written without a `forall`; the elaborator is responsible
    /// for implicitly quantifying the free type variables then.
    pub forall: Vec<String>,
    pub context: Vec<Constraint>,
    pub body: Type,
    pub span: Span,
}

/// One class constraint appearing in a scheme's context or in a
/// `class`/`instance` header. Parsed as `ClassName arg` where the
/// single-argument restriction is spec 07's single-parameter class
/// policy; the parser accepts the surface shape without enforcing
/// arity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub class_name: String,
    pub args: Vec<Type>,
    pub span: Span,
}

/// Surface types (spec 01 / 03 / 04 / 07 / 09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// `a`, `b`, ... — a `lower_ident` type variable.
    Var { name: String, span: Span },
    /// `Int`, `Maybe`, `List`, ... — an `upper_ident` constructor
    /// (possibly module-qualified).
    Con {
        /// Optional module qualifier, e.g. `Data.List.Map`.
        module: Option<ModName>,
        name: String,
        span: Span,
    },
    /// `τ₁ τ₂` — left-associative type application.
    App {
        func: Box<Type>,
        arg: Box<Type>,
        span: Span,
    },
    /// `τ₁ -> τ₂` — right-associative function arrow.
    Fun {
        param: Box<Type>,
        result: Box<Type>,
        span: Span,
    },
    /// `{ f₁ : τ₁, ..., fₙ : τₙ }` — structural record type.
    Record {
        fields: Vec<(String, Type)>,
        span: Span,
    },
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Var { span, .. }
            | Type::Con { span, .. }
            | Type::App { span, .. }
            | Type::Fun { span, .. }
            | Type::Record { span, .. } => *span,
        }
    }
}

// ===================================================================
//  Expressions
// ===================================================================

/// Surface expressions (spec 01 / 03 / 04 / 05 / 06 / 07 / 09 / 10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// `42`, `"hello"` — a literal expression.
    Lit(Literal, Span),
    /// `foo`, `Foo`, `Mod.foo` — a value or constructor reference.
    ///
    /// The spec's 02 §Identifiers split (`lower_ident` vs
    /// `upper_ident`) is kept in `name`'s first character: a name
    /// starting with an uppercase ASCII letter is a constructor
    /// reference by the spec 06 §Design notes namespace rule,
    /// resolved at the name-resolution layer.
    Var {
        /// Optional module qualifier. For a bare `foo` this is
        /// `None`; for `Foo.Bar.baz` or `Foo.Bar.Baz` the prefix
        /// is in `module`.
        module: Option<ModName>,
        name: String,
        span: Span,
    },
    /// `(+)` / `(>>=)` — parenthesised operator used as a value.
    OpRef { symbol: String, span: Span },
    /// `f x` — left-associative function application.
    App {
        func: Box<Expr>,
        arg: Box<Expr>,
        span: Span,
    },
    /// `\x y z -> body` — lambda. The parser accepts the surface
    /// multi-parameter form; it is not desugared here.
    Lambda {
        params: Vec<Pattern>,
        body: Box<Expr>,
        span: Span,
    },
    /// `let x = e₁ in e₂`. Spec 01 is single-binding; multi-binding
    /// `let` (03 OQ 2) is not admitted.
    Let {
        name: String,
        /// `(op)`-form binding, if the left-hand side was a
        /// parenthesised operator.
        operator: bool,
        params: Vec<Pattern>,
        value: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    /// `if c then t else f`.
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: Span,
    },
    /// `case scrutinee of pat -> body ; ...`.
    Case {
        scrutinee: Box<Expr>,
        arms: Vec<CaseArm>,
        span: Span,
    },
    /// Binary application of a named operator. Kept distinct from
    /// `App` so that the operator glyph is recoverable without
    /// walking nested `App { func = Var "(+)" ... }` trees.
    BinOp {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// `-e` — surface unary minus (spec 05 §Unary minus). Desugars
    /// to `negate e` at elaboration time.
    Neg { value: Box<Expr>, span: Span },
    /// `{ f₁ = e₁, ..., fₙ = eₙ }` — record literal.
    RecordLit {
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// `{ e | f = ... }` — record update.
    RecordUpdate {
        record: Box<Expr>,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// `expr.field` — field selection.
    FieldAccess {
        record: Box<Expr>,
        field: String,
        span: Span,
    },
    /// `[]` / `[x, y, z]` — list literal (spec 09).
    ListLit { items: Vec<Expr>, span: Span },
    /// `do { s₁ ; s₂ ; ... ; sₙ }` — monadic do-notation.
    Do { stmts: Vec<DoStmt>, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Lit(_, span)
            | Expr::Var { span, .. }
            | Expr::OpRef { span, .. }
            | Expr::App { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::Let { span, .. }
            | Expr::If { span, .. }
            | Expr::Case { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::Neg { span, .. }
            | Expr::RecordLit { span, .. }
            | Expr::RecordUpdate { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::Do { span, .. } => *span,
        }
    }
}

/// A single arm of a `case` expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// A single `do` block statement (spec 07 §`do` notation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoStmt {
    /// `pat <- e`.
    Bind {
        pattern: Pattern,
        expr: Expr,
        span: Span,
    },
    /// `let x = e` (no `in` — the block's rest of stmts form the
    /// body).
    Let {
        name: String,
        operator: bool,
        params: Vec<Pattern>,
        value: Expr,
        span: Span,
    },
    /// Bare expression as a statement.
    Expr(Expr),
}

// ===================================================================
//  Patterns and literals
// ===================================================================

/// Surface patterns (spec 06 + 09 extensions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// `_`.
    Wildcard(Span),
    /// `x`, `foo_bar` — a `lower_ident` binding.
    Var { name: String, span: Span },
    /// `x@pat` — as-pattern.
    As {
        name: String,
        inner: Box<Pattern>,
        span: Span,
    },
    /// Literal patterns reuse [`Literal`].
    Lit(Literal, Span),
    /// `C p₁ ... pₙ` — constructor pattern with positional
    /// arguments. A nullary constructor reference has
    /// `args.is_empty()`.
    Con {
        /// Optional module qualifier, e.g. `Prelude.Just`.
        module: Option<ModName>,
        name: String,
        args: Vec<Pattern>,
        span: Span,
    },
    /// `p₁ :: p₂` — cons pattern.
    Cons {
        head: Box<Pattern>,
        tail: Box<Pattern>,
        span: Span,
    },
    /// `[]` / `[p, q, r]` — list-literal pattern (spec 09).
    List { items: Vec<Pattern>, span: Span },
    /// `{ f = p, ... }` — record pattern. Subset patterns are
    /// allowed (spec 06 §Record patterns).
    Record {
        fields: Vec<(String, Pattern)>,
        span: Span,
    },
    /// `(pat : type)` — type-annotated pattern.
    Annot {
        inner: Box<Pattern>,
        ty: Type,
        span: Span,
    },
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(span) => *span,
            Pattern::Var { span, .. }
            | Pattern::As { span, .. }
            | Pattern::Lit(_, span)
            | Pattern::Con { span, .. }
            | Pattern::Cons { span, .. }
            | Pattern::List { span, .. }
            | Pattern::Record { span, .. }
            | Pattern::Annot { span, .. } => *span,
        }
    }
}

/// Literals shared between expressions and patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Int(i64),
    Str(String),
}
