use crate::hittable::HitRecord;
use crate::material::Material;
use crate::material::fresnel_schlick;
use crate::material::geometry_schlick_ggx;
use crate::material::pdf_vndf_ggx;
use crate::material::sample_vndf_ggx;
use crate::ray::Ray;
use utils::Color;

use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CookTorrance {
    pub albedo: Color,
    pub roughness: f32,
    pub metallic: f32,
}

impl CookTorrance {
    pub fn new(albedo: Color, roughness: f32, metallic: f32) -> Self {
        Self {
            albedo,
            roughness: roughness.clamp(0.05, 1.0),
            metallic: metallic.clamp(0.0, 1.0),
        }
    }

    // GGX sample (based on spherical coordinates)
    #[allow(dead_code)]
    fn sample_ggx(normal: utils::Vec3, roughness: f32) -> utils::Vec3 {
        let u1 = utils::random();
        let u2 = utils::random();

        let a = roughness * roughness;

        let theta = f32::acos(f32::sqrt((1.0 - u1) / (1.0 + (a * a - 1.0) * u1)));
        let phi = 2.0 * std::f32::consts::PI * u2;

        let sin_theta = f32::sin(theta);
        let x = sin_theta * f32::cos(phi);
        let y = sin_theta * f32::sin(phi);
        let z = f32::cos(theta);

        let h_local = utils::Vec3::new(x, y, z);
        utils::align_to_normal(h_local, normal)
    }
    #[allow(dead_code)]
    fn pdf_ggx(normal: utils::Vec3, h: utils::Vec3, roughness: f32) -> f32 {
        let a = roughness * roughness;
        let a2 = a * a;
        let n_dot_h = f32::max(utils::dot(normal, h), 0.0);
        let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
        let d = a2 / (std::f32::consts::PI * denom * denom);
        d * n_dot_h / (4.0 * utils::dot(h, utils::unit_vector(h)).abs())
    }
}

impl Material for CookTorrance {
    fn scatter(
        &self,
        r_in: &Ray,
        rec: &HitRecord,
        attenuation: &mut Color,
        scattered: &mut Ray,
    ) -> bool {
        let n = rec.normal;
        let v = -utils::unit_vector(r_in.direction());

        // Sample a halfway vector using VNDF
        let h = sample_vndf_ggx(v, self.roughness);
        let l = utils::reflect(-v, h);
        if utils::dot(l, n) <= 0.0 {
            return false;
        }

        let n_dot_v = utils::dot(n, v).max(1e-4);
        let n_dot_l = utils::dot(n, l).max(1e-4);
        let n_dot_h = utils::dot(n, h).max(1e-4);
        let v_dot_h = utils::dot(v, h).max(1e-4);

        let f0 = Color::new(0.04, 0.04, 0.04).lerp(self.albedo, self.metallic);
        let f = fresnel_schlick(v_dot_h, f0);

        let a = self.roughness * self.roughness;
        let a2 = a * a;
        let denom = (n_dot_h * n_dot_h * (a2 - 1.0) + 1.0).powi(2);
        let d = a2 / (std::f32::consts::PI * denom);

        let g = geometry_schlick_ggx(n_dot_v, self.roughness)
            * geometry_schlick_ggx(n_dot_l, self.roughness);
        let specular = (f * d * g) / (4.0 * n_dot_v * n_dot_l + 1e-4);
        let kd = (Color::new(1.0, 1.0, 1.0) - f) * (1.0 - self.metallic);
        let diffuse = self.albedo / std::f32::consts::PI;

        *attenuation = kd * diffuse + specular;
        *scattered = Ray::new(rec.p, l);

        true
    }

    fn scatter_importance(&self, r_in: &Ray, rec: &HitRecord) -> Option<(Ray, Color, f32)> {
        let n = rec.normal;
        let v = -utils::unit_vector(r_in.direction());

        let sample_specular = utils::random() < 0.5;

        let (l, pdf_specular, pdf_diffuse, brdf) = if sample_specular {
            // === Sample GGX specular ===
            let h = sample_vndf_ggx(v, self.roughness);
            let l = utils::reflect(-v, h);
            if utils::dot(l, n) <= 0.0 {
                return None;
            }

            // PDFs
            let pdf_ggx = pdf_vndf_ggx(v, h, n, self.roughness);
            let cosine = utils::dot(n, l).max(1e-4);
            let pdf_cosine = cosine / std::f32::consts::PI;

            // Fresnel term
            let f0 = Color::new(0.04, 0.04, 0.04).lerp(self.albedo, self.metallic);
            let f = fresnel_schlick(utils::dot(v, h), f0);

            // NDF
            let a = self.roughness * self.roughness;
            let a2 = a * a;
            let n_dot_h = utils::dot(n, h).max(1e-4);
            let denom = (n_dot_h * n_dot_h * (a2 - 1.0) + 1.0).powi(2);
            let d = a2 / (std::f32::consts::PI * denom);

            // Geometry term
            let g = geometry_schlick_ggx(utils::dot(n, v), self.roughness)
                * geometry_schlick_ggx(utils::dot(n, l), self.roughness);

            let spec = (f * d * g) / (4.0 * utils::dot(n, v) * utils::dot(n, l) + 1e-4);

            let kd = (Color::new(1.0, 1.0, 1.0) - f) * (1.0 - self.metallic);
            let diffuse = self.albedo / std::f32::consts::PI;

            let brdf = kd * diffuse + spec;

            (l, pdf_ggx * 0.5, pdf_cosine * 0.5, brdf)
        } else {
            // === Sample cosine-weighted hemisphere (diffuse) ===
            let l_local = utils::random_cosine_direction();
            let l = utils::align_to_normal(l_local, n);
            if utils::dot(l, n) <= 0.0 {
                return None;
            }

            let h = utils::unit_vector(v + l);
            let pdf_cosine = utils::dot(n, l).max(1e-4) / std::f32::consts::PI;
            let pdf_ggx = pdf_vndf_ggx(v, h, n, self.roughness);

            let f0 = Color::new(0.04, 0.04, 0.04).lerp(self.albedo, self.metallic);
            let f = fresnel_schlick(utils::dot(v, h), f0);

            let a = self.roughness * self.roughness;
            let a2 = a * a;
            let n_dot_h = utils::dot(n, h).max(1e-4);
            let denom = (n_dot_h * n_dot_h * (a2 - 1.0) + 1.0).powi(2);
            let d = a2 / (std::f32::consts::PI * denom);

            let g = geometry_schlick_ggx(utils::dot(n, v), self.roughness)
                * geometry_schlick_ggx(utils::dot(n, l), self.roughness);

            let spec = (f * d * g) / (4.0 * utils::dot(n, v) * utils::dot(n, l) + 1e-4);
            let kd = (Color::new(1.0, 1.0, 1.0) - f) * (1.0 - self.metallic);
            let diffuse = self.albedo / std::f32::consts::PI;

            let brdf = kd * diffuse + spec;

            (l, pdf_ggx * 0.5, pdf_cosine * 0.5, brdf)
        };

        let weight = utils::balance_heuristic(pdf_specular, pdf_diffuse);
        let final_pdf = pdf_specular + pdf_diffuse;
        let n_dot_l = utils::dot(n, l).max(1e-4);

        let scattered = Ray::new(rec.p, l);
        Some((scattered, brdf * n_dot_l * weight, final_pdf.max(1e-4)))
    }
}
