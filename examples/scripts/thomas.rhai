// Thomas' cyclically symmetric attractor (1999). b is the friction:
// around 0.208 it is chaotic, above about 0.33 it settles to a point.
let b = 0.208186;

[sin(y) - b * x, sin(z) - b * y, sin(x) - b * z]
