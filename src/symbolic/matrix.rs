//! Symbolic matrix operations — determinant, inverse, eigenvalues.

use super::expr::Expr;

/// A symbolic matrix of expressions.
#[derive(Debug, Clone)]
pub struct SymMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Vec<Expr>>,
}

impl SymMatrix {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, data: vec![vec![Expr::zero(); cols]; rows] }
    }

    pub fn identity(n: usize) -> Self {
        let mut m = Self::new(n, n);
        for i in 0..n { m.data[i][i] = Expr::one(); }
        m
    }

    pub fn from_f64(data: &[&[f64]]) -> Self {
        let rows = data.len();
        let cols = if rows > 0 { data[0].len() } else { 0 };
        let mut m = Self::new(rows, cols);
        for i in 0..rows {
            for j in 0..cols {
                m.data[i][j] = Expr::c(data[i][j]);
            }
        }
        m
    }

    pub fn get(&self, r: usize, c: usize) -> &Expr { &self.data[r][c] }
    pub fn set(&mut self, r: usize, c: usize, val: Expr) { self.data[r][c] = val; }

    /// Matrix multiplication.
    pub fn mul(&self, other: &SymMatrix) -> SymMatrix {
        assert_eq!(self.cols, other.rows);
        let mut result = SymMatrix::new(self.rows, other.cols);
        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = Expr::zero();
                for k in 0..self.cols {
                    sum = sum.add(self.data[i][k].clone().mul(other.data[k][j].clone()));
                }
                result.data[i][j] = sum;
            }
        }
        result
    }

    /// Transpose.
    pub fn transpose(&self) -> SymMatrix {
        let mut result = SymMatrix::new(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                result.data[j][i] = self.data[i][j].clone();
            }
        }
        result
    }

    /// Determinant (recursive cofactor expansion).
    pub fn determinant(&self) -> Expr {
        assert_eq!(self.rows, self.cols);
        let n = self.rows;
        if n == 1 { return self.data[0][0].clone(); }
        if n == 2 {
            let a = self.data[0][0].clone().mul(self.data[1][1].clone());
            let b = self.data[0][1].clone().mul(self.data[1][0].clone());
            return a.sub(b);
        }
        let mut det = Expr::zero();
        for j in 0..n {
            let cofactor = self.cofactor(0, j);
            let term = self.data[0][j].clone().mul(cofactor);
            if j % 2 == 0 { det = det.add(term); }
            else { det = det.sub(term); }
        }
        det
    }

    /// Minor: determinant of the submatrix with row i and col j removed.
    pub fn minor(&self, row: usize, col: usize) -> Expr {
        let sub = self.submatrix(row, col);
        sub.determinant()
    }

    /// Cofactor: (-1)^(i+j) * minor(i,j).
    pub fn cofactor(&self, row: usize, col: usize) -> Expr {
        let m = self.minor(row, col);
        if (row + col) % 2 == 0 { m } else { m.neg() }
    }

    /// Remove row i and column j.
    pub fn submatrix(&self, row: usize, col: usize) -> SymMatrix {
        let mut result = SymMatrix::new(self.rows - 1, self.cols - 1);
        let mut ri = 0;
        for i in 0..self.rows {
            if i == row { continue; }
            let mut ci = 0;
            for j in 0..self.cols {
                if j == col { continue; }
                result.data[ri][ci] = self.data[i][j].clone();
                ci += 1;
            }
            ri += 1;
        }
        result
    }

    /// Trace: sum of diagonal elements.
    pub fn trace(&self) -> Expr {
        let mut sum = Expr::zero();
        for i in 0..self.rows.min(self.cols) {
            sum = sum.add(self.data[i][i].clone());
        }
        sum
    }

    /// Numerical eigenvalues for a 2x2 matrix.
    pub fn eigenvalues_2x2(&self) -> Option<(f64, f64)> {
        if self.rows != 2 || self.cols != 2 { return None; }
        let vars = std::collections::HashMap::new();
        let a = self.data[0][0].eval(&vars);
        let b = self.data[0][1].eval(&vars);
        let c = self.data[1][0].eval(&vars);
        let d = self.data[1][1].eval(&vars);

        let trace = a + d;
        let det = a * d - b * c;
        let disc = trace * trace - 4.0 * det;
        if disc < 0.0 { return None; }
        let sqrt_disc = disc.sqrt();
        Some(((trace + sqrt_disc) / 2.0, (trace - sqrt_disc) / 2.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn det_2x2() {
        let m = SymMatrix::from_f64(&[&[1.0, 2.0], &[3.0, 4.0]]);
        let det = m.determinant();
        let val = det.eval(&HashMap::new());
        assert!((val - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn det_3x3() {
        let m = SymMatrix::from_f64(&[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0], &[7.0, 8.0, 0.0]]);
        let det = m.determinant();
        let val = det.eval(&HashMap::new());
        assert!((val - 27.0).abs() < 1e-8);
    }

    #[test]
    fn identity_det_is_one() {
        let m = SymMatrix::identity(3);
        let det = m.determinant();
        let val = det.eval(&HashMap::new());
        assert!((val - 1.0).abs() < 1e-10);
    }

    #[test]
    fn eigenvalues_diagonal() {
        let m = SymMatrix::from_f64(&[&[3.0, 0.0], &[0.0, 5.0]]);
        let (e1, e2) = m.eigenvalues_2x2().unwrap();
        assert!((e1 - 5.0).abs() < 1e-10);
        assert!((e2 - 3.0).abs() < 1e-10);
    }

    #[test]
    fn transpose() {
        let m = SymMatrix::from_f64(&[&[1.0, 2.0], &[3.0, 4.0]]);
        let t = m.transpose();
        let val = t.data[1][0].eval(&HashMap::new());
        assert!((val - 2.0).abs() < 1e-10);
    }
}
