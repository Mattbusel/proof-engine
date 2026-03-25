//! Disease propagation — SIR/SIS models with spatial spread.

/// SIR model state.
#[derive(Debug, Clone)]
pub struct SirState {
    pub susceptible: f64,
    pub infected: f64,
    pub recovered: f64,
    pub beta: f64,    // transmission rate
    pub gamma: f64,   // recovery rate
}

impl SirState {
    pub fn new(pop: f64, initial_infected: f64, beta: f64, gamma: f64) -> Self {
        Self {
            susceptible: pop - initial_infected,
            infected: initial_infected,
            recovered: 0.0,
            beta, gamma,
        }
    }

    pub fn total(&self) -> f64 { self.susceptible + self.infected + self.recovered }

    /// Basic reproduction number.
    pub fn r0(&self) -> f64 { self.beta / self.gamma }

    /// Step the SIR model.
    pub fn step(&mut self, dt: f64) {
        let n = self.total();
        if n < 1.0 { return; }
        let new_infections = self.beta * self.susceptible * self.infected / n * dt;
        let new_recoveries = self.gamma * self.infected * dt;

        self.susceptible -= new_infections;
        self.infected += new_infections - new_recoveries;
        self.recovered += new_recoveries;

        self.susceptible = self.susceptible.max(0.0);
        self.infected = self.infected.max(0.0);
        self.recovered = self.recovered.max(0.0);
    }

    /// Is the epidemic over?
    pub fn is_over(&self) -> bool { self.infected < 0.5 }
}

/// SIS model (no immunity — recovered become susceptible again).
#[derive(Debug, Clone)]
pub struct SisState {
    pub susceptible: f64,
    pub infected: f64,
    pub beta: f64,
    pub gamma: f64,
}

impl SisState {
    pub fn new(pop: f64, initial_infected: f64, beta: f64, gamma: f64) -> Self {
        Self { susceptible: pop - initial_infected, infected: initial_infected, beta, gamma }
    }

    pub fn step(&mut self, dt: f64) {
        let n = self.susceptible + self.infected;
        if n < 1.0 { return; }
        let new_infections = self.beta * self.susceptible * self.infected / n * dt;
        let new_recoveries = self.gamma * self.infected * dt;
        self.susceptible += new_recoveries - new_infections;
        self.infected += new_infections - new_recoveries;
        self.susceptible = self.susceptible.max(0.0);
        self.infected = self.infected.max(0.0);
    }

    /// Endemic equilibrium infected fraction.
    pub fn endemic_fraction(&self) -> f64 {
        if self.beta <= self.gamma { 0.0 }
        else { 1.0 - self.gamma / self.beta }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sir_epidemic() {
        let mut sir = SirState::new(1000.0, 10.0, 0.3, 0.1);
        assert!(sir.r0() > 1.0, "R0 should be > 1 for epidemic");
        for _ in 0..10000 { sir.step(0.1); }
        assert!(sir.is_over(), "epidemic should resolve");
        assert!(sir.recovered > 500.0, "most should have been infected");
    }

    #[test]
    fn test_sir_no_epidemic() {
        let mut sir = SirState::new(1000.0, 10.0, 0.05, 0.1);
        assert!(sir.r0() < 1.0, "R0 should be < 1");
        for _ in 0..10000 { sir.step(0.1); }
        assert!(sir.recovered < 50.0, "disease should die out quickly");
    }

    #[test]
    fn test_sis_endemic() {
        let mut sis = SisState::new(1000.0, 10.0, 0.3, 0.1);
        for _ in 0..10000 { sis.step(0.1); }
        assert!(sis.infected > 10.0, "SIS should reach endemic equilibrium");
    }
}
