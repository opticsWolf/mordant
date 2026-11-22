//! Runtime switches for the optional engines.
//!
//! Engines are compiled into the crate via cargo features and are enabled at
//! runtime through [`Engines`]. An engine is active only when **both** its
//! cargo feature was compiled in and its flag is set. Flags for engines that
//! were not compiled in are silently inert.
//!
//! # Example
//! ```
//! use mordant::extensions::{Engine, Engines};
//!
//! let all = Engines::all();
//! assert!(all.get(Engine::Emoji));
//! let none = Engines::none();
//! assert!(!none.get(Engine::Emoji));
//! let only_math = Engines::only(&[Engine::Math]);
//! assert!(only_math.get(Engine::Math));
//! assert!(!only_math.get(Engine::Emoji));
//! // `is_enabled()` also checks whether the cargo feature was compiled in.
//! # #[cfg(feature = "emoji")] {
//! assert!(all.is_enabled(Engine::Emoji));
//! # }
//! ```

use core::fmt;

/// An optional engine that can be enabled or disabled independently.
///
/// This enum is `#[non_exhaustive]`; new engines may be added in minor
/// releases, so downstream `match` expressions need a wildcard arm.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    /// YAML front matter (`meta` feature).
    Meta,
    /// `:shortcode:` emoji (`emoji` feature).
    Emoji,
    /// PHP Markdown Extra footnotes (`footnotes` feature).
    Footnotes,
    /// Markdown linter (`linter` feature).
    Linter,
    /// Mermaid diagrams (`diagram` feature).
    Diagram,
    /// AST-aware chunking (`chunker` feature).
    Chunker,
    /// KaTeX math (`math` feature).
    Math,
    /// Syntax highlighting (`highlighter` feature).
    Highlighter,
    /// Definition lists (`definition-list` feature).
    DefinitionList,
    /// Fenced divs (`fenced-div` feature).
    FencedDiv,
    /// Link attributes (`link-attributes` feature).
    LinkAttributes,
}

impl Engine {
    /// All engines, in the order in which they are registered.
    pub const ALL: &'static [Engine] = &[
        Engine::Meta,
        Engine::Emoji,
        Engine::Footnotes,
        Engine::Linter,
        Engine::Diagram,
        Engine::Chunker,
        Engine::Math,
        Engine::Highlighter,
        Engine::DefinitionList,
        Engine::FencedDiv,
        Engine::LinkAttributes,
    ];

    /// The stable, snake_case name used by the Python API.
    pub const fn name(self) -> &'static str {
        match self {
            Engine::Meta => "meta",
            Engine::Emoji => "emoji",
            Engine::Footnotes => "footnotes",
            Engine::Linter => "linter",
            Engine::Diagram => "diagram",
            Engine::Chunker => "chunker",
            Engine::Math => "math",
            Engine::Highlighter => "highlighter",
            Engine::DefinitionList => "definition_list",
            Engine::FencedDiv => "fenced_div",
            Engine::LinkAttributes => "link_attributes",
        }
    }

    /// Looks up an engine by its stable name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|e| e.name() == name)
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Runtime switches for all optional engines.
///
/// Defaults to [`Engines::default()`], i.e. every engine except syntax
/// highlighting (which used to require an explicit theme in 0.10).
///
/// This type is `#[non_exhaustive]`: construct it with [`Self::default()`],
/// [`Self::all()`], [`Self::none()`] or [`Self::only()`], then change
/// individual flags with [`Self::set()`] or [`Self::without()`].
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Engines {
    /// Enables YAML front matter (`meta` feature).
    pub meta: bool,
    /// Enables `:shortcode:` emoji parsing and rendering (`emoji` feature).
    pub emoji: bool,
    /// Enables PHP Markdown Extra footnotes (`footnotes` feature).
    pub footnotes: bool,
    /// Enables `lint()` / `fix()` in the Python bindings (`linter` feature).
    /// Has no parser or renderer side effects.
    pub linter: bool,
    /// Enables Mermaid diagram parsing and rendering (`diagram` feature).
    pub diagram: bool,
    /// Enables engine-aware chunking (`chunker` feature). The chunker type
    /// itself is available whenever that feature is compiled in; this flag
    /// controls which syntax extensions affect chunk boundaries.
    pub chunker: bool,
    /// Enables KaTeX math (`math` feature).
    pub math: bool,
    /// Enables syntax highlighting. Off in [`Engines::default()`].
    pub highlighter: bool,
    /// Enables definition lists (`definition-list` feature).
    pub definition_list: bool,
    /// Enables fenced divs (`fenced-div` feature).
    pub fenced_div: bool,
    /// Enables link and image attribute lists (`link-attributes` feature).
    pub link_attributes: bool,
}

impl Default for Engines {
    fn default() -> Self {
        // Keep 0.10 output stable: highlighting is the one engine that used
        // to require an explicit theme. Use `Engines::all()` to turn it on.
        Self::all_without_highlighter()
    }
}

impl Engines {
    /// Enables every engine.
    pub const fn all() -> Self {
        Self {
            meta: true,
            emoji: true,
            footnotes: true,
            linter: true,
            diagram: true,
            chunker: true,
            math: true,
            highlighter: true,
            definition_list: true,
            fenced_div: true,
            link_attributes: true,
        }
    }

    /// Every engine except syntax highlighting.
    ///
    /// This is the default because highlighting used to require an explicit
    /// theme in 0.10; enabling it by default would change the HTML emitted
    /// for every code block.
    pub const fn all_without_highlighter() -> Self {
        Self {
            highlighter: false,
            ..Self::all()
        }
    }

    /// Disables every engine.
    pub const fn none() -> Self {
        Self {
            meta: false,
            emoji: false,
            footnotes: false,
            linter: false,
            diagram: false,
            chunker: false,
            math: false,
            highlighter: false,
            definition_list: false,
            fenced_div: false,
            link_attributes: false,
        }
    }

    /// Enables only the named engines.
    pub fn only(engines: &[Engine]) -> Self {
        let mut result = Self::none();
        for engine in engines {
            result.set(*engine, true);
        }
        result
    }

    /// This engine set except the named ones.
    ///
    /// `Engines::default().without(...)` keeps highlighting off;
    /// `Engines::all().without(...)` starts from every engine.
    pub fn without(self, engines: &[Engine]) -> Self {
        let mut result = self;
        for engine in engines {
            result.set(*engine, false);
        }
        result
    }

    /// Sets a single engine's flag.
    pub fn set(&mut self, engine: Engine, enabled: bool) {
        match engine {
            Engine::Meta => self.meta = enabled,
            Engine::Emoji => self.emoji = enabled,
            Engine::Footnotes => self.footnotes = enabled,
            Engine::Linter => self.linter = enabled,
            Engine::Diagram => self.diagram = enabled,
            Engine::Chunker => self.chunker = enabled,
            Engine::Math => self.math = enabled,
            Engine::Highlighter => self.highlighter = enabled,
            Engine::DefinitionList => self.definition_list = enabled,
            Engine::FencedDiv => self.fenced_div = enabled,
            Engine::LinkAttributes => self.link_attributes = enabled,
        }
    }

    /// Returns the flag for a single engine.
    pub fn get(self, engine: Engine) -> bool {
        match engine {
            Engine::Meta => self.meta,
            Engine::Emoji => self.emoji,
            Engine::Footnotes => self.footnotes,
            Engine::Linter => self.linter,
            Engine::Diagram => self.diagram,
            Engine::Chunker => self.chunker,
            Engine::Math => self.math,
            Engine::Highlighter => self.highlighter,
            Engine::DefinitionList => self.definition_list,
            Engine::FencedDiv => self.fenced_div,
            Engine::LinkAttributes => self.link_attributes,
        }
    }

    /// Returns true if the engine is both compiled in and enabled.
    pub fn is_enabled(self, engine: Engine) -> bool {
        let compiled = match engine {
            Engine::Meta => cfg!(feature = "meta"),
            Engine::Emoji => cfg!(feature = "emoji"),
            Engine::Footnotes => cfg!(feature = "footnotes"),
            Engine::Linter => cfg!(feature = "linter"),
            Engine::Diagram => cfg!(feature = "diagram"),
            Engine::Chunker => cfg!(feature = "chunker"),
            Engine::Math => cfg!(feature = "math"),
            Engine::Highlighter => cfg!(feature = "highlighter"),
            Engine::DefinitionList => cfg!(feature = "definition-list"),
            Engine::FencedDiv => cfg!(feature = "fenced-div"),
            Engine::LinkAttributes => cfg!(feature = "link-attributes"),
        };
        compiled && self.get(engine)
    }
}

impl fmt::Debug for Engines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_struct("Engines");
        for engine in Engine::ALL {
            f.field(engine.name(), &self.get(*engine));
        }
        f.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_and_none() {
        for engine in Engine::ALL {
            assert!(Engines::all().get(*engine));
            assert!(!Engines::none().get(*engine));
        }
    }

    #[test]
    fn without_keeps_the_default_highlighter_state() {
        assert!(!Engines::default()
            .without(&[Engine::Emoji])
            .get(Engine::Highlighter));
        assert!(Engines::all()
            .without(&[Engine::Emoji])
            .get(Engine::Highlighter));
    }

    #[test]
    fn only_and_names() {
        let engines = Engines::only(&[Engine::Emoji, Engine::Math]);
        assert!(engines.get(Engine::Emoji));
        assert!(engines.get(Engine::Math));
        assert!(!engines.get(Engine::Diagram));
        assert_eq!(
            Engine::from_name("definition_list"),
            Some(Engine::DefinitionList)
        );
        assert_eq!(Engine::from_name("nope"), None);
    }
}
