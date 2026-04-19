//! Local-scope stack used while walking expression bodies.
//!
//! Unlike top-level bindings — which live in [`super::env::ModuleEnv`]
//! — local bindings come and go with the enclosing lambda, `let`,
//! `case` arm, or `do` block. The resolver manages them with a stack
//! of frames: entering a binder pushes a frame, leaving it pops.
//!
//! Shadowing is admitted everywhere per spec 06 §Design notes. The
//! resolver does not warn on shadowing; the LSP layer may later.

use std::collections::HashMap;

/// A single frame of bindings introduced together (one lambda, one
/// `let`'s parameters, one `do` bind pattern's variables, etc.).
///
/// Kept as a plain `HashMap<String, ()>` because local names do not
/// need a richer identity than "this name is bound here" — they
/// never leave the expression in which they are introduced.
#[derive(Debug, Default, Clone)]
pub struct Frame {
    names: HashMap<String, ()>,
}

impl Frame {
    pub fn insert(&mut self, name: &str) {
        self.names.insert(name.to_string(), ());
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }
}

/// A stack of frames, pushed and popped as expression-level binders
/// open and close.
#[derive(Debug, Default, Clone)]
pub struct ScopeStack {
    frames: Vec<Frame>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self) {
        self.frames.push(Frame::default());
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    pub fn bind(&mut self, name: &str) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name);
        }
    }

    pub fn lookup(&self, name: &str) -> bool {
        for frame in self.frames.iter().rev() {
            if frame.contains(name) {
                return true;
            }
        }
        false
    }

    /// Current nesting depth. Handy for tests that want to assert
    /// the stack is balanced after walking an expression.
    #[allow(dead_code)]
    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}
