// The Lorenz system (1963): a toy model of convection, and the first
// strange attractor. Edit sigma, rho or beta and save to see it change.
let sigma = 10.0;
let rho = 28.0;
let beta = 8.0 / 3.0;

[sigma * (y - x), x * (rho - z) - y, x * y - beta * z]
