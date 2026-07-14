//! LaTeX math rendering for Mordant, backed by the pure-Rust `katex-rs` crate.
//!
//! Efficiency model (verified against katex-rs 0.2.4 source):
//! - `KatexContext` (font metrics, symbol tables, function/environment registries)
//!   is expensive to build but `Send + Sync`, and every render takes `&ctx`. It is
//!   built ONCE in a `LazyLock` and shared read-only across all renders and threads.
//! - Rendered markup is memoized on `(display, output, latex)`: documents repeat
//!   formulas and rendering is the costly step. `Arc<str>` makes cache hits cheap.
//! - Rendering is pure-Rust CPU work, so it runs with the GIL released.
//!
//! Output format (`OutputFormat`, verified in types/settings.rs):
//! - "both"   -> HtmlAndMathml (default): styled HTML + MathML. Needs katex.css.
//! - "html"   -> Html only: styled HTML. Needs katex.css + fonts.
//! - "mathml" -> Mathml only: semantic MathML. Renders with no CSS/fonts in a
//!   MathML-capable engine (e.g. Chromium >= 109 / recent QtWebEngine).
//!
//! NOTE: `render_to_string` emits markup only, never CSS or fonts. For "both"/"html"
//! the consuming page must load KaTeX's stylesheet + web fonts.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyString;

use katex::{render_to_string, KatexContext, OutputFormat, Settings, StrictMode, StrictSetting};

use rushdown_lib::ast::{Arena, CodeBlock, NodeRef, WalkStatus};
use rushdown_lib::{as_kind_data, Result};
use rushdown_lib::renderer::{self, html, NodeRenderer, RenderNode, TextWrite, NodeRendererRegistry, BoxRenderNode};
use rushdown_lib::renderer::html::{RendererExtension, RendererExtensionFn};

/// Built once, lazily. `KatexContext: Send + Sync` and renders take `&ctx`, so a
/// single global instance serves every thread.
static KATEX: LazyLock<KatexContext> = LazyLock::new(KatexContext::default);

/// KaTeX 0.16.21 minified CSS (matching `katex-rs 0.2.4`).
///
/// Consumers who render `"both"` or `"html"` output must load this stylesheet
/// (plus the KaTeX web fonts) for correct positioning of the nested spans.
/// For `"mathml"` output no CSS is needed — Chromium ≥ 109 renders MathML
/// natively.
///
/// Exposed to Python as `mordant.KATEX_CSS`.
pub const KATEX_CSS: &str = r#"@font-face{font-family:KaTeX_AMS;font-style:normal;font-weight:400;src:url(fonts/KaTeX_AMS-Regular.woff2) format("woff2"),url(fonts/KaTeX_AMS-Regular.woff) format("woff"),url(fonts/KaTeX_AMS-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Caligraphic;font-style:normal;font-weight:700;src:url(fonts/KaTeX_Caligraphic-Bold.woff2) format("woff2"),url(fonts/KaTeX_Caligraphic-Bold.woff) format("woff"),url(fonts/KaTeX_Caligraphic-Bold.ttf) format("truetype")}@font-face{font-family:KaTeX_Caligraphic;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Caligraphic-Regular.woff2) format("woff2"),url(fonts/KaTeX_Caligraphic-Regular.woff) format("woff"),url(fonts/KaTeX_Caligraphic-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Fraktur;font-style:normal;font-weight:700;src:url(fonts/KaTeX_Fraktur-Bold.woff2) format("woff2"),url(fonts/KaTeX_Fraktur-Bold.woff) format("woff"),url(fonts/KaTeX_Fraktur-Bold.ttf) format("truetype")}@font-face{font-family:KaTeX_Fraktur;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Fraktur-Regular.woff2) format("woff2"),url(fonts/KaTeX_Fraktur-Regular.woff) format("woff"),url(fonts/KaTeX_Fraktur-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Main;font-style:normal;font-weight:700;src:url(fonts/KaTeX_Main-Bold.woff2) format("woff2"),url(fonts/KaTeX_Main-Bold.woff) format("woff"),url(fonts/KaTeX_Main-Bold.ttf) format("truetype")}@font-face{font-family:KaTeX_Main;font-style:italic;font-weight:700;src:url(fonts/KaTeX_Main-BoldItalic.woff2) format("woff2"),url(fonts/KaTeX_Main-BoldItalic.woff) format("woff"),url(fonts/KaTeX_Main-BoldItalic.ttf) format("truetype")}@font-face{font-family:KaTeX_Main;font-style:italic;font-weight:400;src:url(fonts/KaTeX_Main-Italic.woff2) format("woff2"),url(fonts/KaTeX_Main-Italic.woff) format("woff"),url(fonts/KaTeX_Main-Italic.ttf) format("truetype")}@font-face{font-family:KaTeX_Main;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Main-Regular.woff2) format("woff2"),url(fonts/KaTeX_Main-Regular.woff) format("woff"),url(fonts/KaTeX_Main-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Math;font-style:italic;font-weight:700;src:url(fonts/KaTeX_Math-BoldItalic.woff2) format("woff2"),url(fonts/KaTeX_Math-BoldItalic.woff) format("woff"),url(fonts/KaTeX_Math-BoldItalic.ttf) format("truetype")}@font-face{font-family:KaTeX_Math;font-style:italic;font-weight:400;src:url(fonts/KaTeX_Math-Italic.woff2) format("woff2"),url(fonts/KaTeX_Math-Italic.woff) format("woff"),url(fonts/KaTeX_Math-Italic.ttf) format("truetype")}@font-face{font-family:"KaTeX_SansSerif";font-style:normal;font-weight:700;src:url(fonts/KaTeX_SansSerif-Bold.woff2) format("woff2"),url(fonts/KaTeX_SansSerif-Bold.woff) format("woff"),url(fonts/KaTeX_SansSerif-Bold.ttf) format("truetype")}@font-face{font-family:"KaTeX_SansSerif";font-style:italic;font-weight:400;src:url(fonts/KaTeX_SansSerif-Italic.woff2) format("woff2"),url(fonts/KaTeX_SansSerif-Italic.woff) format("woff"),url(fonts/KaTeX_SansSerif-Italic.ttf) format("truetype")}@font-face{font-family:"KaTeX_SansSerif";font-style:normal;font-weight:400;src:url(fonts/KaTeX_SansSerif-Regular.woff2) format("woff2"),url(fonts/KaTeX_SansSerif-Regular.woff) format("woff"),url(fonts/KaTeX_SansSerif-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Script;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Script-Regular.woff2) format("woff2"),url(fonts/KaTeX_Script-Regular.woff) format("woff"),url(fonts/KaTeX_Script-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Size1;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Size1-Regular.woff2) format("woff2"),url(fonts/KaTeX_Size1-Regular.woff) format("woff"),url(fonts/KaTeX_Size1-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Size2;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Size2-Regular.woff2) format("woff2"),url(fonts/KaTeX_Size2-Regular.woff) format("woff"),url(fonts/KaTeX_Size2-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Size3;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Size3-Regular.woff2) format("woff2"),url(fonts/KaTeX_Size3-Regular.woff) format("woff"),url(fonts/KaTeX_Size3-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Size4;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Size4-Regular.woff2) format("woff2"),url(fonts/KaTeX_Size4-Regular.woff) format("woff"),url(fonts/KaTeX_Size4-Regular.ttf) format("truetype")}@font-face{font-family:KaTeX_Typewriter;font-style:normal;font-weight:400;src:url(fonts/KaTeX_Typewriter-Regular.woff2) format("woff2"),url(fonts/KaTeX_Typewriter-Regular.woff) format("woff"),url(fonts/KaTeX_Typewriter-Regular.ttf) format("truetype")}.katex{font:normal 1.21em KaTeX_Main,Times New Roman,serif;line-height:1.2;text-indent:0;text-rendering:auto}.katex *{-ms-high-contrast-adjust:none!important;border-color:currentColor}.katex .katex-version:after{content:"0.16.21"}.katex .katex-mathml{clip:rect(1px,1px,1px,1px);border:0;height:1px;overflow:hidden;padding:0;position:absolute;width:1px}.katex .katex-html>.newline{display:block}.katex .base{position:relative;white-space:nowrap;width:-webkit-min-content;width:-moz-min-content;width:min-content}.katex .base,.katex .strut{display:inline-block}.katex .textbf{font-weight:700}.katex .textit{font-style:italic}.katex .textrm{font-family:KaTeX_Main}.katex .textsf{font-family:KaTeX_SansSerif}.katex .texttt{font-family:KaTeX_Typewriter}.katex .mathnormal{font-family:KaTeX_Math;font-style:italic}.katex .mathit{font-family:KaTeX_Main;font-style:italic}.katex .mathrm{font-style:normal}.katex .mathbf{font-family:KaTeX_Main;font-weight:700}.katex .boldsymbol{font-family:KaTeX_Math;font-style:italic;font-weight:700}.katex .amsrm,.katex .mathbb,.katex .textbb{font-family:KaTeX_AMS}.katex .mathcal{font-family:KaTeX_Caligraphic}.katex .mathfrak,.katex .textfrak{font-family:KaTeX_Fraktur}.katex .mathboldfrak,.katex .textboldfrak{font-family:KaTeX_Fraktur;font-weight:700}.katex .mathtt{font-family:KaTeX_Typewriter}.katex .mathscr,.katex .textscr{font-family:KaTeX_Script}.katex .mathsf,.katex .textsf{font-family:KaTeX_SansSerif}.katex .mathboldsf,.katex .textboldsf{font-family:KaTeX_SansSerif;font-weight:700}.katex .mathitsf,.katex .mathsfit,.katex .textitsf{font-family:KaTeX_SansSerif;font-style:italic}.katex .mainrm{font-family:KaTeX_Main;font-style:normal}.katex .vlist-t{border-collapse:collapse;display:inline-table;table-layout:fixed}.katex .vlist-r{display:table-row}.katex .vlist{display:table-cell;position:relative;vertical-align:bottom}.katex .vlist>span{display:block;height:0;position:relative}.katex .vlist>span>span{display:inline-block}.katex .vlist>span>.pstrut{overflow:hidden;width:0}.katex .vlist-t2{margin-right:-2px}.katex .vlist-s{display:table-cell;font-size:1px;min-width:2px;vertical-align:bottom;width:2px}.katex .vbox{align-items:baseline;display:inline-flex;flex-direction:column}.katex .hbox{width:100%}.katex .hbox,.katex .thinbox{display:inline-flex;flex-direction:row}.katex .thinbox{max-width:0;width:0}.katex .msupsub{text-align:left}.katex .mfrac>span>span{text-align:center}.katex .mfrac .frac-line{border-bottom-style:solid;display:inline-block;width:100%}.katex .hdashline,.katex .hline,.katex .mfrac .frac-line,.katex .overline .overline-line,.katex .rule,.katex .underline .underline-line{min-height:1px}.katex .mspace{display:inline-block}.katex .clap,.katex .llap,.katex .rlap{position:relative;width:0}.katex .clap>.inner,.katex .llap>.inner,.katex .rlap>.inner{position:absolute}.katex .clap>.fix,.katex .llap>.fix,.katex .rlap>.fix{display:inline-block}.katex .llap>.inner{right:0}.katex .clap>.inner,.katex .rlap>.inner{left:0}.katex .clap>.inner>span{margin-left:-50%;margin-right:50%}.katex .rule{border:0 solid;display:inline-block;position:relative}.katex .hline,.katex .overline .overline-line,.katex .underline .underline-line{border-bottom-style:solid;display:inline-block;width:100%}.katex .hdashline{border-bottom-style:dashed;display:inline-block;width:100%}.katex .sqrt>.root{margin-left:.2777777778em;margin-right:-.5555555556em}.katex .fontsize-ensurer.reset-size1.size1,.katex .sizing.reset-size1.size1{font-size:1em}.katex .fontsize-ensurer.reset-size1.size2,.katex .sizing.reset-size1.size2{font-size:1.2em}.katex .fontsize-ensurer.reset-size1.size3,.katex .sizing.reset-size1.size3{font-size:1.4em}.katex .fontsize-ensurer.reset-size1.size4,.katex .sizing.reset-size1.size4{font-size:1.6em}.katex .fontsize-ensurer.reset-size1.size5,.katex .sizing.reset-size1.size5{font-size:1.8em}.katex .fontsize-ensurer.reset-size1.size6,.katex .sizing.reset-size1.size6{font-size:2em}.katex .fontsize-ensurer.reset-size1.size7,.katex .sizing.reset-size1.size7{font-size:2.4em}.katex .fontsize-ensurer.reset-size1.size8,.katex .sizing.reset-size1.size8{font-size:2.88em}.katex .fontsize-ensurer.reset-size1.size9,.katex .sizing.reset-size1.size9{font-size:3.456em}.katex .fontsize-ensurer.reset-size1.size10,.katex .sizing.reset-size1.size10{font-size:4.148em}.katex .fontsize-ensurer.reset-size1.size11,.katex .sizing.reset-size1.size11{font-size:4.976em}.katex .fontsize-ensurer.reset-size2.size1,.katex .sizing.reset-size2.size1{font-size:.8333333333em}.katex .fontsize-ensurer.reset-size2.size2,.katex .sizing.reset-size2.size2{font-size:1em}.katex .fontsize-ensurer.reset-size2.size3,.katex .sizing.reset-size2.size3{font-size:1.1666666667em}.katex .fontsize-ensurer.reset-size2.size4,.katex .sizing.reset-size2.size4{font-size:1.3333333333em}.katex .fontsize-ensurer.reset-size2.size5,.katex .sizing.reset-size2.size5{font-size:1.5em}.katex .fontsize-ensurer.reset-size2.size6,.katex .sizing.reset-size2.size6{font-size:1.6666666667em}.katex .fontsize-ensurer.reset-size2.size7,.katex .sizing.reset-size2.size7{font-size:2em}.katex .fontsize-ensurer.reset-size2.size8,.katex .sizing.reset-size2.size8{font-size:2.4em}.katex .fontsize-ensurer.reset-size2.size9,.katex .sizing.reset-size2.size9{font-size:2.88em}.katex .fontsize-ensurer.reset-size2.size10,.katex .sizing.reset-size2.size10{font-size:3.4566666667em}.katex .fontsize-ensurer.reset-size2.size11,.katex .sizing.reset-size2.size11{font-size:4.1466666667em}.katex .fontsize-ensurer.reset-size3.size1,.katex .sizing.reset-size3.size1{font-size:.7142857143em}.katex .fontsize-ensurer.reset-size3.size2,.katex .sizing.reset-size3.size2{font-size:.8571428571em}.katex .fontsize-ensurer.reset-size3.size3,.katex .sizing.reset-size3.size3{font-size:1em}.katex .fontsize-ensurer.reset-size3.size4,.katex .sizing.reset-size3.size4{font-size:1.1428571429em}.katex .fontsize-ensurer.reset-size3.size5,.katex .sizing.reset-size3.size5{font-size:1.2857142857em}.katex .fontsize-ensurer.reset-size3.size6,.katex .sizing.reset-size3.size6{font-size:1.4285714286em}.katex .fontsize-ensurer.reset-size3.size7,.katex .sizing.reset-size3.size7{font-size:1.7142857143em}.katex .fontsize-ensurer.reset-size3.size8,.katex .sizing.reset-size3.size8{font-size:2.0571428571em}.katex .fontsize-ensurer.reset-size3.size9,.katex .sizing.reset-size3.size9{font-size:2.4685714286em}.katex .fontsize-ensurer.reset-size3.size10,.katex .sizing.reset-size3.size10{font-size:2.9628571429em}.katex .fontsize-ensurer.reset-size3.size11,.katex .sizing.reset-size3.size11{font-size:3.5542857143em}.katex .fontsize-ensurer.reset-size4.size1,.katex .sizing.reset-size4.size1{font-size:.625em}.katex .fontsize-ensurer.reset-size4.size2,.katex .sizing.reset-size4.size2{font-size:.75em}.katex .fontsize-ensurer.reset-size4.size3,.katex .sizing.reset-size4.size3{font-size:.875em}.katex .fontsize-ensurer.reset-size4.size4,.katex .sizing.reset-size4.size4{font-size:1em}.katex .fontsize-ensurer.reset-size4.size5,.katex .sizing.reset-size4.size5{font-size:1.125em}.katex .fontsize-ensurer.reset-size4.size6,.katex .sizing.reset-size4.size6{font-size:1.25em}.katex .fontsize-ensurer.reset-size4.size7,.katex .sizing.reset-size4.size7{font-size:1.5em}.katex .fontsize-ensurer.reset-size4.size8,.katex .sizing.reset-size4.size8{font-size:1.8em}.katex .fontsize-ensurer.reset-size4.size9,.katex .sizing.reset-size4.size9{font-size:2.16em}.katex .fontsize-ensurer.reset-size4.size10,.katex .sizing.reset-size4.size10{font-size:2.5925em}.katex .fontsize-ensurer.reset-size4.size11,.katex .sizing.reset-size4.size11{font-size:3.11em}.katex .fontsize-ensurer.reset-size5.size1,.katex .sizing.reset-size5.size1{font-size:.5555555556em}.katex .fontsize-ensurer.reset-size5.size2,.katex .sizing.reset-size5.size2{font-size:.6666666667em}.katex .fontsize-ensurer.reset-size5.size3,.katex .sizing.reset-size5.size3{font-size:.7777777778em}.katex .fontsize-ensurer.reset-size5.size4,.katex .sizing.reset-size5.size4{font-size:.8888888889em}.katex .fontsize-ensurer.reset-size5.size5,.katex .sizing.reset-size5.size5{font-size:1em}.katex .fontsize-ensurer.reset-size5.size6,.katex .sizing.reset-size5.size6{font-size:1.1111111111em}.katex .fontsize-ensurer.reset-size5.size7,.katex .sizing.reset-size5.size7{font-size:1.3333333333em}.katex .fontsize-ensurer.reset-size5.size8,.katex .sizing.reset-size5.size8{font-size:1.6em}.katex .fontsize-ensurer.reset-size5.size9,.katex .sizing.reset-size5.size9{font-size:1.92em}.katex .fontsize-ensurer.reset-size5.size10,.katex .sizing.reset-size5.size10{font-size:2.3044444444em}.katex .fontsize-ensurer.reset-size5.size11,.katex .sizing.reset-size5.size11{font-size:2.7644444444em}.katex .fontsize-ensurer.reset-size6.size1,.katex .sizing.reset-size6.size1{font-size:.5em}.katex .fontsize-ensurer.reset-size6.size2,.katex .sizing.reset-size6.size2{font-size:.6em}.katex .fontsize-ensurer.reset-size6.size3,.katex .sizing.reset-size6.size3{font-size:.7em}.katex .fontsize-ensurer.reset-size6.size4,.katex .sizing.reset-size6.size4{font-size:.8em}.katex .fontsize-ensurer.reset-size6.size5,.katex .sizing.reset-size6.size5{font-size:.9em}.katex .fontsize-ensurer.reset-size6.size6,.katex .sizing.reset-size6.size6{font-size:1em}.katex .fontsize-ensurer.reset-size6.size7,.katex .sizing.reset-size6.size7{font-size:1.2em}.katex .fontsize-ensurer.reset-size6.size8,.katex .sizing.reset-size6.size8{font-size:1.44em}.katex .fontsize-ensurer.reset-size6.size9,.katex .sizing.reset-size6.size9{font-size:1.728em}.katex .fontsize-ensurer.reset-size6.size10,.katex .sizing.reset-size6.size10{font-size:2.074em}.katex .fontsize-ensurer.reset-size6.size11,.katex .sizing.reset-size6.size11{font-size:2.488em}.katex .fontsize-ensurer.reset-size7.size1,.katex .sizing.reset-size7.size1{font-size:.4166666667em}.katex .fontsize-ensurer.reset-size7.size2,.katex .sizing.reset-size7.size2{font-size:.5em}.katex .fontsize-ensurer.reset-size7.size3,.katex .sizing.reset-size7.size3{font-size:.5833333333em}.katex .fontsize-ensurer.reset-size7.size4,.katex .sizing.reset-size7.size4{font-size:.6666666667em}.katex .fontsize-ensurer.reset-size7.size5,.katex .sizing.reset-size7.size5{font-size:.75em}.katex .fontsize-ensurer.reset-size7.size6,.katex .sizing.reset-size7.size6{font-size:.8333333333em}.katex .fontsize-ensurer.reset-size7.size7,.katex .sizing.reset-size7.size7{font-size:1em}.katex .fontsize-ensurer.reset-size7.size8,.katex .sizing.reset-size7.size8{font-size:1.2em}.katex .fontsize-ensurer.reset-size7.size9,.katex .sizing.reset-size7.size9{font-size:1.44em}.katex .fontsize-ensurer.reset-size7.size10,.katex .sizing.reset-size7.size10{font-size:1.7283333333em}.katex .fontsize-ensurer.reset-size7.size11,.katex .sizing.reset-size7.size11{font-size:2.0733333333em}.katex .fontsize-ensurer.reset-size8.size1,.katex .sizing.reset-size8.size1{font-size:.3472222222em}.katex .fontsize-ensurer.reset-size8.size2,.katex .sizing.reset-size8.size2{font-size:.4166666667em}.katex .fontsize-ensurer.reset-size8.size3,.katex .sizing.reset-size8.size3{font-size:.4861111111em}.katex .fontsize-ensurer.reset-size8.size4,.katex .sizing.reset-size8.size4{font-size:.5555555556em}.katex .fontsize-ensurer.reset-size8.size5,.katex .sizing.reset-size8.size5{font-size:.625em}.katex .fontsize-ensurer.reset-size8.size6,.katex .sizing.reset-size8.size6{font-size:.6944444444em}.katex .fontsize-ensurer.reset-size8.size7,.katex .sizing.reset-size8.size7{font-size:.8333333333em}.katex .fontsize-ensurer.reset-size8.size8,.katex .sizing.reset-size8.size8{font-size:1em}.katex .fontsize-ensurer.reset-size8.size9,.katex .sizing.reset-size8.size9{font-size:1.2em}.katex .fontsize-ensurer.reset-size8.size10,.katex .sizing.reset-size8.size10{font-size:1.4402777778em}.katex .fontsize-ensurer.reset-size8.size11,.katex .sizing.reset-size8.size11{font-size:1.7277777778em}.katex .fontsize-ensurer.reset-size9.size1,.katex .sizing.reset-size9.size1{font-size:.2893518519em}.katex .fontsize-ensurer.reset-size9.size2,.katex .sizing.reset-size9.size2{font-size:.3472222222em}.katex .fontsize-ensurer.reset-size9.size3,.katex .sizing.reset-size9.size3{font-size:.4050925926em}.katex .fontsize-ensurer.reset-size9.size4,.katex .sizing.reset-size9.size4{font-size:.462962963em}.katex .fontsize-ensurer.reset-size9.size5,.katex .sizing.reset-size9.size5{font-size:.5208333333em}.katex .fontsize-ensurer.reset-size9.size6,.katex .sizing.reset-size9.size6{font-size:.5787037037em}.katex .fontsize-ensurer.reset-size9.size7,.katex .sizing.reset-size9.size7{font-size:.6944444444em}.katex .fontsize-ensurer.reset-size9.size8,.katex .sizing.reset-size9.size8{font-size:.8333333333em}.katex .fontsize-ensurer.reset-size9.size9,.katex .sizing.reset-size9.size9{font-size:1em}.katex .fontsize-ensurer.reset-size9.size10,.katex .sizing.reset-size9.size10{font-size:1.2002314815em}.katex .fontsize-ensurer.reset-size9.size11,.katex .sizing.reset-size9.size11{font-size:1.4398148148em}.katex .fontsize-ensurer.reset-size10.size1,.katex .sizing.reset-size10.size1{font-size:.2410800386em}.katex .fontsize-ensurer.reset-size10.size2,.katex .sizing.reset-size10.size2{font-size:.2892960463em}.katex .fontsize-ensurer.reset-size10.size3,.katex .sizing.reset-size10.size3{font-size:.337512054em}.katex .fontsize-ensurer.reset-size10.size4,.katex .sizing.reset-size10.size4{font-size:.3857280617em}.katex .fontsize-ensurer.reset-size10.size5,.katex .sizing.reset-size10.size5{font-size:.4339440694em}.katex .fontsize-ensurer.reset-size10.size6,.katex .sizing.reset-size10.size6{font-size:.4821600771em}.katex .fontsize-ensurer.reset-size10.size7,.katex .sizing.reset-size10.size7{font-size:.5785920926em}.katex .fontsize-ensurer.reset-size10.size8,.katex .sizing.reset-size10.size8{font-size:.6943105111em}.katex .fontsize-ensurer.reset-size10.size9,.katex .sizing.reset-size10.size9{font-size:.8331726133em}.katex .fontsize-ensurer.reset-size10.size10,.katex .sizing.reset-size10.size10{font-size:1em}.katex .fontsize-ensurer.reset-size10.size11,.katex .sizing.reset-size10.size11{font-size:1.1996142719em}.katex .fontsize-ensurer.reset-size11.size1,.katex .sizing.reset-size11.size1{font-size:.2009646302em}.katex .fontsize-ensurer.reset-size11.size2,.katex .sizing.reset-size11.size2{font-size:.2411575563em}.katex .fontsize-ensurer.reset-size11.size3,.katex .sizing.reset-size11.size3{font-size:.2813504823em}.katex .fontsize-ensurer.reset-size11.size4,.katex .sizing.reset-size11.size4{font-size:.3215434084em}.katex .fontsize-ensurer.reset-size11.size5,.katex .sizing.reset-size11.size5{font-size:.3617363344em}.katex .fontsize-ensurer.reset-size11.size6,.katex .sizing.reset-size11.size6{font-size:.4019292605em}.katex .fontsize-ensurer.reset-size11.size7,.katex .sizing.reset-size11.size7{font-size:.4823151125em}.katex .fontsize-ensurer.reset-size11.size8,.katex .sizing.reset-size11.size8{font-size:.578778135em}.katex .fontsize-ensurer.reset-size11.size9,.katex .sizing.reset-size11.size9{font-size:.6945337621em}.katex .fontsize-ensurer.reset-size11.size10,.katex .sizing.reset-size11.size10{font-size:.8336012862em}.katex .fontsize-ensurer.reset-size11.size11,.katex .sizing.reset-size11.size11{font-size:1em}.katex .delimsizing.size1{font-family:KaTeX_Size1}.katex .delimsizing.size2{font-family:KaTeX_Size2}.katex .delimsizing.size3{font-family:KaTeX_Size3}.katex .delimsizing.size4{font-family:KaTeX_Size4}.katex .delimsizing.mult .delim-size1>span{font-family:KaTeX_Size1}.katex .delimsizing.mult .delim-size4>span{font-family:KaTeX_Size4}.katex .nulldelimiter{display:inline-block;width:.12em}.katex .delimcenter,.katex .op-symbol{position:relative}.katex .op-symbol.small-op{font-family:KaTeX_Size1}.katex .op-symbol.large-op{font-family:KaTeX_Size2}.katex .accent>.vlist-t,.katex .op-limits>.vlist-t{text-align:center}.katex .accent .accent-body{position:relative}.katex .accent .accent-body:not(.accent-full){width:0}.katex .overlay{display:block}.katex .mtable .vertical-separator{display:inline-block;min-width:1px}.katex .mtable .arraycolsep{display:inline-block}.katex .mtable .col-align-c>.vlist-t{text-align:center}.katex .mtable .col-align-l>.vlist-t{text-align:left}.katex .mtable .col-align-r>.vlist-t{text-align:right}.katex .svg-align{text-align:left}.katex svg{fill:currentColor;stroke:currentColor;fill-rule:nonzero;fill-opacity:1;stroke-width:1;stroke-linecap:butt;stroke-linejoin:miter;stroke-miterlimit:4;stroke-dasharray:none;stroke-dashoffset:0;stroke-opacity:1;display:block;height:inherit;position:absolute;width:100%}.katex svg path{stroke:none}.katex img{border-style:none;max-height:none;max-width:none;min-height:0;min-width:0}.katex .stretchy{display:block;overflow:hidden;position:relative;width:100%}.katex .stretchy:after,.katex .stretchy:before{content:""}.katex .hide-tail{overflow:hidden;position:relative;width:100%}.katex .halfarrow-left{left:0;overflow:hidden;position:absolute;width:50.2%}.katex .halfarrow-right{overflow:hidden;position:absolute;right:0;width:50.2%}.katex .brace-left{left:0;overflow:hidden;position:absolute;width:25.1%}.katex .brace-center{left:25%;overflow:hidden;position:absolute;width:50%}.katex .brace-right{overflow:hidden;position:absolute;right:0;width:25.1%}.katex .x-arrow-pad{padding:0 .5em}.katex .cd-arrow-pad{padding:0 .55556em 0 .27778em}.katex .mover,.katex .munder,.katex .x-arrow{text-align:center}.katex .boxpad{padding:0 .3em}.katex .fbox,.katex .fcolorbox{border:.04em solid;box-sizing:border-box}.katex .cancel-pad{padding:0 .2em}.katex .cancel-lap{margin-left:-.2em;margin-right:-.2em}.katex .sout{border-bottom-style:solid;border-bottom-width:.08em}.katex .angl{border-right:.049em solid;border-top:.049em solid;box-sizing:border-box;margin-right:.03889em}.katex .anglpad{padding:0 .03889em}.katex .eqn-num:before{content:"(" counter(katexEqnNo) ")";counter-increment:katexEqnNo}.katex .mml-eqn-num:before{content:"(" counter(mmlEqnNo) ")";counter-increment:mmlEqnNo}.katex .mtr-glue{width:50%}.katex .cd-vert-arrow{display:inline-block;position:relative}.katex .cd-label-left{display:inline-block;position:absolute;right:calc(50% + .3em);text-align:left}.katex .cd-label-right{display:inline-block;left:calc(50% + .3em);position:absolute;text-align:right}.katex-display{display:block;margin:1em 0;text-align:center}.katex-display>.katex{display:block;text-align:center;white-space:nowrap}.katex-display>.katex>.katex-html{display:block;position:relative}.katex-display>.katex>.katex-html>.tag{position:absolute;right:0}.katex-display.leqno>.katex>.katex-html>.tag{left:0;right:auto}.katex-display.fleqn>.katex{padding-left:2em;text-align:left}body{counter-reset:katexEqnNo mmlEqnNo}"#;

/// `(display, output_discriminant, latex)` -> rendered markup.
///
/// `OutputFormat` derives `Eq` but not `Hash`, so we key on a small discriminant
/// instead of the enum. Unbounded, process-global: ideal for batch/CLI use, but
/// bound it (LRU) or scope it per pass in a long-running server.
static CACHE: LazyLock<RwLock<HashMap<(bool, u8, String), Arc<str>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Map a Python-facing string to an `OutputFormat`. Case-insensitive.
fn output_from_str(s: &str) -> Option<OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "both" | "htmlandmathml" | "html_and_mathml" => Some(OutputFormat::HtmlAndMathml),
        "html" => Some(OutputFormat::Html),
        "mathml" => Some(OutputFormat::Mathml),
        _ => None,
    }
}

/// Stable cache discriminant for an `OutputFormat`.
fn output_disc(o: OutputFormat) -> u8 {
    match o {
        OutputFormat::HtmlAndMathml => 0,
        OutputFormat::Html => 1,
        OutputFormat::Mathml => 2,
    }
}

/// Render one LaTeX expression to KaTeX markup in the requested format, memoized.
///
/// Never fails: a KaTeX `ParseError` yields an HTML-escaped error span containing
/// the original source (KaTeX's `throw_on_error = false` behavior) instead of
/// aborting the surrounding document.
pub fn render_math_cached(latex: &str, display: bool, output: OutputFormat) -> Arc<str> {
    let key = (display, output_disc(output), latex.to_owned());

    if let Some(hit) = CACHE.read().unwrap().get(&key) {
        return hit.clone();
    }

    let markup: Arc<str> = Arc::from(render_one(latex, display, output));

    let mut w = CACHE.write().unwrap();
    w.entry(key).or_insert_with(|| markup.clone()).clone()
}

fn render_one(latex: &str, display: bool, output: OutputFormat) -> String {
    // Settings is cheap relative to a render; build per call, no shared macro state.
    let settings = Settings::builder()
        .display_mode(display)
        .output(output)
        // Warn-and-continue on questionable input rather than erroring out.
        .strict(StrictSetting::Mode(StrictMode::Warn))
        // `trust` defaults to false: blocks \includegraphics, \href, etc. Keep it
        // false because markdown / PDF-extracted math is typically untrusted.
        .build();

    match render_to_string(&*KATEX, latex, &settings) {
        Ok(markup) => markup,
        // Note: this fallback is an HTML <span>. In an HTML document body it renders
        // fine even alongside MathML; if you insert "mathml"-only output into a strict
        // MathML context, swap this for a <math><merror>...</merror></math>.
        Err(err) => error_fallback(latex, &err.to_string()),
    }
}

/// HTML-escaped fallback span. Both the source and the message are escaped: the
/// LaTeX is untrusted input and the message embeds a slice of it, so emitting
/// either raw would allow HTML/script injection.
fn error_fallback(latex: &str, message: &str) -> String {
    format!(
        "<span class=\"katex-error\" title=\"{}\">{}</span>",
        escape_html(message),
        escape_html(latex),
    )
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

// -----------------------------------------------------------------------------
// Python surface
// -----------------------------------------------------------------------------

/// mordant.render_math(latex, display=False, output="both") -> str
///
/// Render a LaTeX expression to KaTeX markup. `output` is one of "both"
/// (HTML+MathML, default), "html", or "mathml". Independent of the Markdown AST,
/// so it works with no parser changes. The GIL is released during the render.
///
/// ```python
/// import mordant
/// # MathML-only, for a QtWebEngine (Chromium >= 109) view with no CSS/fonts:
/// mordant.render_math(r"\int_0^\infty e^{-x^2}\,dx", display=True, output="mathml")
/// ```
#[pyfunction]
#[pyo3(signature = (latex, display = false, output = "both"))]
pub fn render_math(
    py: Python<'_>,
    latex: &str,
    display: bool,
    output: &str,
) -> PyResult<Py<PyString>> {
    let fmt = output_from_str(output).ok_or_else(|| {
        PyValueError::new_err(format!(
            "output must be 'html', 'mathml', or 'both', got {output:?}"
        ))
    })?;

    let markup = py.detach(move || render_math_cached(latex, display, fmt));
    Ok(PyString::new(py, &markup).unbind())
}

// -----------------------------------------------------------------------------
// Rushdown renderer extension — always intercepts ```math / ```latex blocks
// -----------------------------------------------------------------------------

/// Options for the math HTML renderer extension.
#[derive(Debug, Clone)]
pub struct MathRendererOptions {
    /// Output format for math blocks (default: HtmlAndMathml = "both").
    pub output: OutputFormat,
}

impl Default for MathRendererOptions {
    fn default() -> Self {
        Self {
            output: OutputFormat::HtmlAndMathml,
        }
    }
}

impl rushdown_lib::renderer::RendererOptions for MathRendererOptions {}

/// Python-exposed math renderer options.
#[pyclass(module = "mordant", name = "MathRendererOptions")]
#[derive(Clone)]
pub struct PyMathRendererOptions {
    #[pyo3(get, set)]
    pub output: String,
}

#[pymethods]
impl PyMathRendererOptions {
    #[new]
    #[pyo3(signature = (output = "both"))]
    pub fn new(output: &str) -> Self {
        PyMathRendererOptions {
            output: output.to_string(),
        }
    }
}

impl PyMathRendererOptions {
    pub fn to_rushdown(&self) -> MathRendererOptions {
        MathRendererOptions {
            output: output_from_str(&self.output).unwrap_or(OutputFormat::HtmlAndMathml),
        }
    }
}

struct MathHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    writer: html::Writer,
    options: MathRendererOptions,
}

impl<W: TextWrite> MathHtmlRenderer<W> {
    fn new(html_opts: html::Options, options: MathRendererOptions) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
            writer: html::Writer::with_options(html_opts),
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for MathHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _ctx: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if entering {
            let kd = as_kind_data!(arena, node_ref, CodeBlock);
            let mut code = String::new();
            for line in kd.value().iter(source) {
                code.push_str(&line);
            }
            let lang = kd.language_str(source).unwrap_or("plaintext");

            if lang == "math" || lang == "latex" {
                let latex = code.trim_end_matches('\n');
                let markup = render_math_cached(latex, true, self.options.output);
                w.write_str(&markup)?;
            } else {
                // Render as plain code block (default behavior for non-math)
                self.writer.write_safe_str(w, "<pre><code")?;
                if let Some(lang_str) = kd.language_str(source) {
                    self.writer.write_safe_str(w, " class=\"language-")?;
                    self.writer.write(w, lang_str)?;
                    self.writer.write_safe_str(w, "\"")?;
                }
                self.writer.write_safe_str(w, ">")?;
                for line in kd.value().iter(source) {
                    self.writer.raw_write(w, &line)?;
                }
                self.writer.write_safe_str(w, "</code></pre>\n")?;
            }
        }
        Ok(WalkStatus::Continue)
    }
}


impl<'cb, W> NodeRenderer<'cb, W> for MathHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<CodeBlock>(), BoxRenderNode::new(self));
    }
}

/// Build a math renderer extension that is always active.
pub fn math_html_renderer_extension<'cb, W>(
    options: MathRendererOptions,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut html::Renderer<'cb, W>| {
        r.add_node_renderer(MathHtmlRenderer::new, options);
    })
}

// -----------------------------------------------------------------------------
// Level 2: Inline $...$ math parser extension
// -----------------------------------------------------------------------------

use rushdown_lib::ast::{KindData, NodeKind, NodeType, PrettyPrint, pp_indent};
use core::fmt;
use core::fmt::Write as FmtWrite;
use rushdown_lib::parser::{self, AnyInlineParser, InlineParser, Parser, ParserExtension, ParserExtensionFn, ParserOptions, PRIORITY_EMPHASIS};
use rushdown_lib::text::Reader;
use rushdown_lib::as_extension_data;

/// Represents an inline math expression in the AST.
#[derive(Debug)]
pub struct MathData {
    /// The raw LaTeX source (without delimiters).
    latex: String,
    /// Whether this is display math (`$$...$$`) or inline math (`$...$`).
    display: bool,
}

impl MathData {
    pub fn new(latex: String, display: bool) -> Self {
        Self { latex, display }
    }

    pub fn latex(&self) -> &str {
        &self.latex
    }

    pub fn display(&self) -> bool {
        self.display
    }
}

impl NodeKind for MathData {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "Math"
    }
}

impl PrettyPrint for MathData {
    fn pretty_print(&self, w: &mut dyn FmtWrite, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}latex: {:?}", pp_indent(level), self.latex)?;
        writeln!(w, "{}display: {}", pp_indent(level), self.display)
    }
}

impl From<MathData> for KindData {
    fn from(m: MathData) -> Self {
        KindData::Extension(Box::new(m))
    }
}

/// Options for the math parser extension.
#[derive(Debug, Clone)]
pub struct MathParserOptions {
    /// Whether to enable inline `$...$` math (default: true).
    pub inline_math: bool,
    /// Whether to enable display `$$...$$` math (default: true).
    pub display_math: bool,
}

impl Default for MathParserOptions {
    fn default() -> Self {
        Self {
            inline_math: true,
            display_math: true,
        }
    }
}

impl ParserOptions for MathParserOptions {}

#[derive(Debug)]
struct MathParser {
    options: MathParserOptions,
}

impl MathParser {
    fn with_options(options: MathParserOptions) -> Self {
        Self { options }
    }
}

impl InlineParser for MathParser {
    fn trigger(&self) -> &[u8] {
        b"$"
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut rushdown_lib::text::BlockReader,
        _ctx: &mut parser::Context,
    ) -> Option<NodeRef> {
        let (line, _) = reader.peek_line_bytes()?;
        if line.len() < 2 || line[0] != b'$' {
            return None;
        }

        // Check for display math `$$`
        let is_display = line.len() >= 2 && line[1] == b'$';
        if is_display && !self.options.display_math {
            return None;
        }
        if !is_display && !self.options.inline_math {
            return None;
        }

        let delimiter_len = if is_display { 2 } else { 1 };

        // Find the closing delimiter
        let search_start = delimiter_len;
        if line.len() <= search_start {
            return None;
        }

        // For inline math, don't allow spaces right after/before $
        if !is_display {
            if line.len() > search_start && line[search_start] == b' ' {
                return None;
            }
        }

        // Search for closing delimiter
        let mut search = search_start;
        let mut found = None;
        while search + delimiter_len <= line.len() {
            let match_close = if is_display {
                line[search] == b'$' && line[search + 1] == b'$'
            } else {
                line[search] == b'$'
            };
            if match_close {
                // For inline math, don't allow space before closing $
                if !is_display && search > search_start && line[search - 1] == b' ' {
                    search += 1;
                    continue;
                }
                found = Some(search);
                break;
            }
            search += 1;
        }

        // Multi-line display math: a `$$` at column 0 with no same-line closer,
        // content on subsequent lines, and a closing `$$` on a later line. Only
        // attempted after the single-line scan above fails, and only when the
        // opening `$$` is at the line start, so inline `$…$` / `$$…$$` parsing is
        // completely unaffected.
        if is_display && found.is_none() && reader.line_offset() == 0 {
            if let Some(latex) = self.parse_multiline_display(reader, &line) {
                return Some(arena.new_node(MathData::new(latex, true)));
            }
            return None;
        }

        let end = found?;
        let latex_bytes = &line[search_start..end];
        let latex = core::str::from_utf8(latex_bytes).unwrap_or("").to_string();

        if latex.is_empty() {
            return None;
        }

        reader.advance(end + delimiter_len);
        Some(arena.new_node(MathData::new(latex, is_display)))
    }
}

impl MathParser {
    /// Parse a multi-line display-math block beginning with an opening `$$` on its
    /// own line. Consumes following lines until a closing `$$` is found. If no
    /// closer exists, the reader position is restored and `None` is returned so an
    /// unbalanced `$$` remains literal text (no content is silently consumed).
    fn parse_multiline_display(
        &self,
        reader: &mut rushdown_lib::text::BlockReader,
        opening_line: &[u8],
    ) -> Option<String> {
        let (saved_line, saved_pos) = reader.position();

        let mut latex: Vec<u8> = Vec::new();
        // Content after the opening `$$` on the first line (if any).
        let rest = &opening_line[2..];
        if rest.iter().any(|b| !b.is_ascii_whitespace()) {
            latex.extend_from_slice(rest);
            latex.push(b'\n');
        }

        // Advance past the entire opening line to the next line.
        reader.advance(opening_line.len());

        let mut closed = false;
        loop {
            let (nxt, _) = match reader.peek_line_bytes() {
                Some(x) => x,
                None => break,
            };
            // Locate a `$$` on this line.
            let mut close_pos: Option<usize> = None;
            let mut i = 0;
            while i + 1 < nxt.len() {
                if nxt[i] == b'$' && nxt[i + 1] == b'$' {
                    close_pos = Some(i);
                    break;
                }
                i += 1;
            }
            match close_pos {
                Some(pos) => {
                    if pos > 0 {
                        latex.extend_from_slice(&nxt[..pos]);
                    }
                    reader.advance(pos + 2);
                    closed = true;
                    break;
                }
                None => {
                    latex.extend_from_slice(&nxt);
                    latex.push(b'\n');
                    reader.advance(nxt.len());
                }
            }
        }

        if !closed {
            reader.set_position(saved_line, saved_pos);
            return None;
        }

        let latex_str = String::from_utf8_lossy(&latex).trim().to_string();
        if latex_str.is_empty() {
            reader.set_position(saved_line, saved_pos);
            return None;
        }
        Some(latex_str)
    }
}

impl From<MathParser> for AnyInlineParser {
    fn from(p: MathParser) -> Self {
        AnyInlineParser::Extension(Box::new(p))
    }
}

/// Returns a parser extension that parses `$...$` inline math.
pub fn math_parser_extension(options: MathParserOptions) -> impl ParserExtension {
    ParserExtensionFn::new(move |p: &mut Parser| {
        p.add_inline_parser(MathParser::with_options, options.clone(), PRIORITY_EMPHASIS - 50);
    })
}

// -----------------------------------------------------------------------------
// Level 2: Math HTML renderer extension (for inline $...$ nodes)
// -----------------------------------------------------------------------------

/// Options for the math inline HTML renderer.
#[derive(Debug, Clone)]
pub struct MathInlineRendererOptions {
    /// Output format for inline math (default: HtmlAndMathml = "both").
    pub output: OutputFormat,
}

impl Default for MathInlineRendererOptions {
    fn default() -> Self {
        Self {
            output: OutputFormat::HtmlAndMathml,
        }
    }
}

impl rushdown_lib::renderer::RendererOptions for MathInlineRendererOptions {}

struct MathInlineHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    options: MathInlineRendererOptions,
}

impl<W: TextWrite> MathInlineHtmlRenderer<W> {
    fn new(_html_opts: html::Options, options: MathInlineRendererOptions) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for MathInlineHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _ctx: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if entering {
            let math = as_extension_data!(arena, node_ref, MathData);
            let markup = render_math_cached(math.latex(), math.display(), self.options.output);
            w.write_str(&markup)?;
        }
        Ok(WalkStatus::Continue)
    }
}

impl<'cb, W> NodeRenderer<'cb, W> for MathInlineHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<MathData>(), BoxRenderNode::new(self));
    }
}

/// Build a math inline renderer extension for `$...$` nodes.
pub fn math_inline_html_renderer_extension<'cb, W>(
    options: MathInlineRendererOptions,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut html::Renderer<'cb, W>| {
        r.add_node_renderer(MathInlineHtmlRenderer::new, options);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlighter::{highlighting_html_renderer_extension, HighlightingRendererOptions};

    /// Render with only the math extensions (no code highlighting).
    fn render_math(source: &str) -> String {
        let parser_ext = math_parser_extension(MathParserOptions::default());
        let renderer_ext = math_html_renderer_extension(MathRendererOptions::default())
            .and(math_inline_html_renderer_extension(MathInlineRendererOptions::default()));
        let html_opts = html::Options::default();
        let mut result = String::new();
        let f = rushdown_lib::new_markdown_to_html(
            parser::Options::default(),
            html_opts,
            parser_ext,
            renderer_ext,
        );
        f(&mut result, source).unwrap();
        result
    }

    /// Render with math extensions AND the code highlighter active. Mirrors
    /// md_viewer, which always passes a highlighting theme.
    fn render_math_highlighted(source: &str) -> String {
        let parser_ext = math_parser_extension(MathParserOptions::default());
        let renderer_ext = math_html_renderer_extension(MathRendererOptions::default())
            .and(math_inline_html_renderer_extension(MathInlineRendererOptions::default()))
            .and(highlighting_html_renderer_extension(
                HighlightingRendererOptions::default(),
            ));
        let html_opts = html::Options::default();
        let mut result = String::new();
        let f = rushdown_lib::new_markdown_to_html(
            parser::Options::default(),
            html_opts,
            parser_ext,
            renderer_ext,
        );
        f(&mut result, source).unwrap();
        result
    }

    #[test]
    fn fenced_math_without_highlighting() {
        let h = render_math("```math\nE = mc^2\n```");
        assert!(h.contains("katex"), "fenced math should render KaTeX: {h}");
        assert!(h.contains("katex-display"), "fenced math is display mode: {h}");
        assert!(!h.contains("language-math"), "should not be a code block: {h}");
    }

    // Bug A: fenced math/latex must become KaTeX even when a theme is active.
    #[test]
    fn fenced_math_with_highlighting_bug_a() {
        let h = render_math_highlighted("```math\nE = mc^2\n```");
        assert!(h.contains("katex"), "fenced math under a theme should render KaTeX: {h}");
        assert!(h.contains("katex-display"), "fenced math is display mode: {h}");
        assert!(!h.contains("language-math"), "should not fall through to highlighting: {h}");
    }

    #[test]
    fn latex_fence_with_highlighting_bug_a() {
        let h = render_math_highlighted("```latex\nE = mc^2\n```");
        assert!(h.contains("katex"), "fenced latex under a theme should render KaTeX: {h}");
        assert!(!h.contains("language-latex"), "should not fall through to highlighting: {h}");
    }

    #[test]
    fn other_code_blocks_still_highlighted() {
        let h = render_math_highlighted("```python\nx = 1\n```");
        assert!(h.contains("language-python"), "non-math code should still highlight: {h}");
        assert!(!h.contains("katex"), "python block must not be treated as math: {h}");
    }

    #[test]
    fn inline_math_regression() {
        let h = render_math("Inline $x^2$ here.");
        assert!(h.contains("katex"), "inline math should render: {h}");
        assert!(!h.contains("katex-display"), "inline math is not display mode: {h}");
    }

    #[test]
    fn single_line_display_math_regression() {
        let h = render_math("$$x^2$$");
        assert!(h.contains("katex"), "single-line $$ should render: {h}");
        assert!(h.contains("katex-display"), "single-line $$ is display mode: {h}");
    }

    // Bug B: `$$` on its own line with content on following lines.
    #[test]
    fn multiline_display_math_bug_b() {
        let h = render_math("$$\nE = mc^2\n$$");
        assert!(h.contains("katex"), "multi-line $$ should render KaTeX: {h}");
        assert!(h.contains("katex-display"), "multi-line $$ is display mode: {h}");
        assert!(h.contains("E = mc"), "formula content preserved: {h}");
    }

    #[test]
    fn multiline_display_math_with_highlighting() {
        let h = render_math_highlighted("$$\nE = mc^2\n$$");
        assert!(h.contains("katex"), "multi-line $$ under a theme should render: {h}");
        assert!(h.contains("E = mc"), "formula content preserved: {h}");
    }

    #[test]
    fn unbalanced_display_math_stays_literal() {
        // No closing `$$`: must remain literal, not consume content.
        let h = render_math("$$\nthis has no closer");
        assert!(!h.contains("katex"), "unbalanced $$ must not render as math: {h}");
        assert!(h.contains("$$"), "the literal $$ should remain: {h}");
        assert!(h.contains("this has no closer"), "content must not be consumed: {h}");
    }

    #[test]
    fn katex_css_is_non_empty() {
        assert!(KATEX_CSS.len() > 10_000, "KATEX_CSS should be ~23KB, got {} bytes", KATEX_CSS.len());
        assert!(KATEX_CSS.contains("@font-face"), "CSS should contain font-face");
        assert!(KATEX_CSS.contains(".katex"), "CSS should contain .katex class");
        assert!(KATEX_CSS.contains(".katex-display"), "CSS should contain .katex-display");
        assert!(KATEX_CSS.contains("0.16.21"), "CSS should declare version 0.16.21");
    }
}
