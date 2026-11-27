//! Small TeX -> Unicode renderer.
//!
//! The goal is readable math inside chat without shipping a full TeX engine:
//! fractions, roots, scripts, greek letters, operators and common accents are
//! converted to Unicode. Display math is drawn larger and centered.

use egui::{Color32, RichText, Ui};

use crate::theme::{body_font, CHIP_FILL};

/// Inline math (`$...$` / `\(...\)`).
pub fn inline(ui: &mut Ui, tex: &str, color: Color32) {
    let text = to_unicode(tex);
    if text.trim().is_empty() {
        return;
    }
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .font(body_font(14.0))
                .italics()
                .color(color),
        )
        .wrap(),
    );
}

/// Display math (`$$...$$` / `\[...\]`).
pub fn display(ui: &mut Ui, tex: &str, color: Color32) {
    let text = to_unicode(tex);
    if text.trim().is_empty() {
        return;
    }

    ui.add_space(2.0);
    egui::Frame::new()
        .fill(CHIP_FILL)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(text)
                            .font(body_font(18.0))
                            .italics()
                            .color(color),
                    )
                    .wrap(),
                );
            });
        });
    ui.add_space(4.0);
}

/// Convert a TeX snippet to Unicode text.
pub fn to_unicode(tex: &str) -> String {
    let mut parser = Parser {
        chars: tex.chars().collect(),
        pos: 0,
        env: Vec::new(),
    };
    normalize(&parser.sequence(None))
}

fn normalize(input: &str) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for ch in input.chars() {
        match ch {
            ' ' | '\t' => {
                if !last_space && !out.is_empty() {
                    out.push(' ');
                    last_space = true;
                }
            }
            '\n' => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                last_space = false;
            }
            _ => {
                out.push(ch);
                last_space = false;
            }
        }
    }
    out.trim().to_string()
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    env: Vec<String>,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn sequence(&mut self, stop: Option<char>) -> String {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if Some(ch) == stop {
                self.pos += 1;
                break;
            }
            match ch {
                '{' => {
                    self.pos += 1;
                    out.push_str(&self.sequence(Some('}')));
                }
                '}' => {
                    self.pos += 1;
                }
                '\\' => {
                    self.pos += 1;
                    out.push_str(&self.command());
                }
                '^' => {
                    self.pos += 1;
                    let group = self.group();
                    out.push_str(&superscript(&group));
                }
                '_' => {
                    self.pos += 1;
                    let group = self.group();
                    out.push_str(&subscript(&group));
                }
                '~' => {
                    self.pos += 1;
                    out.push('\u{00A0}');
                }
                '&' => {
                    self.pos += 1;
                    out.push_str("  ");
                }
                '\n' | '\r' => {
                    self.pos += 1;
                    out.push(' ');
                }
                _ => {
                    self.pos += 1;
                    out.push(ch);
                }
            }
        }
        out
    }

    fn group(&mut self) -> String {
        self.skip_spaces();
        match self.peek() {
            Some('{') => {
                self.pos += 1;
                self.sequence(Some('}'))
            }
            Some('\\') => {
                self.pos += 1;
                self.command()
            }
            Some(_) => self.bump().map(String::from).unwrap_or_default(),
            None => String::new(),
        }
    }

    fn command(&mut self) -> String {
        let start = self.pos;
        if matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
                self.pos += 1;
            }
            let name: String = self.chars[start..self.pos].iter().collect();
            self.named_command(&name)
        } else {
            let ch = self.bump().unwrap_or(' ');
            match ch {
                ',' => "\u{2009}".into(),
                ';' | ':' => "\u{2005}".into(),
                '!' => String::new(),
                ' ' | '\n' | '\r' | '\t' => " ".into(),
                '\\' => "\n".into(),
                _ => ch.to_string(),
            }
        }
    }

    fn named_command(&mut self, name: &str) -> String {
        match name {
            "frac" | "dfrac" | "tfrac" => {
                let num = self.group();
                let den = self.group();
                fraction(&num, &den)
            }
            "sqrt" => {
                self.skip_spaces();
                let mut index = None;
                if self.peek() == Some('[') {
                    self.pos += 1;
                    index = Some(self.sequence(Some(']')));
                }
                let body = self.group();
                match index {
                    Some(n) => format!("{}√({})", superscript(&n), body),
                    None => format!("√({})", body),
                }
            }
            "text" | "textrm" | "mathrm" | "mathbf" | "mathit" | "mathsf"
            | "mathtt" | "operatorname" => self.group(),
            "mathbb" => blackboard(&self.group()),
            "mathcal" | "mathscr" => script(&self.group()),
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl"
            | "bigr" | "Bigl" | "Bigr" | "middle" => {
                self.skip_spaces();
                match self.peek() {
                    Some('\\') => {
                        self.pos += 1;
                        self.command()
                    }
                    Some(_) => self.bump().map(String::from).unwrap_or_default(),
                    None => String::new(),
                }
            }
            "begin" => {
                let env = self.group();
                self.env.push(env.clone());
                match env.as_str() {
                    "pmatrix" | "matrix" | "smallmatrix" => "(".into(),
                    "bmatrix" => "[".into(),
                    "Bmatrix" => "{\u{2009}".into(),
                    "vmatrix" => "|".into(),
                    "Vmatrix" => "‖".into(),
                    "cases" => "{\u{2009}".into(),
                    _ => String::new(),
                }
            }
            "end" => {
                let env = self.group();
                self.env.pop();
                match env.as_str() {
                    "pmatrix" | "matrix" | "smallmatrix" => ")".into(),
                    "bmatrix" => "]".into(),
                    "Bmatrix" => "\u{2009}}".into(),
                    "vmatrix" => "|".into(),
                    "Vmatrix" => "‖".into(),
                    "cases" => String::new(),
                    _ => String::new(),
                }
            }
            "overline" => accent(&self.group(), '\u{0305}'),
            "underline" => accent(&self.group(), '\u{0332}'),
            "bar" => accent(&self.group(), '\u{0304}'),
            "hat" | "widehat" => accent(&self.group(), '\u{0302}'),
            "tilde" | "widetilde" => accent(&self.group(), '\u{0303}'),
            "vec" | "overrightarrow" => accent(&self.group(), '\u{20D7}'),
            "dot" => accent(&self.group(), '\u{0307}'),
            "ddot" => accent(&self.group(), '\u{0308}'),
            "quad" => "\u{2003}".into(),
            "qquad" => "\u{2003}\u{2003}".into(),
            "hspace" | "vspace" | "phantom" | "hphantom" | "vphantom" => {
                let _ = self.group();
                String::new()
            }
            "label" | "tag" | "color" | "textcolor" | "style" => {
                let _ = self.group();
                String::new()
            }
            "limits" | "nolimits" | "displaystyle" | "textstyle" | "scriptstyle" => {
                String::new()
            }
            _ => symbol(name).unwrap_or_else(|| name.to_string()),
        }
    }
}

fn fraction(num: &str, den: &str) -> String {
    let simple = |s: &str| {
        let s = s.trim();
        !s.is_empty() && s.chars().count() <= 3 && s.chars().all(|c| c.is_alphanumeric())
    };
    if simple(num) && simple(den) {
        format!("{}/{}", num.trim(), den.trim())
    } else {
        format!("({})/({})", num.trim(), den.trim())
    }
}

fn accent(text: &str, mark: char) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        out.push(ch);
        out.push(mark);
    }
    out
}

fn superscript(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '0' => out.push('⁰'),
            '1' => out.push('¹'),
            '2' => out.push('²'),
            '3' => out.push('³'),
            '4' => out.push('⁴'),
            '5' => out.push('⁵'),
            '6' => out.push('⁶'),
            '7' => out.push('⁷'),
            '8' => out.push('⁸'),
            '9' => out.push('⁹'),
            '+' => out.push('⁺'),
            '-' | '−' => out.push('⁻'),
            '=' => out.push('⁼'),
            '(' => out.push('⁽'),
            ')' => out.push('⁾'),
            'n' => out.push('ⁿ'),
            'i' => out.push('ⁱ'),
            ' ' => {}
            _ => return format!("^({})", text),
        }
    }
    if out.is_empty() {
        String::new()
    } else {
        out
    }
}

fn subscript(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '0' => out.push('₀'),
            '1' => out.push('₁'),
            '2' => out.push('₂'),
            '3' => out.push('₃'),
            '4' => out.push('₄'),
            '5' => out.push('₅'),
            '6' => out.push('₆'),
            '7' => out.push('₇'),
            '8' => out.push('₈'),
            '9' => out.push('₉'),
            '+' => out.push('₊'),
            '-' | '−' => out.push('₋'),
            '=' => out.push('₌'),
            '(' => out.push('₍'),
            ')' => out.push('₎'),
            'a' => out.push('ₐ'),
            'e' => out.push('ₑ'),
            'h' => out.push('ₕ'),
            'i' => out.push('ᵢ'),
            'j' => out.push('ⱼ'),
            'k' => out.push('ₖ'),
            'l' => out.push('ₗ'),
            'm' => out.push('ₘ'),
            'n' => out.push('ₙ'),
            'o' => out.push('ₒ'),
            'p' => out.push('ₚ'),
            'r' => out.push('ᵣ'),
            's' => out.push('ₛ'),
            't' => out.push('ₜ'),
            'u' => out.push('ᵤ'),
            'v' => out.push('ᵥ'),
            'x' => out.push('ₓ'),
            ' ' => {}
            _ => return format!("_({})", text),
        }
    }
    if out.is_empty() {
        String::new()
    } else {
        out
    }
}

fn blackboard(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        out.push(match ch {
            'R' => 'ℝ',
            'N' => 'ℕ',
            'Z' => 'ℤ',
            'Q' => 'ℚ',
            'C' => 'ℂ',
            'P' => 'ℙ',
            'H' => 'ℍ',
            other => other,
        });
    }
    out
}

fn script(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        out.push(match ch {
            'A' => '𝒜',
            'B' => 'ℬ',
            'C' => '𝒞',
            'D' => '𝒟',
            'E' => 'ℰ',
            'F' => 'ℱ',
            'G' => '𝒢',
            'H' => 'ℋ',
            'I' => 'ℐ',
            'J' => '𝒥',
            'K' => '𝒦',
            'L' => 'ℒ',
            'M' => 'ℳ',
            'N' => '𝒩',
            'O' => '𝒪',
            'P' => '𝒫',
            'Q' => '𝒬',
            'R' => 'ℛ',
            'S' => '𝒮',
            'T' => '𝒯',
            'U' => '𝒰',
            'V' => '𝒱',
            'W' => '𝒲',
            'X' => '𝒳',
            'Y' => '𝒴',
            'Z' => '𝒵',
            other => other,
        });
    }
    out
}

fn symbol(name: &str) -> Option<String> {
    let value = match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ε",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "φ",
        "varphi" => "ϕ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "times" => "×",
        "div" => "÷",
        "cdot" => "·",
        "pm" => "±",
        "mp" => "∓",
        "leq" | "le" => "≤",
        "geq" | "ge" => "≥",
        "neq" | "ne" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "sim" => "∼",
        "simeq" => "≃",
        "cong" => "≅",
        "propto" => "∝",
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "in" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "subset" => "⊂",
        "subseteq" => "⊆",
        "supset" => "⊃",
        "supseteq" => "⊇",
        "cup" => "∪",
        "cap" => "∩",
        "setminus" => "∖",
        "emptyset" | "varnothing" => "∅",
        "wedge" | "land" => "∧",
        "vee" | "lor" => "∨",
        "neg" | "lnot" => "¬",
        "oplus" => "⊕",
        "otimes" => "⊗",
        "perp" => "⊥",
        "parallel" => "∥",
        "angle" => "∠",
        "triangle" => "△",
        "to" | "rightarrow" => "→",
        "leftarrow" | "gets" => "←",
        "Rightarrow" | "implies" => "⇒",
        "Leftarrow" => "⇐",
        "leftrightarrow" => "↔",
        "Leftrightarrow" | "iff" => "⇔",
        "mapsto" => "↦",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "cdots" => "⋯",
        "ldots" | "dots" => "…",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "prime" => "′",
        "degree" => "°",
        "circ" => "∘",
        "ast" => "∗",
        "star" => "⋆",
        "bullet" => "•",
        "ell" => "ℓ",
        "hbar" => "ℏ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        "therefore" => "∴",
        "because" => "∵",
        "langle" => "⟨",
        "rangle" => "⟩",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "vert" | "mid" => "|",
        "Vert" => "‖",
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "arcsin" | "arccos"
        | "arctan" | "sinh" | "cosh" | "tanh" | "log" | "ln" | "exp" | "lim"
        | "max" | "min" | "sup" | "inf" | "det" | "dim" | "ker" | "deg"
        | "gcd" | "lcm" | "mod" | "bmod" => {
            return Some(format!("{} ", name));
        }
        _ => return None,
    };
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::to_unicode;

    #[test]
    fn plain_text_is_kept() {
        assert_eq!(to_unicode("x + y"), "x + y");
    }

    #[test]
    fn powers_and_subscripts_use_unicode() {
        assert_eq!(to_unicode("x^2"), "x²");
        assert_eq!(to_unicode("a_i"), "aᵢ");
    }

    #[test]
    fn fractions_are_rendered() {
        assert_eq!(to_unicode("\\frac{a}{b}"), "a/b");
        assert_eq!(to_unicode("\\frac{a+b}{c}"), "(a+b)/(c)");
    }

    #[test]
    fn roots_and_greek_are_translated() {
        assert_eq!(to_unicode("\\sqrt{x}"), "√(x)");
        assert_eq!(to_unicode("\\alpha + \\beta"), "α + β");
        assert_eq!(to_unicode("\\sum_{i=1}^{n} x_i"), "∑ᵢ₌₁ⁿ xᵢ");
    }

    #[test]
    fn text_commands_keep_their_words() {
        assert_eq!(to_unicode("\\text{hello world}"), "hello world");
    }
}

