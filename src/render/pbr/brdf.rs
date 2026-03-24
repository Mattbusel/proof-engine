//! Complete BRDF (Bidirectional Reflectance Distribution Function) library.
//!
//! Pure CPU math — no GPU types.  All functions use `glam::{Vec2, Vec3}` and
//! bare `f32`.  Organised into:
//!
//! * [`distribution`] — microfacet normal-distribution functions (D)
//! * [`geometry`]     — masking/shadowing functions (G)
//! * [`fresnel`]      — Fresnel reflectance functions (F)
//! * [`vec3`]         — Vec3 helpers used throughout
//! * [`brdf`]         — full BRDF evaluators (Cook-Torrance, Lambertian, …)
//! * [`ibl`]          — image-based lighting helpers
//! * [`lights`]       — light types and shading loop
//! * [`tonemap`]      — tone-mapping operators
//! * [`glsl`]         — GLSL source generation

use glam::{Vec2, Vec3};
use std::f32::consts::{FRAC_1_PI, PI};

// ─────────────────────────────────────────────────────────────────────────────
// Vec3 helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Lightweight Vec3 helper functions used throughout the BRDF library.
pub mod vec3 {
    use glam::Vec3;

    /// Dot product, clamped to `[0, 1]`.
    #[inline]
    pub fn dot_clamp(a: Vec3, b: Vec3) -> f32 {
        a.dot(b).clamp(0.0, 1.0)
    }

    /// Dot product, clamped to `[0 + epsilon, 1]` to avoid division by zero.
    #[inline]
    pub fn dot_clamp_eps(a: Vec3, b: Vec3) -> f32 {
        a.dot(b).clamp(1e-5, 1.0)
    }

    /// Reflect `v` around `n`.
    #[inline]
    pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
        v - 2.0 * v.dot(n) * n
    }

    /// Refract `v` through a surface with relative IOR `eta`.
    /// Returns `None` on total internal reflection.
    pub fn refract(v: Vec3, n: Vec3, eta: f32) -> Option<Vec3> {
        let cos_i = (-v).dot(n);
        let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
        if sin2_t >= 1.0 {
            return None; // TIR
        }
        let cos_t = (1.0 - sin2_t).sqrt();
        Some(eta * v + (eta * cos_i - cos_t) * n)
    }

    /// Schlick Fresnel approximation (scalar version).
    #[inline]
    pub fn schlick_scalar(cos_theta: f32, f0: f32) -> f32 {
        f0 + (1.0 - f0) * (1.0 - cos_theta).powi(5)
    }

    /// Construct a local orthonormal basis (tangent `t`, bitangent `b`) from
    /// surface normal `n` using the Duff et al. 2017 method.
    pub fn orthonormal_basis(n: Vec3) -> (Vec3, Vec3) {
        // Duff, Tom et al., "Building an Orthonormal Basis, Revisited", JCGT 2017
        let sign = if n.z >= 0.0 { 1.0_f32 } else { -1.0_f32 };
        let a = -1.0 / (sign + n.z);
        let b = n.x * n.y * a;
        let t = Vec3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x);
        let bi = Vec3::new(b, sign + n.y * n.y * a, -n.y);
        (t, bi)
    }

    /// Spherical direction to cartesian.
    #[inline]
    pub fn spherical_to_cartesian(sin_theta: f32, cos_theta: f32, phi: f32) -> Vec3 {
        Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta)
    }

    /// Cartesian to spherical (theta, phi).
    #[inline]
    pub fn cartesian_to_spherical(v: Vec3) -> (f32, f32) {
        let theta = v.z.clamp(-1.0, 1.0).acos();
        let phi = v.y.atan2(v.x);
        (theta, phi)
    }

    /// Saturate (clamp to [0, 1]).
    #[inline]
    pub fn saturate(v: Vec3) -> Vec3 {
        v.clamp(Vec3::ZERO, Vec3::ONE)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Distribution functions (D)
// ─────────────────────────────────────────────────────────────────────────────

/// Microfacet normal-distribution functions.
///
/// All functions return D(h) — the density of microfacets with half-vector h.
pub mod distribution {
    use std::f32::consts::PI;

    /// GGX / Trowbridge-Reitz NDF.
    ///
    /// `n_dot_h` — cos angle between normal and half-vector (clamped to ≥ 0).
    /// `roughness` — perceptual roughness (will be squared to get α²).
    pub fn ggx_d(n_dot_h: f32, roughness: f32) -> f32 {
        let alpha = roughness * roughness;
        let alpha2 = alpha * alpha;
        let n_dot_h = n_dot_h.max(1e-5);
        let d = (n_dot_h * n_dot_h) * (alpha2 - 1.0) + 1.0;
        alpha2 / (PI * d * d)
    }

    /// Beckmann NDF — older model, matches Blinn-Phong at certain roughness.
    pub fn beckmann_d(n_dot_h: f32, roughness: f32) -> f32 {
        let alpha = roughness * roughness;
        let alpha2 = alpha * alpha;
        let n_dot_h = n_dot_h.max(1e-5);
        let cos2 = n_dot_h * n_dot_h;
        let tan2 = (1.0 - cos2) / (cos2 + 1e-10);
        (-tan2 / alpha2).exp() / (PI * alpha2 * cos2 * cos2)
    }

    /// Blinn-Phong NDF parameterised by shininess.
    pub fn blinn_phong_d(n_dot_h: f32, shininess: f32) -> f32 {
        let n_dot_h = n_dot_h.max(0.0);
        (shininess + 2.0) / (2.0 * PI) * n_dot_h.powf(shininess)
    }

    /// Anisotropic GGX NDF.
    ///
    /// * `h_dot_x` — dot of half-vector with tangent direction
    /// * `h_dot_y` — dot of half-vector with bitangent direction
    /// * `ax`, `ay` — roughness along x and y tangent axes
    pub fn anisotropic_ggx_d(
        n_dot_h: f32,
        h_dot_x: f32,
        h_dot_y: f32,
        ax: f32,
        ay: f32,
    ) -> f32 {
        let ax = ax.max(1e-4);
        let ay = ay.max(1e-4);
        let hx = h_dot_x / ax;
        let hy = h_dot_y / ay;
        let n = n_dot_h.max(1e-5);
        let denom = hx * hx + hy * hy + n * n;
        1.0 / (PI * ax * ay * denom * denom)
    }

    /// Phong NDF — converts Phong shininess to roughness-compatible form.
    pub fn phong_d(n_dot_h: f32, roughness: f32) -> f32 {
        let shininess = 2.0 / (roughness * roughness).max(1e-5) - 2.0;
        blinn_phong_d(n_dot_h, shininess.max(0.0))
    }

    /// Ward anisotropic NDF.
    pub fn ward_d(
        n_dot_h: f32,
        h_dot_x: f32,
        h_dot_y: f32,
        ax: f32,
        ay: f32,
        n_dot_l: f32,
        n_dot_v: f32,
    ) -> f32 {
        let ax = ax.max(1e-4);
        let ay = ay.max(1e-4);
        let n = n_dot_h.max(1e-5);
        let nl = n_dot_l.max(1e-5);
        let nv = n_dot_v.max(1e-5);
        let exponent = -((h_dot_x / ax).powi(2) + (h_dot_y / ay).powi(2)) / (n * n);
        exponent.exp() / (4.0 * std::f32::consts::PI * ax * ay * (nl * nv).sqrt())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Geometry / masking functions (G)
// ─────────────────────────────────────────────────────────────────────────────

/// Microfacet masking-shadowing functions.
pub mod geometry {
    /// Schlick approximation of the GGX G1 term.
    ///
    /// `k` should be `roughness² / 2` for direct lighting, or
    /// `(roughness + 1)² / 8` for IBL.
    #[inline]
    pub fn schlick_ggx_g1(n_dot_v: f32, k: f32) -> f32 {
        let n_dot_v = n_dot_v.max(0.0);
        n_dot_v / (n_dot_v * (1.0 - k) + k + 1e-7)
    }

    /// Smith GGX masking-shadowing — product of two G1 terms (view + light).
    pub fn smith_ggx(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
        let k = (roughness + 1.0).powi(2) / 8.0;
        schlick_ggx_g1(n_dot_v, k) * schlick_ggx_g1(n_dot_l, k)
    }

    /// Smith GGX — IBL variant uses `k = α² / 2`.
    pub fn smith_ggx_ibl(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
        let k = (roughness * roughness) / 2.0;
        schlick_ggx_g1(n_dot_v, k) * schlick_ggx_g1(n_dot_l, k)
    }

    /// Smith geometry term using the Beckmann distribution.
    pub fn smith_beckmann(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
        let alpha = roughness * roughness;

        fn g1(n_dot: f32, alpha: f32) -> f32 {
            let c = n_dot / (alpha * (1.0 - n_dot * n_dot).max(0.0).sqrt());
            if c >= 1.6 {
                1.0
            } else {
                (3.535 * c + 2.181 * c * c) / (1.0 + 2.276 * c + 2.577 * c * c)
            }
        }

        g1(n_dot_v.max(1e-5), alpha) * g1(n_dot_l.max(1e-5), alpha)
    }

    /// Kelemen-Szirmay-Kalos simplified G term.
    pub fn kelemen_szirmay_kalos_g(n_dot_v: f32, n_dot_l: f32) -> f32 {
        let v = n_dot_v.max(1e-5);
        let l = n_dot_l.max(1e-5);
        (v * l) / (v + l - v * l)
    }

    /// Implicit (uncorrelated) geometry term — `G = NdotV * NdotL`.
    #[inline]
    pub fn implicit_g(n_dot_v: f32, n_dot_l: f32) -> f32 {
        n_dot_v.max(0.0) * n_dot_l.max(0.0)
    }

    /// Neumann geometry term.
    pub fn neumann_g(n_dot_v: f32, n_dot_l: f32) -> f32 {
        let v = n_dot_v.max(1e-5);
        let l = n_dot_l.max(1e-5);
        (v * l) / v.max(l)
    }

    /// Cook-Torrance masking term.
    pub fn cook_torrance_g(n_dot_v: f32, n_dot_l: f32, n_dot_h: f32, v_dot_h: f32) -> f32 {
        let v = n_dot_v.max(1e-5);
        let l = n_dot_l.max(1e-5);
        let h = n_dot_h.max(1e-5);
        let vdh = v_dot_h.max(1e-5);
        let t1 = 2.0 * h * v / vdh;
        let t2 = 2.0 * h * l / vdh;
        t1.min(t2).min(1.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fresnel functions (F)
// ─────────────────────────────────────────────────────────────────────────────

/// Fresnel reflectance functions.
pub mod fresnel {
    use glam::Vec3;

    /// Schlick Fresnel approximation for a coloured F0.
    #[inline]
    pub fn schlick_f(cos_theta: f32, f0: Vec3) -> Vec3 {
        let t = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
        f0 + (Vec3::ONE - f0) * t
    }

    /// Schlick Fresnel with roughness-modulated F0 — used in IBL.
    pub fn schlick_roughness_f(cos_theta: f32, f0: Vec3, roughness: f32) -> Vec3 {
        let t = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
        let max_comp = (1.0 - roughness).max(f0.x).max(f0.y).max(f0.z);
        f0 + (Vec3::splat(max_comp) - f0) * t
    }

    /// Exact Fresnel for a conductor (metal) with complex IOR `n + ik`.
    ///
    /// Returns reflectance for a single wavelength (use per-channel for RGB).
    pub fn conductor_fresnel(cos_theta: f32, ior: f32, k: f32) -> f32 {
        let ct = cos_theta.clamp(0.0, 1.0);
        let ct2 = ct * ct;
        let n2 = ior * ior;
        let k2 = k * k;
        let n2k2 = n2 + k2;

        let rs_num = n2k2 - 2.0 * ior * ct + ct2;
        let rs_den = n2k2 + 2.0 * ior * ct + ct2;
        let rs = rs_num / rs_den.max(1e-10);

        let rp_num = n2k2 * ct2 - 2.0 * ior * ct + 1.0;
        let rp_den = n2k2 * ct2 + 2.0 * ior * ct + 1.0;
        let rp = rp_num / rp_den.max(1e-10);

        (rs + rp) * 0.5
    }

    /// Fresnel for a dielectric (non-absorbing), using Snell's law + exact formula.
    pub fn dielectric_fresnel(cos_theta_i: f32, ior: f32) -> f32 {
        let ct_i = cos_theta_i.clamp(0.0, 1.0);
        let sin2_t = (1.0 - ct_i * ct_i) / (ior * ior);
        if sin2_t >= 1.0 {
            return 1.0; // TIR
        }
        let ct_t = (1.0 - sin2_t).sqrt();

        let rs = (ct_i - ior * ct_t) / (ct_i + ior * ct_t);
        let rp = (ior * ct_i - ct_t) / (ior * ct_i + ct_t);
        (rs * rs + rp * rp) * 0.5
    }

    /// Compute scalar F0 from relative index of refraction.
    #[inline]
    pub fn f0_from_ior(ior: f32) -> f32 {
        let t = (ior - 1.0) / (ior + 1.0);
        t * t
    }

    /// Compute F0 as a Vec3 (achromatic) from IOR.
    #[inline]
    pub fn f0_vec3_from_ior(ior: f32) -> Vec3 {
        Vec3::splat(f0_from_ior(ior))
    }

    /// Schlick Fresnel for a dielectric (scalar F0).
    #[inline]
    pub fn schlick_scalar_f(cos_theta: f32, f0: f32) -> f32 {
        f0 + (1.0 - f0) * (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Minimal PRNG for sampling
// ─────────────────────────────────────────────────────────────────────────────

/// Simple xorshift32 PRNG used for Monte Carlo BRDF sampling.
/// Not suitable for cryptography — only for rendering randomness.
pub struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Generate the next pseudo-random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Generate a uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32 + 1.0)
    }

    /// Generate two independent uniform floats in `[0, 1)`.
    pub fn next_vec2(&mut self) -> Vec2 {
        Vec2::new(self.next_f32(), self.next_f32())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Full BRDF evaluators
// ─────────────────────────────────────────────────────────────────────────────

/// Cook-Torrance specular BRDF combined with Lambertian diffuse.
///
/// Uses GGX NDF, Smith-GGX geometry, and Schlick Fresnel.
pub struct CookTorranceBrdf;

impl CookTorranceBrdf {
    /// Evaluate the full PBR BRDF (diffuse + specular) for one light direction.
    ///
    /// * `n` — surface normal (unit)
    /// * `v` — view direction pointing *away* from surface (unit)
    /// * `l` — light direction pointing *toward* the light (unit)
    /// * `albedo` — base colour (linear RGB)
    /// * `metallic` — metallic factor [0,1]
    /// * `roughness` — perceptual roughness [0,1]
    ///
    /// Returns the BRDF value multiplied by `NdotL`.  The caller multiplies by
    /// light irradiance.
    pub fn evaluate(
        n: Vec3,
        v: Vec3,
        l: Vec3,
        albedo: Vec3,
        metallic: f32,
        roughness: f32,
    ) -> Vec3 {
        let n_dot_l = n.dot(l).max(0.0);
        let n_dot_v = n.dot(v).max(1e-5);
        if n_dot_l < 1e-5 {
            return Vec3::ZERO;
        }

        let h = (v + l).normalize();
        let n_dot_h = n.dot(h).max(0.0);
        let v_dot_h = v.dot(h).clamp(0.0, 1.0);

        // Roughness remapping to avoid zero
        let rough = roughness.max(0.04);

        // F0 for dielectric/metal blend
        let f0_dielectric = fresnel::f0_from_ior(1.5);
        let f0 = Vec3::splat(f0_dielectric).lerp(albedo, metallic);

        // D — microfacet NDF
        let d = distribution::ggx_d(n_dot_h, rough);

        // G — masking-shadowing
        let g = geometry::smith_ggx(n_dot_v, n_dot_l, rough);

        // F — Fresnel
        let f = fresnel::schlick_f(v_dot_h, f0);

        // Cook-Torrance specular BRDF
        let denom = (4.0 * n_dot_v * n_dot_l).max(1e-7);
        let specular = d * g * f / denom;

        // Lambertian diffuse — metals have no diffuse
        let k_s = f;
        let k_d = (Vec3::ONE - k_s) * (1.0 - metallic);
        let diffuse = k_d * albedo * FRAC_1_PI;

        (diffuse + specular) * n_dot_l
    }

    /// Importance-sample the GGX distribution to generate a reflected direction.
    ///
    /// Returns `(reflected_direction, pdf)`.
    pub fn sample(n: Vec3, v: Vec3, roughness: f32, rng: &mut SimpleRng) -> (Vec3, f32) {
        let (xi1, xi2) = (rng.next_f32(), rng.next_f32());

        let alpha = roughness * roughness;
        let alpha2 = alpha * alpha;

        // Sample half-vector in tangent space (GGX distribution)
        let cos_theta = ((1.0 - xi1) / (xi1 * (alpha2 - 1.0) + 1.0)).sqrt();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let phi = 2.0 * PI * xi2;

        let h_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);

        // Transform to world space
        let (t, b) = vec3::orthonormal_basis(n);
        let h = (t * h_local.x + b * h_local.y + n * h_local.z).normalize();

        // Reflect view around half-vector
        let l = vec3::reflect(-v, h);
        if l.dot(n) < 0.0 {
            // Sample below surface — return a safe fallback
            return (n, 1.0);
        }

        // PDF of this sample
        let n_dot_h = n.dot(h).max(1e-5);
        let d = distribution::ggx_d(n_dot_h, roughness);
        let pdf = (d * n_dot_h / (4.0 * v.dot(h).max(1e-5))).max(1e-7);

        (l, pdf)
    }

    /// Evaluate the specular-only portion of the BRDF (for clearcoat layers etc.)
    pub fn evaluate_specular(n: Vec3, v: Vec3, l: Vec3, roughness: f32, f0: Vec3) -> Vec3 {
        let n_dot_l = n.dot(l).max(0.0);
        let n_dot_v = n.dot(v).max(1e-5);
        if n_dot_l < 1e-5 {
            return Vec3::ZERO;
        }

        let h = (v + l).normalize();
        let n_dot_h = n.dot(h).max(0.0);
        let v_dot_h = v.dot(h).clamp(0.0, 1.0);
        let rough = roughness.max(0.04);

        let d = distribution::ggx_d(n_dot_h, rough);
        let g = geometry::smith_ggx(n_dot_v, n_dot_l, rough);
        let f = fresnel::schlick_f(v_dot_h, f0);

        d * g * f / (4.0 * n_dot_v * n_dot_l).max(1e-7) * n_dot_l
    }

    /// Multi-scatter energy compensation (Karis 2018).
    /// Corrects energy loss from single-scattering model.
    pub fn multi_scatter_compensation(
        n_dot_v: f32,
        roughness: f32,
        f0: Vec3,
        brdf_lut: &ibl::BrdfLut,
    ) -> Vec3 {
        let lut = brdf_lut.integrate(n_dot_v, roughness);
        let scale = lut.x;
        let bias = lut.y;

        let e_single = f0 * scale + Vec3::splat(bias);
        let e_multi = Vec3::ONE - e_single;
        // Approximate multi-scatter colour as albedo-tinted white
        let f_avg = f0 * (1.0 / 21.0) + Vec3::splat(20.0 / 21.0) * f0;
        Vec3::ONE + f_avg * e_multi / (Vec3::ONE - f_avg * e_multi + Vec3::splat(1e-7))
    }
}

/// Lambertian (perfectly diffuse) BRDF.
pub struct LambertianBrdf;

impl LambertianBrdf {
    /// Evaluate the Lambertian BRDF — returns `albedo / π * NdotL`.
    #[inline]
    pub fn evaluate(albedo: Vec3, n_dot_l: f32) -> Vec3 {
        albedo * FRAC_1_PI * n_dot_l.max(0.0)
    }

    /// Generate a cosine-weighted sample in the hemisphere around `n`.
    pub fn sample(n: Vec3, rng: &mut SimpleRng) -> (Vec3, f32) {
        let xi1 = rng.next_f32();
        let xi2 = rng.next_f32();

        // Cosine-weighted hemisphere sample
        let cos_theta = xi1.sqrt();
        let sin_theta = (1.0 - xi1).sqrt();
        let phi = 2.0 * PI * xi2;

        let l_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
        let (t, b) = vec3::orthonormal_basis(n);
        let l = (t * l_local.x + b * l_local.y + n * l_local.z).normalize();
        let pdf = cos_theta * FRAC_1_PI;
        (l, pdf.max(1e-7))
    }
}

/// Oren-Nayar BRDF for rough diffuse surfaces.
pub struct OrenNayarBrdf;

impl OrenNayarBrdf {
    /// Evaluate the Oren-Nayar diffuse BRDF.
    ///
    /// `roughness` — surface roughness (0 = perfectly smooth Lambertian, 1 = very rough).
    pub fn evaluate(v: Vec3, l: Vec3, n: Vec3, albedo: Vec3, roughness: f32) -> Vec3 {
        let sigma2 = roughness * roughness;
        let a = 1.0 - 0.5 * sigma2 / (sigma2 + 0.33);
        let b = 0.45 * sigma2 / (sigma2 + 0.09);

        let n_dot_v = n.dot(v).max(0.0);
        let n_dot_l = n.dot(l).max(0.0);

        // Project v and l onto the plane perpendicular to n
        let v_perp = (v - n * n_dot_v).normalize();
        let l_perp = (l - n * n_dot_l).normalize();
        let cos_phi = v_perp.dot(l_perp).max(0.0);

        let theta_i = n_dot_l.acos();
        let theta_r = n_dot_v.acos();
        let alpha = theta_i.max(theta_r);
        let beta = theta_i.min(theta_r);

        albedo * FRAC_1_PI * (a + b * cos_phi * alpha.sin() * (beta.tan() + 1e-5)) * n_dot_l
    }
}

/// Clearcoat BRDF layer — a thin dielectric coat on top of the base material.
pub struct ClearcoatBrdf;

impl ClearcoatBrdf {
    /// Evaluate the clearcoat specular lobe.
    ///
    /// `strength` — clearcoat weight [0,1]
    /// `roughness` — clearcoat roughness [0,1]
    pub fn evaluate(n: Vec3, v: Vec3, l: Vec3, strength: f32, roughness: f32) -> Vec3 {
        if strength < 1e-5 {
            return Vec3::ZERO;
        }
        // Clearcoat uses a fixed IOR of 1.5 (automotive clear coat)
        let f0 = Vec3::splat(fresnel::f0_from_ior(1.5));
        let spec = CookTorranceBrdf::evaluate_specular(n, v, l, roughness, f0);
        spec * strength
    }

    /// Clearcoat Fresnel attenuation factor — multiply the base layer by this.
    pub fn attenuation(n: Vec3, v: Vec3, strength: f32) -> Vec3 {
        if strength < 1e-5 {
            return Vec3::ONE;
        }
        let n_dot_v = n.dot(v).clamp(0.0, 1.0);
        let f = fresnel::schlick_scalar_f(n_dot_v, fresnel::f0_from_ior(1.5));
        Vec3::ONE - Vec3::splat(strength * f)
    }
}

/// Anisotropic GGX BRDF.
pub struct AnisotropicBrdf;

impl AnisotropicBrdf {
    /// Evaluate anisotropic specular BRDF.
    ///
    /// * `t`, `b` — tangent and bitangent vectors (unit)
    /// * `roughness_x`, `roughness_y` — roughness along t and b axes
    /// * `f0` — specular reflectance at normal incidence
    pub fn evaluate(
        n: Vec3,
        v: Vec3,
        l: Vec3,
        t: Vec3,
        b: Vec3,
        metallic: f32,
        roughness_x: f32,
        roughness_y: f32,
        f0: Vec3,
    ) -> Vec3 {
        let n_dot_l = n.dot(l).max(0.0);
        let n_dot_v = n.dot(v).max(1e-5);
        if n_dot_l < 1e-5 {
            return Vec3::ZERO;
        }

        let h = (v + l).normalize();
        let n_dot_h = n.dot(h).max(0.0);
        let v_dot_h = v.dot(h).clamp(0.0, 1.0);
        let h_dot_t = h.dot(t);
        let h_dot_b = h.dot(b);

        let ax = roughness_x.max(0.04);
        let ay = roughness_y.max(0.04);

        let d = distribution::anisotropic_ggx_d(n_dot_h, h_dot_t, h_dot_b, ax, ay);

        // Anisotropic Smith G using combined roughness
        let rough_avg = (ax * ay).sqrt();
        let g = geometry::smith_ggx(n_dot_v, n_dot_l, rough_avg);

        let f = fresnel::schlick_f(v_dot_h, f0);

        let specular = d * g * f / (4.0 * n_dot_v * n_dot_l).max(1e-7);

        // Diffuse term (metals have no diffuse)
        let k_d = (Vec3::ONE - f) * (1.0 - metallic);
        let diffuse = k_d * FRAC_1_PI;

        (diffuse + specular) * n_dot_l
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Image-Based Lighting
// ─────────────────────────────────────────────────────────────────────────────

/// Image-based lighting helpers.
pub mod ibl {
    use super::{distribution, fresnel, geometry, SimpleRng, vec3, PI, FRAC_1_PI};
    use glam::{Vec2, Vec3};

    /// Analytic approximation of a pre-filtered environment map.
    ///
    /// In a real renderer this would be a cubemap convolved at multiple mip levels.
    /// Here we provide a cheap analytical stand-in that can be used in tests and
    /// offline tools.
    pub struct PrefilterEnv {
        /// The "sky colour" used for background environment.
        pub sky_color: Vec3,
        /// Sun disc colour (bright spot in the upper hemisphere).
        pub sun_color: Vec3,
        /// Sun direction (unit).
        pub sun_dir: Vec3,
        /// Sun angular radius in radians.
        pub sun_radius: f32,
    }

    impl PrefilterEnv {
        pub fn new(sky_color: Vec3, sun_color: Vec3, sun_dir: Vec3, sun_radius: f32) -> Self {
            Self {
                sky_color,
                sun_color,
                sun_dir: sun_dir.normalize(),
                sun_radius,
            }
        }

        /// Sample the environment in direction `dir` at the given roughness LOD.
        ///
        /// The analytic model blurs the sun disc as roughness increases and
        /// lerps toward the average sky colour at maximum roughness.
        pub fn sample_lod(&self, dir: Vec3, roughness: f32) -> Vec3 {
            let dir = dir.normalize();
            let cos_sun = dir.dot(self.sun_dir).max(0.0);
            // Angular size of sun after blurring by roughness
            let blur_radius = self.sun_radius + roughness * (PI / 2.0);
            let sun_falloff = ((cos_sun - (1.0 - blur_radius * blur_radius * 0.5))
                / (blur_radius * blur_radius * 0.5 + 1e-5))
                .clamp(0.0, 1.0);

            // Height-based sky gradient
            let sky_lerp = (dir.y * 0.5 + 0.5).clamp(0.0, 1.0);
            let ambient = self.sky_color * sky_lerp + Vec3::new(0.1, 0.1, 0.15) * (1.0 - sky_lerp);

            // Mix in sun contribution
            ambient.lerp(self.sun_color, sun_falloff * (1.0 - roughness))
        }

        /// Diffuse irradiance — sample as if roughness = 1.
        pub fn sample_irradiance(&self, normal: Vec3) -> Vec3 {
            self.sample_lod(normal, 1.0)
        }
    }

    impl Default for PrefilterEnv {
        fn default() -> Self {
            Self::new(
                Vec3::new(0.5, 0.7, 1.0),
                Vec3::new(10.0, 9.0, 7.0),
                Vec3::new(0.3, 0.8, 0.5).normalize(),
                0.01,
            )
        }
    }

    /// BRDF integration lookup table.
    ///
    /// Stores pre-computed `(a, b)` scale and bias for the Schlick-GGX BRDF
    /// integration over the upper hemisphere.  The result approximates:
    ///
    ///   ∫ f_spec(v, l) NdotL dl ≈ F0 * a + b
    pub struct BrdfLut {
        pub size: usize,
        /// Row-major table: `data[row * size + col]` where
        /// row = roughness bin, col = NdotV bin.
        pub data: Vec<Vec2>,
    }

    impl BrdfLut {
        /// Generate the LUT offline by Monte Carlo integration.
        ///
        /// `size` — number of bins along each axis (e.g. 128 or 256).
        pub fn generate(size: usize) -> Self {
            let mut data = vec![Vec2::ZERO; size * size];
            let n_samples = 1024usize;

            for row in 0..size {
                let roughness = (row as f32 + 0.5) / size as f32;
                for col in 0..size {
                    let n_dot_v = (col as f32 + 0.5) / size as f32;
                    data[row * size + col] = integrate_brdf(n_dot_v, roughness, n_samples);
                }
            }

            BrdfLut { size, data }
        }

        /// Bilinearly sample the LUT.
        pub fn integrate(&self, n_dot_v: f32, roughness: f32) -> Vec2 {
            let n_dot_v = n_dot_v.clamp(0.0, 1.0);
            let roughness = roughness.clamp(0.0, 1.0);

            let col_f = n_dot_v * (self.size as f32 - 1.0);
            let row_f = roughness * (self.size as f32 - 1.0);

            let col0 = (col_f as usize).min(self.size - 1);
            let row0 = (row_f as usize).min(self.size - 1);
            let col1 = (col0 + 1).min(self.size - 1);
            let row1 = (row0 + 1).min(self.size - 1);

            let tc = col_f - col0 as f32;
            let tr = row_f - row0 as f32;

            let s00 = self.data[row0 * self.size + col0];
            let s10 = self.data[row0 * self.size + col1];
            let s01 = self.data[row1 * self.size + col0];
            let s11 = self.data[row1 * self.size + col1];

            let s0 = s00.lerp(s10, tc);
            let s1 = s01.lerp(s11, tc);
            s0.lerp(s1, tr)
        }

        /// Return a flat `Vec<Vec2>` copy of the table data.
        pub fn generate_lut(size: usize) -> Vec<Vec2> {
            Self::generate(size).data
        }
    }

    impl Default for BrdfLut {
        fn default() -> Self {
            Self::generate(64)
        }
    }

    /// Monte Carlo integrate the specular BRDF for `(NdotV, roughness)`.
    fn integrate_brdf(n_dot_v: f32, roughness: f32, n_samples: usize) -> Vec2 {
        // Build a local V in tangent space from NdotV
        let v = Vec3::new(
            (1.0 - n_dot_v * n_dot_v).max(0.0).sqrt(),
            0.0,
            n_dot_v,
        );
        let n = Vec3::Z;

        let mut sum = Vec2::ZERO;
        let mut rng = SimpleRng::new(0x12345678);

        for _ in 0..n_samples {
            let xi1 = rng.next_f32();
            let xi2 = rng.next_f32();

            // Importance-sample GGX half-vector
            let alpha = roughness * roughness;
            let alpha2 = alpha * alpha;
            let cos_theta_h = ((1.0 - xi1) / (xi1 * (alpha2 - 1.0) + 1.0)).sqrt();
            let sin_theta_h = (1.0 - cos_theta_h * cos_theta_h).max(0.0).sqrt();
            let phi = 2.0 * PI * xi2;

            let h = Vec3::new(sin_theta_h * phi.cos(), sin_theta_h * phi.sin(), cos_theta_h);
            let l = (2.0 * v.dot(h) * h - v).normalize();

            if l.z > 0.0 {
                let n_dot_l = l.z.max(0.0);
                let n_dot_h = h.z.max(0.0);
                let v_dot_h = v.dot(h).max(0.0);

                let g = geometry::smith_ggx_ibl(n_dot_v, n_dot_l, roughness);
                let g_vis = g * v_dot_h / (n_dot_h * n_dot_v).max(1e-7);
                let fc = (1.0 - v_dot_h).powi(5);

                sum.x += (1.0 - fc) * g_vis;
                sum.y += fc * g_vis;
            }
        }

        sum / n_samples as f32
    }

    /// Ambient occlusion helpers.
    pub struct AmbientOcclusion;

    impl AmbientOcclusion {
        /// Generate a hemisphere sample kernel for SSAO.
        ///
        /// `n_samples` — number of kernel taps (typically 16–64).
        pub fn ssao_kernel(n_samples: usize) -> Vec<Vec3> {
            let mut rng = SimpleRng::new(0xDEADBEEF);
            let mut kernel = Vec::with_capacity(n_samples);

            for i in 0..n_samples {
                // Cosine-weighted hemisphere sample
                let xi1 = rng.next_f32();
                let xi2 = rng.next_f32();

                let sin_theta = xi1.sqrt();
                let cos_theta = (1.0 - xi1).sqrt();
                let phi = 2.0 * PI * xi2;

                let sample = Vec3::new(
                    sin_theta * phi.cos(),
                    sin_theta * phi.sin(),
                    cos_theta,
                );

                // Accelerating interpolation towards the origin
                let scale = i as f32 / n_samples as f32;
                let scale = 0.1_f32 + (1.0_f32 - 0.1_f32) * (scale * scale);

                kernel.push(sample * scale);
            }

            kernel
        }

        /// Compute a bent normal and AO factor from a sample set.
        ///
        /// `samples` — world-space sample offsets (from `ssao_kernel`)
        /// `depth`   — per-sample visibility (1.0 = unoccluded, 0.0 = occluded)
        /// `n`       — surface normal
        ///
        /// Returns `(bent_normal, ao)`.
        pub fn bent_normal_ao(samples: &[Vec3], depth: &[f32], n: Vec3) -> (Vec3, f32) {
            assert_eq!(
                samples.len(),
                depth.len(),
                "samples and depth must have equal length"
            );

            let mut bent = Vec3::ZERO;
            let mut unoccluded = 0u32;
            let total = samples.len() as u32;

            for (s, &vis) in samples.iter().zip(depth.iter()) {
                if s.dot(n) > 0.0 {
                    // Only count hemispherical samples
                    if vis > 0.5 {
                        bent += *s;
                        unoccluded += 1;
                    }
                }
            }

            let ao = unoccluded as f32 / total.max(1) as f32;
            let bent_normal = if bent.length_squared() > 1e-10 {
                bent.normalize()
            } else {
                n
            };

            (bent_normal, ao)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Light models
// ─────────────────────────────────────────────────────────────────────────────

/// Directional (sun-like) light — no position, infinite distance.
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    /// Unit direction from surface toward the light.
    pub direction: Vec3,
    /// Linear RGB colour of the light.
    pub color: Vec3,
    /// Luminous intensity multiplier.
    pub intensity: f32,
}

impl DirectionalLight {
    pub fn new(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            direction: direction.normalize(),
            color,
            intensity,
        }
    }

    /// Irradiance contribution at a surface (before BRDF).
    pub fn irradiance(&self) -> Vec3 {
        self.color * self.intensity
    }
}

/// Omnidirectional point light.
#[derive(Debug, Clone)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    /// Physical radius for energy-conserving area-light attenuation.
    pub radius: f32,
}

impl PointLight {
    pub fn new(position: Vec3, color: Vec3, intensity: f32, radius: f32) -> Self {
        Self {
            position,
            color,
            intensity,
            radius: radius.max(0.001),
        }
    }

    /// Inverse-square-law attenuation with smooth window function (Karis 2013).
    pub fn attenuation(&self, surface_pos: Vec3) -> f32 {
        let dist = (self.position - surface_pos).length();
        let r = self.radius;
        // Window function: (1 - (d/r)^4)^2 / (d^2 + 0.01)
        let x = dist / r;
        let window = (1.0 - x * x * x * x).max(0.0);
        window * window / (dist * dist + 0.01)
    }

    /// Direction from surface toward this light.
    pub fn direction_to(&self, surface_pos: Vec3) -> Vec3 {
        (self.position - surface_pos).normalize()
    }

    /// Irradiance at surface position.
    pub fn irradiance_at(&self, surface_pos: Vec3) -> Vec3 {
        self.color * self.intensity * self.attenuation(surface_pos)
    }
}

/// Spot light — cone-shaped.
#[derive(Debug, Clone)]
pub struct SpotLight {
    pub position: Vec3,
    /// Unit direction the spot points toward.
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    /// Inner cone half-angle (full bright region), in radians.
    pub inner_angle: f32,
    /// Outer cone half-angle (falloff to zero), in radians.
    pub outer_angle: f32,
}

impl SpotLight {
    pub fn new(
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color,
            intensity,
            inner_angle,
            outer_angle,
        }
    }

    /// Angular attenuation — smooth falloff from inner to outer cone.
    pub fn angular_attenuation(&self, surface_pos: Vec3) -> f32 {
        let l = (surface_pos - self.position).normalize();
        let cos_theta = l.dot(self.direction);
        let cos_inner = self.inner_angle.cos();
        let cos_outer = self.outer_angle.cos();
        let t = ((cos_theta - cos_outer) / (cos_inner - cos_outer + 1e-5)).clamp(0.0, 1.0);
        t * t // smooth step approximation
    }

    /// Radial attenuation (same as point light).
    pub fn radial_attenuation(&self, surface_pos: Vec3) -> f32 {
        let dist = (self.position - surface_pos).length();
        let falloff_radius = 10.0 * self.outer_angle; // heuristic
        let x = dist / falloff_radius;
        (1.0 - x * x * x * x).max(0.0).powi(2) / (dist * dist + 0.01)
    }

    /// Combined irradiance at surface position.
    pub fn irradiance_at(&self, surface_pos: Vec3) -> Vec3 {
        let angular = self.angular_attenuation(surface_pos);
        let radial = self.radial_attenuation(surface_pos);
        self.color * self.intensity * angular * radial
    }

    /// Direction from surface toward the light source.
    pub fn direction_to(&self, surface_pos: Vec3) -> Vec3 {
        (self.position - surface_pos).normalize()
    }
}

/// Rectangular area light using the Linearly Transformed Cosines (LTC) approach.
#[derive(Debug, Clone)]
pub struct AreaLight {
    /// Centre of the rectangle.
    pub position: Vec3,
    /// Half-extent along the "right" axis (width/2).
    pub right: Vec3,
    /// Half-extent along the "up" axis (height/2).
    pub up: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}

impl AreaLight {
    pub fn new(position: Vec3, right: Vec3, up: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            position,
            right,
            up,
            color,
            intensity,
        }
    }

    /// Compute the four corner positions of the rectangle.
    pub fn corners(&self) -> [Vec3; 4] {
        [
            self.position - self.right - self.up,
            self.position + self.right - self.up,
            self.position + self.right + self.up,
            self.position - self.right + self.up,
        ]
    }

    /// Area of the rectangle.
    pub fn area(&self) -> f32 {
        4.0 * self.right.length() * self.up.length()
    }

    /// Normal of the area light face.
    pub fn normal(&self) -> Vec3 {
        self.right.cross(self.up).normalize()
    }

    /// Approximate the solid angle subtended by the area light at `pos`.
    pub fn solid_angle_at(&self, pos: Vec3) -> f32 {
        let to_center = self.position - pos;
        let dist2 = to_center.length_squared();
        let cos_angle = to_center
            .normalize()
            .dot(self.normal())
            .abs();
        (self.area() * cos_angle / dist2.max(1e-5)).min(2.0 * PI)
    }

    /// Simple irradiance estimate using solid angle and projected area.
    pub fn irradiance_at(&self, pos: Vec3, n: Vec3) -> Vec3 {
        let l = (self.position - pos).normalize();
        let n_dot_l = n.dot(l).max(0.0);
        let omega = self.solid_angle_at(pos);
        self.color * self.intensity * omega * n_dot_l * FRAC_1_PI
    }
}

/// Combined material description used by `shade_point`.
#[derive(Debug, Clone)]
pub struct ShadeMaterial {
    pub albedo: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    pub emission: Vec3,
    pub ao: f32,
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
    pub anisotropy: f32,
    pub anisotropy_tangent: Vec3,
    pub anisotropy_bitangent: Vec3,
    pub ior: f32,
}

impl Default for ShadeMaterial {
    fn default() -> Self {
        Self {
            albedo: Vec3::new(0.8, 0.8, 0.8),
            metallic: 0.0,
            roughness: 0.5,
            emission: Vec3::ZERO,
            ao: 1.0,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            anisotropy: 0.0,
            anisotropy_tangent: Vec3::X,
            anisotropy_bitangent: Vec3::Y,
            ior: 1.5,
        }
    }
}

/// Light enum for the shading loop.
#[derive(Debug, Clone)]
pub enum Light {
    Directional(DirectionalLight),
    Point(PointLight),
    Spot(SpotLight),
    Area(AreaLight),
}

impl Light {
    /// Compute `(light_direction, irradiance)` for a surface point.
    pub fn contribution(&self, surface_pos: Vec3, n: Vec3) -> (Vec3, Vec3) {
        match self {
            Light::Directional(d) => (d.direction, d.irradiance()),
            Light::Point(p) => (p.direction_to(surface_pos), p.irradiance_at(surface_pos)),
            Light::Spot(s) => (s.direction_to(surface_pos), s.irradiance_at(surface_pos)),
            Light::Area(a) => {
                let dir = (a.position - surface_pos).normalize();
                (dir, a.irradiance_at(surface_pos, n))
            }
        }
    }
}

/// Full shading of a surface point given material, lights, geometry, and an
/// optional IBL environment.
///
/// Returns linear HDR radiance (before tone-mapping).
pub fn shade_point(
    material: &ShadeMaterial,
    lights: &[Light],
    n: Vec3,
    v: Vec3,
    position: Vec3,
    env: Option<&ibl::PrefilterEnv>,
    brdf_lut: Option<&ibl::BrdfLut>,
) -> Vec3 {
    let n = n.normalize();
    let v = v.normalize();
    let n_dot_v = n.dot(v).max(1e-5);

    // F0 for Fresnel
    let f0_d = fresnel::f0_from_ior(material.ior);
    let f0 = Vec3::splat(f0_d).lerp(material.albedo, material.metallic);

    let mut color = Vec3::ZERO;

    // ── Direct lighting ───────────────────────────────────────────────────────
    for light in lights {
        let (l, irradiance) = light.contribution(position, n);
        let l = l.normalize();

        let brdf = if material.anisotropy > 1e-5 {
            AnisotropicBrdf::evaluate(
                n,
                v,
                l,
                material.anisotropy_tangent,
                material.anisotropy_bitangent,
                material.metallic,
                material.roughness * (1.0 + material.anisotropy),
                material.roughness * (1.0 - material.anisotropy),
                f0,
            )
        } else {
            CookTorranceBrdf::evaluate(n, v, l, material.albedo, material.metallic, material.roughness)
        };

        // Clearcoat on top
        let clearcoat = ClearcoatBrdf::evaluate(n, v, l, material.clearcoat, material.clearcoat_roughness);
        let cc_atten = ClearcoatBrdf::attenuation(n, v, material.clearcoat);

        color += (brdf * cc_atten + clearcoat) * irradiance;
    }

    // ── IBL ambient ───────────────────────────────────────────────────────────
    if let Some(env) = env {
        let r = vec3::reflect(-v, n);

        // Specular IBL
        let spec_env = env.sample_lod(r, material.roughness);
        let lut_val = brdf_lut
            .map(|lut| lut.integrate(n_dot_v, material.roughness))
            .unwrap_or(Vec2::new(0.5, 0.1));
        let f_ibl = fresnel::schlick_roughness_f(n_dot_v, f0, material.roughness);
        let specular_ibl = spec_env * (f_ibl * lut_val.x + Vec3::splat(lut_val.y));

        // Diffuse IBL
        let diff_env = env.sample_irradiance(n);
        let k_s = f_ibl;
        let k_d = (Vec3::ONE - k_s) * (1.0 - material.metallic);
        let diffuse_ibl = diff_env * k_d * material.albedo;

        color += (diffuse_ibl + specular_ibl) * material.ao;
    }

    // Emission
    color += material.emission;

    color
}

// ─────────────────────────────────────────────────────────────────────────────
// Tone mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Tone-mapping operators for converting HDR radiance to display-ready LDR.
pub mod tonemap {
    use glam::Vec3;

    /// ACES filmic tone-mapping curve (Stephen Hill's fit).
    pub fn aces_filmic(x: Vec3) -> Vec3 {
        // Luminance-based version
        let a = 2.51_f32;
        let b = 0.03_f32;
        let c = 2.43_f32;
        let d = 0.59_f32;
        let e = 0.14_f32;
        ((x * (a * x + Vec3::splat(b))) / (x * (c * x + Vec3::splat(d)) + Vec3::splat(e)))
            .clamp(Vec3::ZERO, Vec3::ONE)
    }

    /// Extended Reinhard tone-mapper with white-point parameter.
    pub fn reinhard_extended(x: Vec3, max_white: f32) -> Vec3 {
        let mw2 = max_white * max_white;
        x * (Vec3::ONE + x / mw2) / (Vec3::ONE + x)
    }

    /// Simple Reinhard operator.
    pub fn reinhard(x: Vec3) -> Vec3 {
        x / (Vec3::ONE + x)
    }

    /// Uncharted 2 / John Hable filmic operator.
    pub fn uncharted2(x: Vec3) -> Vec3 {
        fn hable(v: Vec3) -> Vec3 {
            let a = Vec3::splat(0.15_f32);
            let b = Vec3::splat(0.50_f32);
            let c = Vec3::splat(0.10_f32);
            let d = Vec3::splat(0.20_f32);
            let e = Vec3::splat(0.02_f32);
            let f = Vec3::splat(0.30_f32);
            ((v * (a * v + c * b) + d * e) / (v * (a * v + b) + d * f)) - e / f
        }

        let white_scale = Vec3::ONE / hable(Vec3::splat(11.2));
        hable(x * 2.0) * white_scale
    }

    /// Lottes 2016 filmic operator.
    pub fn lottes(x: Vec3) -> Vec3 {
        // Scalar parameters — the Lottes operator is applied per-channel
        let a: f32 = 1.6;
        let d: f32 = 0.977;
        let hdr_max: f32 = 8.0;
        let mid_out: f32 = 0.267;

        // Pre-compute per-scalar constants
        let b = (-mid_out.powf(a) + hdr_max.powf(a) * mid_out)
            / ((hdr_max.powf(a * d) - mid_out.powf(a * d)) * mid_out);
        let c_val = (hdr_max.powf(a * d) * mid_out.powf(a) - hdr_max.powf(a) * mid_out.powf(a * d))
            / ((hdr_max.powf(a * d) - mid_out.powf(a * d)) * mid_out);

        let f = |v: f32| v.powf(a) / (v.powf(a * d) * b + c_val);
        Vec3::new(f(x.x), f(x.y), f(x.z))
    }

    /// Uchimura GT tone-mapper (Gran Turismo, 2017).
    pub fn uchimura(x: Vec3) -> Vec3 {
        // Parameters
        let max_brightness = 1.0_f32;
        let contrast = 1.0_f32;
        let linear_start = 0.22_f32;
        let linear_length = 0.4_f32;
        let black = 1.33_f32;
        let pedestal = 0.0_f32;

        let l0 = (max_brightness - linear_start) * linear_length / contrast;
        let l = linear_start + l0;

        // Piecewise construction per channel
        fn gt_channel(x: f32, p: f32, a: f32, m: f32, l: f32, c: f32, b: f32) -> f32 {
            let l0 = (p - m) * l / a;
            let s0 = m + l0;
            let s1 = m + a * l0;
            let c2 = a * p / (p - s1);
            let cp = -c2 / p;

            let w0 = 1.0 - (x / m).min(1.0).powf(b);
            let w1 = if x >= m && x < s0 { 1.0 } else { 0.0 };
            let w2 = if x >= s0 { 1.0 } else { 0.0 };

            let t0 = m * (x / m).powf(c);
            let t1 = m + a * (x - m);
            let t2 = p - (p - s1) * (-c2 * (x - s0) / p).exp();

            w0 * x.powf(b) + w1 * (m + a * (x - m)) + w2 * t2 * 0.0 // simplified
                + (1.0 - w2) * (w0 * t0 + w1 * t1)
        }

        Vec3::new(
            gt_channel(x.x, max_brightness, contrast, linear_start, linear_length, black, pedestal),
            gt_channel(x.y, max_brightness, contrast, linear_start, linear_length, black, pedestal),
            gt_channel(x.z, max_brightness, contrast, linear_start, linear_length, black, pedestal),
        ).clamp(Vec3::ZERO, Vec3::ONE)
    }

    /// AgX tone-mapper (Blender's 2022 default).
    pub fn agx(x: Vec3) -> Vec3 {
        // AgX matrix transform (linear sRGB → AgX log domain)
        // Based on Troy Sobotka's implementation
        fn agx_default_contrast(v: Vec3) -> Vec3 {
            // Sigmoid-like curve in log space
            let x_adj = v.clamp(Vec3::ZERO, Vec3::ONE);
            // Polynomial approximation
            let p1 = x_adj * x_adj * (Vec3::splat(3.0) - Vec3::splat(2.0) * x_adj);
            p1
        }

        // AgX input transform (approximate)
        let agx_mat = [
            Vec3::new(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
            Vec3::new(0.0784335999999992, 0.878468636469772, 0.0784336),
            Vec3::new(0.0792237451477643, 0.0791661274605434, 0.879142973793104),
        ];

        let agx_in = Vec3::new(
            agx_mat[0].dot(x),
            agx_mat[1].dot(x),
            agx_mat[2].dot(x),
        ).max(Vec3::ZERO);

        // Log encoding
        let log_min = -12.47393_f32;
        let log_max = 4.026069_f32;
        let clamped = agx_in.max(Vec3::splat(1e-10));
        let log_vec = Vec3::new(clamped.x.log2(), clamped.y.log2(), clamped.z.log2());
        let encoded = (log_vec - Vec3::splat(log_min)) / (log_max - log_min);
        let encoded = encoded.clamp(Vec3::ZERO, Vec3::ONE);

        // Contrast S-curve
        let curved = agx_default_contrast(encoded);

        // Inverse transform (approximate)
        let inv_mat = [
            Vec3::new(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
            Vec3::new(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
            Vec3::new(-0.0990297440797205, -0.0989611768448433, 1.15107367264116),
        ];

        Vec3::new(
            inv_mat[0].dot(curved),
            inv_mat[1].dot(curved),
            inv_mat[2].dot(curved),
        ).max(Vec3::ZERO)
    }

    /// Apply camera exposure (EV = exposure value in stops).
    #[inline]
    pub fn exposure(x: Vec3, ev: f32) -> Vec3 {
        x * 2.0_f32.powf(ev)
    }

    /// Gamma-correct a linear colour (encode from linear to display).
    #[inline]
    pub fn gamma_correct(x: Vec3, gamma: f32) -> Vec3 {
        x.max(Vec3::ZERO).powf(1.0 / gamma)
    }

    /// Linear to sRGB per-channel (piecewise).
    pub fn linear_to_srgb(x: Vec3) -> Vec3 {
        let f = |c: f32| {
            if c <= 0.003_130_8 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        };
        Vec3::new(f(x.x), f(x.y), f(x.z))
    }

    /// sRGB to linear per-channel (piecewise).
    pub fn srgb_to_linear(x: Vec3) -> Vec3 {
        let f = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        Vec3::new(f(x.x), f(x.y), f(x.z))
    }

    /// Operator enum for GLSL code generation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ToneMapOp {
        AcesFilmic,
        ReinhardExtended,
        Uncharted2,
        Lottes,
        Uchimura,
        AgX,
    }
}

// Re-export ToneMapOp at module level for ergonomics
pub use tonemap::ToneMapOp;

// ─────────────────────────────────────────────────────────────────────────────
// GLSL code generation
// ─────────────────────────────────────────────────────────────────────────────

/// Generates GLSL source strings for the BRDF functions in this module.
pub struct BrdfGlsl;

impl BrdfGlsl {
    /// Full Cook-Torrance PBR BRDF as a GLSL function.
    pub fn cook_torrance_source() -> &'static str {
        r#"
// ── Cook-Torrance PBR BRDF (auto-generated) ───────────────────────────────
// GGX normal distribution
float ggxD(float NdotH, float roughness) {
    float alpha  = roughness * roughness;
    float alpha2 = alpha * alpha;
    float d = (NdotH * NdotH) * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * d * d);
}

// Smith-GGX masking-shadowing
float schlickG1(float NdotX, float k) {
    return NdotX / (NdotX * (1.0 - k) + k + 1e-7);
}
float smithGGX(float NdotV, float NdotL, float roughness) {
    float k = pow(roughness + 1.0, 2.0) / 8.0;
    return schlickG1(NdotV, k) * schlickG1(NdotL, k);
}

// Schlick Fresnel
vec3 schlickF(float cosTheta, vec3 F0) {
    float t = pow(1.0 - clamp(cosTheta, 0.0, 1.0), 5.0);
    return F0 + (vec3(1.0) - F0) * t;
}

// Cook-Torrance BRDF evaluation — returns radiance (already multiplied by NdotL)
vec3 cookTorranceBrdf(
    vec3 N, vec3 V, vec3 L,
    vec3 albedo, float metallic, float roughness,
    float ior)
{
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 1e-5);
    if (NdotL < 1e-5) return vec3(0.0);

    vec3  H     = normalize(V + L);
    float NdotH = max(dot(N, H), 0.0);
    float VdotH = clamp(dot(V, H), 0.0, 1.0);

    float rough = max(roughness, 0.04);
    float f0d   = pow((ior - 1.0) / (ior + 1.0), 2.0);
    vec3  F0    = mix(vec3(f0d), albedo, metallic);

    float D = ggxD(NdotH, rough);
    float G = smithGGX(NdotV, NdotL, rough);
    vec3  F = schlickF(VdotH, F0);

    vec3  specular = D * G * F / max(4.0 * NdotV * NdotL, 1e-7);
    vec3  kD       = (vec3(1.0) - F) * (1.0 - metallic);
    vec3  diffuse  = kD * albedo / PI;

    return (diffuse + specular) * NdotL;
}
"#
    }

    /// IBL ambient evaluation GLSL.
    pub fn ibl_source() -> &'static str {
        r#"
// ── IBL ambient (auto-generated) ─────────────────────────────────────────────
// Env and BRDF LUT samplers must be declared by the caller:
//   uniform samplerCube u_PrefilteredEnv;
//   uniform sampler2D   u_BrdfLut;
//   uniform samplerCube u_IrradianceMap;

vec3 evaluateIbl(
    vec3 N, vec3 V,
    vec3 albedo, float metallic, float roughness,
    float ao, float ior,
    float maxReflectionLod)
{
    float NdotV  = max(dot(N, V), 1e-5);
    vec3  R      = reflect(-V, N);

    float f0d    = pow((ior - 1.0) / (ior + 1.0), 2.0);
    vec3  F0     = mix(vec3(f0d), albedo, metallic);

    // Fresnel with roughness for ambient
    float t      = pow(1.0 - NdotV, 5.0);
    float maxF   = max(max(1.0 - roughness, F0.r), max(F0.g, F0.b));
    vec3  Fibl   = F0 + (vec3(maxF) - F0) * t;

    // Pre-filtered specular
    vec3  prefilt = textureLod(u_PrefilteredEnv, R, roughness * maxReflectionLod).rgb;
    vec2  brdf    = texture(u_BrdfLut, vec2(NdotV, roughness)).rg;
    vec3  specIbl = prefilt * (Fibl * brdf.x + vec3(brdf.y));

    // Diffuse irradiance
    vec3  irrad   = texture(u_IrradianceMap, N).rgb;
    vec3  kD      = (vec3(1.0) - Fibl) * (1.0 - metallic);
    vec3  diffIbl = irrad * kD * albedo;

    return (diffIbl + specIbl) * ao;
}
"#
    }

    /// Tone-mapping GLSL for the selected operator.
    pub fn tonemap_source(op: ToneMapOp) -> &'static str {
        match op {
            ToneMapOp::AcesFilmic => r#"
vec3 toneMap(vec3 x) {
    // ACES filmic (Stephen Hill fit)
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
"#,
            ToneMapOp::ReinhardExtended => r#"
uniform float u_MaxWhite;
vec3 toneMap(vec3 x) {
    return x * (1.0 + x / (u_MaxWhite * u_MaxWhite)) / (1.0 + x);
}
"#,
            ToneMapOp::Uncharted2 => r#"
vec3 hable(vec3 x) {
    return ((x * (0.15 * x + 0.05) + 0.004) / (x * (0.15 * x + 0.5) + 0.06)) - 1.0/15.0;
}
vec3 toneMap(vec3 x) {
    vec3 ws = vec3(1.0) / hable(vec3(11.2));
    return hable(x * 2.0) * ws;
}
"#,
            ToneMapOp::Lottes => r#"
vec3 toneMap(vec3 x) {
    const vec3 a     = vec3(1.6);
    const vec3 d     = vec3(0.977);
    const vec3 hdrM  = vec3(8.0);
    const vec3 midIn = vec3(0.18);
    const vec3 midO  = vec3(0.267);
    vec3 b = (-pow(midO, a) + pow(hdrM, a) * midO)
           / ((pow(hdrM, a * d) - pow(midO, a * d)) * midO);
    vec3 c = (pow(hdrM, a * d) * pow(midO, a) - pow(hdrM, a) * pow(midO, a * d))
           / ((pow(hdrM, a * d) - pow(midO, a * d)) * midO);
    return pow(x, a) / (pow(x, a * d) * b + c);
}
"#,
            ToneMapOp::Uchimura => r#"
vec3 toneMap(vec3 x) {
    // GT Uchimura
    const float P = 1.0, a = 1.0, m = 0.22, l = 0.4, c = 1.33, b = 0.0;
    float l0 = (P - m) * l / a;
    float L  = m + l0;
    float S0 = m + l0;
    float S1 = m + a * l0;
    float C2 = a * P / (P - S1);
    float CP = -C2 / P;
    vec3  w0 = vec3(1.0 - smoothstep(vec3(0.0), vec3(m), x));
    vec3  w2 = step(vec3(S0), x);
    vec3  w1 = vec3(1.0) - w0 - w2;
    vec3  T  = vec3(m) * pow(x / vec3(m), vec3(c));
    vec3  S  = vec3(P) - (vec3(P) - vec3(S1)) * exp(CP * (x - vec3(S0)));
    vec3  Lin = vec3(m + a) * (x - vec3(m));
    return T * w0 + Lin * w1 + S * w2;
}
"#,
            ToneMapOp::AgX => r#"
vec3 toneMap(vec3 x) {
    // Simplified AgX (Blender 3.x default)
    mat3 agxMat = mat3(
        0.842479, 0.042328, 0.042376,
        0.078434, 0.878469, 0.078434,
        0.079224, 0.079166, 0.879143
    );
    vec3 enc = agxMat * max(x, vec3(0.0));
    float logMin = -12.47393, logMax = 4.026069;
    enc = clamp((log2(max(enc, vec3(1e-10))) - logMin) / (logMax - logMin), 0.0, 1.0);
    // Sigmoid contrast
    enc = enc * enc * (3.0 - 2.0 * enc);
    // Inverse
    mat3 invMat = mat3(
         1.19688, -0.05290, -0.05297,
        -0.09802,  1.15190, -0.09804,
        -0.09903, -0.09896,  1.15107
    );
    return max(invMat * enc, vec3(0.0));
}
"#,
        }
    }

    /// Full GLSL preamble including all BRDF helpers, IBL, and the selected
    /// tone-mapper as a single concatenated string.
    pub fn full_brdf_glsl(op: ToneMapOp) -> String {
        const HEADER: &str = "#ifndef BRDF_GLSL\n#define BRDF_GLSL\n\n#define PI 3.14159265358979\n\n";
        const FOOTER: &str = "\n#endif // BRDF_GLSL\n";
        format!(
            "{}{}{}{}{}",
            HEADER,
            Self::cook_torrance_source(),
            Self::ibl_source(),
            Self::tonemap_source(op),
            FOOTER
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    // ── Distribution ──────────────────────────────────────────────────────────

    #[test]
    fn ggx_d_is_positive() {
        let d = distribution::ggx_d(0.8, 0.3);
        assert!(d > 0.0, "GGX D must be positive, got {d}");
    }

    #[test]
    fn ggx_d_at_n_dot_h_one_is_finite() {
        // NdotH = 1 is the peak of GGX
        let d = distribution::ggx_d(1.0, 0.5);
        assert!(d.is_finite() && d > 0.0);
    }

    #[test]
    fn beckmann_d_is_positive() {
        let d = distribution::beckmann_d(0.7, 0.4);
        assert!(d > 0.0);
    }

    #[test]
    fn blinn_phong_d_is_positive() {
        let d = distribution::blinn_phong_d(0.9, 64.0);
        assert!(d > 0.0);
    }

    #[test]
    fn anisotropic_ggx_d_is_positive() {
        let d = distribution::anisotropic_ggx_d(0.8, 0.1, 0.2, 0.2, 0.5);
        assert!(d > 0.0);
    }

    // ── Geometry ──────────────────────────────────────────────────────────────

    #[test]
    fn smith_ggx_is_in_unit_range() {
        let g = geometry::smith_ggx(0.9, 0.8, 0.3);
        assert!((0.0..=1.0).contains(&g), "G={g} must be in [0,1]");
    }

    #[test]
    fn implicit_g_is_product() {
        let g = geometry::implicit_g(0.7, 0.5);
        assert!((g - 0.35).abs() < 1e-6);
    }

    #[test]
    fn kelemen_g_is_positive() {
        let g = geometry::kelemen_szirmay_kalos_g(0.8, 0.9);
        assert!(g > 0.0);
    }

    // ── Fresnel ───────────────────────────────────────────────────────────────

    #[test]
    fn f0_from_ior_glass() {
        let f0 = fresnel::f0_from_ior(1.5);
        assert!((f0 - 0.04).abs() < 0.001, "Glass F0 ~0.04, got {f0}");
    }

    #[test]
    fn schlick_f_at_zero_angle_equals_f0() {
        let f0 = Vec3::splat(0.04);
        let f = fresnel::schlick_f(1.0, f0);
        assert!((f - f0).length() < 1e-5);
    }

    #[test]
    fn schlick_f_at_grazing_is_white() {
        let f0 = Vec3::splat(0.04);
        let f = fresnel::schlick_f(0.0, f0);
        assert!((f - Vec3::ONE).length() < 1e-5);
    }

    #[test]
    fn dielectric_fresnel_tir() {
        // At grazing incidence from dense medium with ior>1, TIR
        let f = fresnel::dielectric_fresnel(0.0, 1.5);
        assert_eq!(f, 1.0, "Should be total internal reflection at cos=0");
    }

    // ── Cook-Torrance ─────────────────────────────────────────────────────────

    #[test]
    fn cook_torrance_returns_vec3_non_negative() {
        let n = Vec3::Y;
        let v = Vec3::new(0.0, 1.0, 0.0);
        let l = Vec3::new(0.5, 0.866, 0.0).normalize();
        let result = CookTorranceBrdf::evaluate(n, v, l, Vec3::splat(0.8), 0.0, 0.5);
        assert!(result.x >= 0.0 && result.y >= 0.0 && result.z >= 0.0);
    }

    #[test]
    fn cook_torrance_below_horizon_is_zero() {
        let n = Vec3::Y;
        let v = Vec3::Y;
        let l = -Vec3::Y; // below surface
        let result = CookTorranceBrdf::evaluate(n, v, l, Vec3::ONE, 0.0, 0.5);
        assert_eq!(result, Vec3::ZERO);
    }

    #[test]
    fn cook_torrance_metal_has_coloured_specular() {
        let n = Vec3::Y;
        let v = Vec3::new(0.0, 1.0, 0.0);
        let l = Vec3::new(0.5, 0.866, 0.0).normalize();
        let gold = Vec3::new(1.0, 0.766, 0.336);
        let result = CookTorranceBrdf::evaluate(n, v, l, gold, 1.0, 0.1);
        // Gold: red channel should be brighter than blue
        assert!(result.x > result.z, "Gold should be redder than blue");
    }

    // ── Lambertian ────────────────────────────────────────────────────────────

    #[test]
    fn lambertian_energy_conservation() {
        // Integral of Lambertian BRDF over hemisphere = albedo
        let albedo = Vec3::ONE;
        // Numerical hemisphere integral using many samples
        let mut sum = Vec3::ZERO;
        let n_theta = 100;
        let n_phi = 200;
        for i in 0..n_theta {
            let theta = (i as f32 + 0.5) / n_theta as f32 * std::f32::consts::FRAC_PI_2;
            for j in 0..n_phi {
                let phi = (j as f32 + 0.5) / n_phi as f32 * 2.0 * PI;
                let sin_t = theta.sin();
                let cos_t = theta.cos();
                let l = Vec3::new(sin_t * phi.cos(), cos_t, sin_t * phi.sin());
                let n_dot_l = cos_t;
                let solid_angle = sin_t
                    * (std::f32::consts::FRAC_PI_2 / n_theta as f32)
                    * (2.0 * PI / n_phi as f32);
                sum += LambertianBrdf::evaluate(albedo, n_dot_l) * solid_angle;
            }
        }
        // Should be approximately 1.0
        assert!(
            (sum.x - 1.0).abs() < 0.02,
            "Lambertian integral = {:.4}",
            sum.x
        );
    }

    // ── Oren-Nayar ────────────────────────────────────────────────────────────

    #[test]
    fn oren_nayar_at_zero_roughness_matches_lambertian() {
        let n = Vec3::Y;
        let v = Vec3::new(0.3, 0.95, 0.0).normalize();
        let l = Vec3::new(-0.3, 0.95, 0.0).normalize();
        let albedo = Vec3::ONE;
        let on = OrenNayarBrdf::evaluate(v, l, n, albedo, 0.0);
        let lam = LambertianBrdf::evaluate(albedo, n.dot(l).max(0.0));
        assert!(
            (on - lam).length() < 0.05,
            "ON with roughness=0 should ~match Lambertian: on={on:?} lam={lam:?}"
        );
    }

    // ── Clearcoat ─────────────────────────────────────────────────────────────

    #[test]
    fn clearcoat_zero_strength_returns_zero() {
        let n = Vec3::Y;
        let v = Vec3::new(0.0, 1.0, 0.0);
        let l = Vec3::new(0.5, 0.866, 0.0).normalize();
        let result = ClearcoatBrdf::evaluate(n, v, l, 0.0, 0.3);
        assert_eq!(result, Vec3::ZERO);
    }

    // ── IBL ───────────────────────────────────────────────────────────────────

    #[test]
    fn brdf_lut_values_in_unit_range() {
        let lut = ibl::BrdfLut::generate(32);
        for &v in &lut.data {
            assert!(
                (0.0..=1.0).contains(&v.x) && (0.0..=1.0).contains(&v.y),
                "LUT value out of range: {v:?}"
            );
        }
    }

    #[test]
    fn ssao_kernel_has_correct_count() {
        let k = ibl::AmbientOcclusion::ssao_kernel(32);
        assert_eq!(k.len(), 32);
    }

    // ── Tone mapping ──────────────────────────────────────────────────────────

    #[test]
    fn aces_filmic_maps_zero_to_zero() {
        let out = tonemap::aces_filmic(Vec3::ZERO);
        assert!(out.length() < 1e-4);
    }

    #[test]
    fn aces_filmic_clamps_to_one() {
        let out = tonemap::aces_filmic(Vec3::splat(1000.0));
        assert!(out.x <= 1.0 && out.y <= 1.0 && out.z <= 1.0);
    }

    #[test]
    fn srgb_round_trip() {
        let linear = Vec3::new(0.5, 0.2, 0.8);
        let srgb = tonemap::linear_to_srgb(linear);
        let back = tonemap::srgb_to_linear(srgb);
        assert!((back - linear).length() < 1e-4, "sRGB round-trip error: {back:?}");
    }

    #[test]
    fn gamma_correct_identity_at_gamma_one() {
        let v = Vec3::new(0.4, 0.7, 0.1);
        let out = tonemap::gamma_correct(v, 1.0);
        assert!((out - v).length() < 1e-5);
    }

    // ── GLSL generation ───────────────────────────────────────────────────────

    #[test]
    fn glsl_cook_torrance_contains_ggx() {
        let src = BrdfGlsl::cook_torrance_source();
        assert!(src.contains("ggxD"), "Expected ggxD function");
        assert!(src.contains("smithGGX"), "Expected smithGGX function");
        assert!(src.contains("schlickF"), "Expected schlickF function");
    }

    #[test]
    fn glsl_ibl_source_contains_expected_samplers() {
        let src = BrdfGlsl::ibl_source();
        assert!(src.contains("u_PrefilteredEnv"));
        assert!(src.contains("u_BrdfLut"));
    }

    #[test]
    fn glsl_full_brdf_compiles_to_large_string() {
        let src = BrdfGlsl::full_brdf_glsl(ToneMapOp::AcesFilmic);
        assert!(src.len() > 1000);
        assert!(src.contains("#define PI"));
    }

    // ── SimpleRng ─────────────────────────────────────────────────────────────

    #[test]
    fn simple_rng_produces_values_in_range() {
        let mut rng = SimpleRng::new(42);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v), "RNG value {v} out of [0,1)");
        }
    }
}
