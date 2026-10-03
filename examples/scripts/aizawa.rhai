// The Aizawa attractor: a sphere-like shell pierced by a tube.
let a = 0.95;
let b = 0.7;
let c = 0.6;
let d = 3.5;
let e = 0.25;
let f = 0.1;

[
    (z - b) * x - d * y,
    d * x + (z - b) * y,
    c + a * z - z * z * z / 3.0 - (x * x + y * y) * (1.0 + e * z) + f * z * x * x * x,
]
