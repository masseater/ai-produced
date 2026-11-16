use std::convert::Infallible;

use basin::{
    BoundedCmaEs, BoxBounds, ConstraintDescription, ConstraintSet, Constraints, CostFunction, De,
    DenseMatrix, Executor, PopulationProgress,
};

struct Boxed<F> {
    f: F,
    lower: Vec<f64>,
    upper: Vec<f64>,
}

impl<F: Fn(&[f64]) -> f64> CostFunction for Boxed<F> {
    type Param = Vec<f64>;
    type Output = f64;
    type Error = Infallible;

    fn cost(&self, x: &Vec<f64>) -> Result<f64, Infallible> {
        let value = (self.f)(x);
        Ok(if value.is_finite() { value } else { 99.0 })
    }
}

impl<F> Constraints for Boxed<F> {
    type Set = ConstraintSet<BoxBounds<Vec<f64>>>;

    fn constraints(&self) -> <Self::Set as ConstraintDescription>::Borrowed<'_> {
        ConstraintSet::new().with_bounds(BoxBounds::new(Some(&self.lower), Some(&self.upper)))
    }
}

pub struct Best {
    pub x: Vec<f64>,
    pub f: f64,
}

pub fn differential_evolution<F: Fn(&[f64]) -> f64>(
    f: F,
    bounds: &[(f64, f64)],
    population_per_dimension: usize,
    max_iter: u64,
    seed: u64,
) -> anyhow::Result<Best> {
    let problem = Boxed {
        f,
        lower: bounds.iter().map(|b| b.0).collect(),
        upper: bounds.iter().map(|b| b.1).collect(),
    };
    let solver = De::<f64>::new(seed)
        .with_pop_size((population_per_dimension * bounds.len()).max(5))
        .with_dither(0.5, 1.0)
        .with_cr(0.7);
    let result = Executor::new(problem, solver, PopulationProgress::<Vec<f64>>::empty())
        .max_iter(max_iter)
        .run()
        .map_err(|e| anyhow::anyhow!("optimizer failed: {e:?}"))?;
    Ok(Best {
        x: result.best_param().clone(),
        f: result.best_cost(),
    })
}

pub fn cma_es<F: Fn(&[f64]) -> f64>(
    f: F,
    x0: Vec<f64>,
    sigma: f64,
    lambda: usize,
    max_iter: u64,
    seed: u64,
) -> anyhow::Result<Best> {
    let n = x0.len();
    let problem = Boxed {
        f,
        lower: vec![0.0; n],
        upper: vec![1.0; n],
    };
    let solver = BoundedCmaEs::<Vec<f64>, DenseMatrix>::new(seed, sigma).with_lambda(lambda);
    let result = Executor::new(
        problem,
        solver,
        PopulationProgress::<Vec<f64>>::from_point(x0),
    )
    .max_iter(max_iter)
    .run()
    .map_err(|e| anyhow::anyhow!("optimizer failed: {e:?}"))?;
    Ok(Best {
        x: result.best_param().clone(),
        f: result.best_cost(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(x: &[f64]) -> f64 {
        x.iter().map(|v| (v - 0.3) * (v - 0.3)).sum()
    }

    #[test]
    fn differential_evolution_finds_the_minimum() {
        let best = differential_evolution(sphere, &[(0.0, 1.0); 3], 10, 100, 1).unwrap();
        assert!(best.f < 1e-6, "{}", best.f);
    }

    #[test]
    fn cma_es_finds_the_minimum_inside_the_box() {
        let best = cma_es(sphere, vec![0.8; 3], 0.25, 12, 150, 1).unwrap();
        assert!(best.f < 1e-6, "{}", best.f);
        assert!(best.x.iter().all(|v| (0.0..=1.0).contains(v)));
    }
}
