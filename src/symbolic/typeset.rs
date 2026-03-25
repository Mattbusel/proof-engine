//! Mathematical typesetting — render equations as positioned glyphs with proper notation.

use super::expr::Expr;
use glam::{Vec2, Vec4};

/// A positioned glyph for mathematical typesetting.
#[derive(Debug, Clone)]
pub struct MathGlyph {
    pub character: char,
    pub position: Vec2,
    pub scale: f32,
    pub color: Vec4,
}

/// A typeset expression ready for rendering.
#[derive(Debug, Clone)]
pub struct TypesetExpr {
    pub glyphs: Vec<MathGlyph>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

impl TypesetExpr {
    /// Typeset an expression starting at the given position.
    pub fn layout(expr: &Expr, start: Vec2, base_scale: f32, color: Vec4) -> Self {
        let mut glyphs = Vec::new();
        let mut cursor = start;
        let bounds = layout_expr(expr, &mut cursor, base_scale, color, &mut glyphs, 0);
        Self {
            glyphs,
            width: bounds.0,
            height: bounds.1,
            baseline: start.y,
        }
    }
}

fn layout_expr(
    expr: &Expr, cursor: &mut Vec2, scale: f32, color: Vec4,
    glyphs: &mut Vec<MathGlyph>, depth: u32,
) -> (f32, f32) {
    let start_x = cursor.x;
    let char_w = scale * 0.6;
    let char_h = scale;

    match expr {
        Expr::Const(v) => {
            let s = if v.fract() == 0.0 && v.abs() < 1e12 { format!("{}", *v as i64) } else { format!("{v:.2}") };
            for ch in s.chars() {
                glyphs.push(MathGlyph { character: ch, position: *cursor, scale, color });
                cursor.x += char_w;
            }
        }
        Expr::Var(name) => {
            for ch in name.chars() {
                glyphs.push(MathGlyph { character: ch, position: *cursor, scale, color });
                cursor.x += char_w;
            }
        }
        Expr::Add(a, b) => {
            layout_expr(a, cursor, scale, color, glyphs, depth);
            push_char('+', cursor, char_w, scale, color, glyphs);
            layout_expr(b, cursor, scale, color, glyphs, depth);
        }
        Expr::Sub(a, b) => {
            layout_expr(a, cursor, scale, color, glyphs, depth);
            push_char('-', cursor, char_w, scale, color, glyphs);
            layout_expr(b, cursor, scale, color, glyphs, depth);
        }
        Expr::Mul(a, b) => {
            let needs_parens_a = matches!(**a, Expr::Add(_, _) | Expr::Sub(_, _));
            let needs_parens_b = matches!(**b, Expr::Add(_, _) | Expr::Sub(_, _));
            if needs_parens_a { push_char('(', cursor, char_w, scale, color, glyphs); }
            layout_expr(a, cursor, scale, color, glyphs, depth);
            if needs_parens_a { push_char(')', cursor, char_w, scale, color, glyphs); }
            push_char('·', cursor, char_w, scale, color, glyphs);
            if needs_parens_b { push_char('(', cursor, char_w, scale, color, glyphs); }
            layout_expr(b, cursor, scale, color, glyphs, depth);
            if needs_parens_b { push_char(')', cursor, char_w, scale, color, glyphs); }
        }
        Expr::Div(a, b) => {
            // Fraction: numerator above, line, denominator below
            let frac_scale = scale * 0.8;
            let num_y = cursor.y + char_h * 0.5;
            let den_y = cursor.y - char_h * 0.5;
            let save_x = cursor.x;

            let mut num_cursor = Vec2::new(cursor.x, num_y);
            layout_expr(a, &mut num_cursor, frac_scale, color, glyphs, depth + 1);
            let num_width = num_cursor.x - save_x;

            let mut den_cursor = Vec2::new(cursor.x, den_y);
            layout_expr(b, &mut den_cursor, frac_scale, color, glyphs, depth + 1);
            let den_width = den_cursor.x - save_x;

            // Fraction line
            let line_width = num_width.max(den_width);
            let line_y = cursor.y;
            for i in 0..(line_width / (char_w * 0.5)).ceil() as usize {
                glyphs.push(MathGlyph {
                    character: '─',
                    position: Vec2::new(save_x + i as f32 * char_w * 0.5, line_y),
                    scale: frac_scale,
                    color,
                });
            }
            cursor.x = save_x + line_width + char_w * 0.3;
        }
        Expr::Pow(base, exp) => {
            layout_expr(base, cursor, scale, color, glyphs, depth);
            // Superscript
            let exp_scale = scale * 0.6;
            let exp_y = cursor.y + char_h * 0.5;
            let mut exp_cursor = Vec2::new(cursor.x, exp_y);
            layout_expr(exp, &mut exp_cursor, exp_scale, color, glyphs, depth + 1);
            cursor.x = exp_cursor.x;
        }
        Expr::Sqrt(a) => {
            push_char('√', cursor, char_w, scale, color, glyphs);
            push_char('(', cursor, char_w, scale, color, glyphs);
            layout_expr(a, cursor, scale, color, glyphs, depth);
            push_char(')', cursor, char_w, scale, color, glyphs);
        }
        Expr::Sin(a) => { push_str("sin", cursor, char_w, scale, color, glyphs); push_char('(', cursor, char_w, scale, color, glyphs); layout_expr(a, cursor, scale, color, glyphs, depth); push_char(')', cursor, char_w, scale, color, glyphs); }
        Expr::Cos(a) => { push_str("cos", cursor, char_w, scale, color, glyphs); push_char('(', cursor, char_w, scale, color, glyphs); layout_expr(a, cursor, scale, color, glyphs, depth); push_char(')', cursor, char_w, scale, color, glyphs); }
        Expr::Tan(a) => { push_str("tan", cursor, char_w, scale, color, glyphs); push_char('(', cursor, char_w, scale, color, glyphs); layout_expr(a, cursor, scale, color, glyphs, depth); push_char(')', cursor, char_w, scale, color, glyphs); }
        Expr::Ln(a) => { push_str("ln", cursor, char_w, scale, color, glyphs); push_char('(', cursor, char_w, scale, color, glyphs); layout_expr(a, cursor, scale, color, glyphs, depth); push_char(')', cursor, char_w, scale, color, glyphs); }
        Expr::Exp(a) => { push_char('e', cursor, char_w, scale, color, glyphs); let mut sc = Vec2::new(cursor.x, cursor.y + char_h * 0.5); layout_expr(a, &mut sc, scale * 0.6, color, glyphs, depth + 1); cursor.x = sc.x; }
        Expr::Neg(a) => { push_char('-', cursor, char_w, scale, color, glyphs); layout_expr(a, cursor, scale, color, glyphs, depth); }
        Expr::Sum { body, var, from, to } => {
            push_char('Σ', cursor, char_w * 1.5, scale * 1.3, color, glyphs);
            layout_expr(body, cursor, scale, color, glyphs, depth);
        }
        Expr::Integral { body, var } => {
            push_char('∫', cursor, char_w * 1.2, scale * 1.3, color, glyphs);
            layout_expr(body, cursor, scale, color, glyphs, depth);
            push_str(&format!("d{var}"), cursor, char_w, scale * 0.8, color, glyphs);
        }
        _ => {
            let s = format!("{expr}");
            push_str(&s, cursor, char_w, scale, color, glyphs);
        }
    }

    let width = cursor.x - start_x;
    (width, char_h)
}

fn push_char(ch: char, cursor: &mut Vec2, char_w: f32, scale: f32, color: Vec4, glyphs: &mut Vec<MathGlyph>) {
    glyphs.push(MathGlyph { character: ch, position: *cursor, scale, color });
    cursor.x += char_w;
}

fn push_str(s: &str, cursor: &mut Vec2, char_w: f32, scale: f32, color: Vec4, glyphs: &mut Vec<MathGlyph>) {
    for ch in s.chars() {
        push_char(ch, cursor, char_w, scale, color, glyphs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typeset_simple_expr() {
        let expr = Expr::var("x").pow(Expr::c(2.0)).add(Expr::c(1.0));
        let ts = TypesetExpr::layout(&expr, Vec2::ZERO, 1.0, Vec4::ONE);
        assert!(!ts.glyphs.is_empty());
        assert!(ts.width > 0.0);
    }

    #[test]
    fn typeset_fraction() {
        let expr = Expr::var("x").div(Expr::var("y"));
        let ts = TypesetExpr::layout(&expr, Vec2::ZERO, 1.0, Vec4::ONE);
        assert!(ts.glyphs.len() >= 2); // at least x, y, and fraction line
    }

    #[test]
    fn typeset_trig() {
        let expr = Expr::var("x").sin();
        let ts = TypesetExpr::layout(&expr, Vec2::ZERO, 1.0, Vec4::ONE);
        let chars: Vec<char> = ts.glyphs.iter().map(|g| g.character).collect();
        assert!(chars.contains(&'s'));
    }
}
