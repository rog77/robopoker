use super::heuristic::Heuristic;
use super::histogram::Histogram;
use super::metric::Metric;
use super::pair::Pair;
use super::sinkhorn::Sinkhorn;
use crate::transport::coupling::Coupling;
use crate::Arbitrary;
use std::collections::BTreeMap;

/// this guy is used just to construct arbitrary metric, histogram, histogram tuples
/// to test transport mechanisms
pub struct EMD(Metric, Histogram, Histogram);

impl EMD {
    pub fn metric(&self) -> &Metric {
        &self.0
    }
    pub fn sinkhorn(&self) -> Sinkhorn {
        Sinkhorn::from((&self.1, &self.2, &self.0)).minimize()
    }
    pub fn heuristic(&self) -> Heuristic {
        Heuristic::from((&self.1, &self.2, &self.0)).minimize()
    }
}

impl Arbitrary for EMD {
    fn random() -> Self {
        // construct random metric satisfying symmetric semipositivity
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let p = Histogram::random();
        let q = Histogram::random();
        let m = Metric::from(
            p.support()
                .chain(q.support())
                .flat_map(|x| p.support().chain(q.support()).map(move |y| (x, y)))
                .filter(|(x, y)| x > y)
                .map(|(x, y)| Pair::from((x, y)))
                .map(|paired| (paired, rng.gen::<f32>()))
                .collect::<BTreeMap<_, _>>(),
        );
        Self(m, p, q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::observation::Observation;
    use crate::cards::street::Street;
    use crate::clustering::histogram::Histogram;

    #[test]
    fn is_equity_emd_symmetric() {
        let metric = Metric::default();
        let ref h1 = Histogram::from(Observation::from(Street::Turn));
        let ref h2 = Histogram::from(Observation::from(Street::Turn));
        let d12 = metric.emd(h1, h2);
        let d21 = metric.emd(h2, h1);
        assert!(d12 == d21);
    }
    #[test]
    fn is_equity_emd_positive() {
        let metric = Metric::default();
        let ref h1 = Histogram::from(Observation::from(Street::Turn));
        let ref h2 = Histogram::from(Observation::from(Street::Turn));
        let d12 = metric.emd(h1, h2);
        let d21 = metric.emd(h2, h1);
        assert!(d12 > 0.);
        assert!(d21 > 0.);
    }
    #[test]
    fn is_equity_emd_zero() {
        let metric = Metric::default();
        let h = Histogram::from(Observation::from(Street::Turn));
        let d = metric.emd(&h, &h);
        assert!(d == 0.);
    }

    #[test]
    fn is_sinkhorn_emd_positive() {
        let EMD(metric, h1, h2) = EMD::random();
        let d12 = Sinkhorn::from((&h1, &h2, &metric)).minimize().cost();
        let d21 = Sinkhorn::from((&h2, &h1, &metric)).minimize().cost();
        assert!(d12 > 0., "non positive \n{} \n{}", d12, d21);
        assert!(d21 > 0., "non positive \n{} \n{}", d12, d21);
    }
    #[test]
    fn is_sinkhorn_emd_zero() {
        const TOLERANCE: f32 = 1e-4;
        let EMD(metric, h1, h2) = EMD::random();
        let d11 = Sinkhorn::from((&h1, &h1, &metric)).minimize().cost();
        let d22 = Sinkhorn::from((&h2, &h2, &metric)).minimize().cost();
        assert!(d11 <= TOLERANCE, "non zero: \n{} \n{}", d11, d22);
        assert!(d22 <= TOLERANCE, "non zero: \n{} \n{}", d11, d22);
    }

    #[test]
    fn is_heuristic_emd_positive() {
        let EMD(metric, h1, h2) = EMD::random();
        let d12 = Heuristic::from((&h1, &h2, &metric)).minimize().cost();
        let d21 = Heuristic::from((&h2, &h1, &metric)).minimize().cost();
        assert!(d12 > 0., "non positive \n{} \n{}", d12, d21);
        assert!(d21 > 0., "non positive \n{} \n{}", d12, d21);
    }
    #[test]
    fn is_heuristic_emd_zero() {
        let EMD(metric, h1, h2) = EMD::random();
        let d11 = Heuristic::from((&h1, &h1, &metric)).minimize().cost();
        let d22 = Heuristic::from((&h2, &h2, &metric)).minimize().cost();
        assert!(d11 == 0., "non zero: \n{} \n{}", d11, d22);
        assert!(d22 == 0., "non zero: \n{} \n{}", d11, d22);
    }
}
