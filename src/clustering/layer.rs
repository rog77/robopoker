use super::abstraction::Abstraction;
use super::datasets::AbstractionSpace;
use super::datasets::IsomorphismSpace;
use super::encoding::Encoder;
use super::histogram::Histogram;
use super::metric::Metric;
use super::xor::Pair;
use crate::cards::isomorphism::Isomorphism;
use crate::cards::observation::Observation;
use crate::cards::street::Street;
use rand::distributions::Distribution;
use rand::distributions::WeightedIndex;
use rand::seq::IteratorRandom;
use rand::Rng;
use rayon::iter::IntoParallelIterator;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::collections::BTreeMap;

/// Hierarchical K Means Learner.
/// this is decomposed into the necessary data structures
/// for kmeans clustering to occur for a given `Street`.
/// it should also parallelize well, with kmeans and lookup
/// being the only mutable fields.
/// EMD dominates compute, by introducing a k^2 dependence
/// for every distance calculation.
///
/// ## kmeans initialization:
/// - CPU := (# centroids)^2 *   (# isomorphisms)
/// - RAM := (# centroids)   +   (# isomorphisms)
///
/// ## kmeans clustering:
/// - CPU := (# centroids)^3 *   (# isomorphisms)   *    (# iterations)
/// - RAM := (# centroids)   +   (# isomorphisms)
///
/// ## metric calculation:
/// - CPU := O(# centroids)^2
/// - RAM := O(# centroids)^2
///
pub struct Layer {
    street: Street,
    metric: Metric,
    lookup: Encoder,
    kmeans: AbstractionSpace,
    points: IsomorphismSpace,
}

impl Layer {
    /// start with the River layer. everything is empty because we
    /// can generate `Abstractor` and `SmallSpace` from "scratch".
    /// - `lookup`: lazy equity calculation of river observations
    /// - `kmeans`: equity percentile buckets of equivalent river observations
    /// - `metric`: absolute value of `Abstraction::Equity` difference
    /// - `points`: not used for inward projection. only used for clustering. and no clustering on River.
    pub fn outer() -> Self {
        Self {
            street: Street::Rive,
            metric: Metric::default(),
            lookup: Encoder::rivers(),
            kmeans: AbstractionSpace::default(),
            points: IsomorphismSpace::default(),
        }
    }
    /// hierarchically, recursively generate the inner layer
    /// 0. initialize empty lookup table and kmeans centroids
    /// 1. generate Street, Metric, and Points as a pure function of the outer Layer
    /// 2. initialize kmeans centroids with weighted random Observation sampling (kmeans++ for faster convergence)
    /// 3. cluster kmeans centroids
    pub fn inner(&self) -> Self {
        let mut layer = Self {
            street: self.inner_street(),         // uniquely determined by outer layer
            metric: self.inner_metric(),         // uniquely determined by outer layer
            points: self.inner_points(),         // uniquely determined by outer layer
            kmeans: AbstractionSpace::default(), // assigned during clustering
            lookup: Encoder::default(),          // assigned during clustering
        };
        layer.cluster();
        layer
    }

    /// edge case for Preflop where we want to calculate the raised metric
    /// without saving it to disk under self.street.next() (i.e. its 1st)
    fn cluster(&mut self) {
        self.metric.save(self.street.next());
        self.kmeans_initial();
        self.kmeans_cluster();
        if self.street == Street::Pref {
            self.metric = self.inner_metric();
            self.metric.save(self.street);
        } else {
            self.lookup.save(self.street);
        }
    }

    /// simply go to the previous street
    fn inner_street(&self) -> Street {
        log::info!(
            "{:<32}{:<32}",
            "advancing street",
            format!("{} <- {}", self.street.prev(), self.street)
        );
        self.street.prev()
    }
    /// compute the outer product of the `Abstraction -> Histogram`s at the current layer,
    /// - generate the _inner layer_ `Metric` between `Abstraction`s
    /// - by using the _outer layer_ `Metric` between `Histogram`s via EMD calcluations.
    ///
    /// we symmetrize the distance by averaging the EMDs in both directions.
    /// the distnace isn't symmetric in the first place only because our greedy heuristic algo
    /// will find different optimal Coupling/Transport plans depending on which direction we consider.
    fn inner_metric(&self) -> Metric {
        log::info!(
            "{:<32}{:<32}",
            "computing metric",
            format!("{} <- {}", self.street.prev(), self.street)
        );
        let mut metric = BTreeMap::new();
        for a in self.kmeans.0.keys() {
            for b in self.kmeans.0.keys() {
                if a > b {
                    let index = Pair::from((a, b));
                    let x = self.kmeans.0.get(a).expect("pre-computed").histogram();
                    let y = self.kmeans.0.get(b).expect("pre-computed").histogram();
                    let distance = self.metric.emd(x, y) + self.metric.emd(y, x);
                    let distance = distance / 2.;
                    metric.insert(index, distance);
                }
            }
        }
        Metric::from(metric)
    }
    /// using the current layer's `Abstractor`,
    /// we generate the `LargeSpace` of `Observation` -> `Histogram`.
    /// 1. take all `Observation`s for `self.street.prev()`
    /// 2. map each to possible `self.street` `Observation`s
    /// 3. use `self.abstractor` to map each into an `Abstraction`
    /// 4. collect `Abstraction`s into a `Histogram`, for each `Observation`
    fn inner_points(&self) -> IsomorphismSpace {
        log::info!(
            "{:<32}{:<32}",
            "collecting histograms",
            format!("{} <- {}", self.street.prev(), self.street)
        );
        let progress = crate::progress(self.street.n_isomorphisms());
        let projection = Observation::exhaust(self.street.prev())
            .filter(Isomorphism::is_canonical)
            .map(Isomorphism::from)
            .collect::<Vec<Isomorphism>>()
            .into_par_iter()
            .map(|inner| (inner, self.lookup.projection(&inner)))
            .inspect(|_| progress.inc(1))
            .collect::<BTreeMap<Isomorphism, Histogram>>();
        progress.finish();
        IsomorphismSpace(projection)
    }

    /// initializes the centroids for k-means clustering using the k-means++ algorithm
    /// 1. choose 1st centroid randomly from the dataset
    /// 2. choose nth centroid with probability proportional to squared distance of nearest neighbors
    /// 3. collect histograms and label with arbitrary (random) `Abstraction`s
    fn kmeans_initial(&mut self) {
        log::info!(
            "{:<32}{:<32}",
            "declaring abstractions",
            format!("{}    {} clusters", self.street, Self::k(self.street))
        );
        let ref mut rng = rand::thread_rng();
        let progress = crate::progress(Self::k(self.street));
        let sample = self.sample_uniform(rng);
        self.kmeans.expand(sample);
        progress.inc(1);
        while self.kmeans.0.len() < Self::k(self.street) {
            let sample = self.sample_outlier(rng);
            self.kmeans.expand(sample);
            progress.inc(1);
        }
        progress.finish();
    }
    /// for however many iterations we want,
    /// 1. assign each `Observation` to the nearest `Centroid`
    /// 2. update each `Centroid` by averaging the `Observation`s assigned to it
    fn kmeans_cluster(&mut self) {
        log::info!(
            "{:<32}{:<32}",
            "clustering observations",
            format!("{}    {} iterations", self.street, Self::t(self.street))
        );
        let progress = crate::progress(Self::t(self.street));
        for _ in 0..Self::t(self.street) {
            let neighbors = self.get_neighbor();
            self.set_neighbor(neighbors);
            self.set_orphaned();
            progress.inc(1);
        }
        progress.finish();
    }

    /// find the nearest neighbor `Abstraction` to each `Observation`.
    /// work in parallel and collect results before mutating kmeans state.
    fn get_neighbor(&self) -> Vec<(Abstraction, f32)> {
        self.points
            .0
            .par_iter()
            .map(|(_, h)| self.nearest(h))
            .collect::<Vec<(Abstraction, f32)>>()
    }
    /// assign each `Observation` to the nearest `Centroid`
    /// by computing the EMD distance between the `Observation`'s `Histogram` and each `Centroid`'s `Histogram`
    /// and returning the `Abstraction` of the nearest `Centroid`
    fn set_neighbor(&mut self, neighbors: Vec<(Abstraction, f32)>) {
        self.kmeans.clear();
        let mut loss = 0.;
        for ((obs, hist), (abs, dist)) in self.points.0.iter_mut().zip(neighbors.iter()) {
            self.lookup.assign(abs, obs);
            self.kmeans.absorb(abs, hist);
            loss += dist * dist;
        }
        log::trace!("LOSS {:.6e}", loss / self.points.0.len() as f32);
    }
    /// centroid drift may make it such that some centroids are empty
    /// so we reinitialize empty centroids with random Observations if necessary
    fn set_orphaned(&mut self) {
        let ref mut rng = rand::thread_rng();
        for ref a in self.kmeans.orphans() {
            let ref sample = self.sample_uniform(rng);
            self.kmeans.absorb(a, sample);
            log::debug!(
                "{:<32}{:<32}",
                "reassigned empty centroid",
                format!("0x{}", a)
            );
        }
    }

    /// the first Centroid is uniformly random across all `Observation` `Histogram`s
    fn sample_uniform<R: Rng>(&self, rng: &mut R) -> Histogram {
        self.points
            .0
            .values()
            .choose(rng)
            .cloned()
            .expect("observation projections have been populated")
    }
    /// each next Centroid is selected with probability proportional to
    /// the squared distance to the nearest neighboring Centroid.
    /// faster convergence, i guess. on the shoulders of giants
    fn sample_outlier<R: Rng>(&self, rng: &mut R) -> Histogram {
        let weights = self
            .points
            .0
            .par_iter()
            .map(|(_obs, hist)| self.nearest(hist))
            .map(|(_abs, dist)| dist * dist)
            .collect::<Vec<f32>>();
        let index = WeightedIndex::new(weights)
            .expect("valid weights array")
            .sample(rng);
        self.points
            .0
            .values()
            .nth(index)
            .cloned()
            .expect("shared index with outer layer")
    }

    /// find the nearest neighbor `Abstraction` to a given `Histogram` for kmeans clustering
    fn nearest(&self, histogram: &Histogram) -> (Abstraction, f32) {
        self.kmeans
            .0
            .par_iter()
            .map(|(abs, centroid)| (abs, centroid.histogram()))
            .map(|(abs, centroid)| (abs, self.metric.emd(histogram, centroid)))
            .min_by(|(_, dx), (_, dy)| dx.partial_cmp(dy).unwrap())
            .map(|(abs, distance)| (abs.clone(), distance))
            .expect("find nearest neighbor")
    }

    /// number of kmeans centroids.
    /// this determines the granularity of the abstraction space.
    ///
    /// - CPU: O(N^2) for kmeans initialization
    /// - CPU: O(N)   for kmeans clustering
    /// - RAM: O(N^2) for learned metric
    /// - RAM: O(N)   for learned centroids
    const fn k(street: Street) -> usize {
        match street {
            Street::Pref => 0,
            Street::Flop => crate::KMEANS_FLOP_CLUSTER_COUNT,
            Street::Turn => crate::KMEANS_TURN_CLUSTER_COUNT,
            Street::Rive => unreachable!(),
        }
    }
    /// number of kmeans iterations.
    /// this controls the precision of the abstraction space.
    ///
    /// - CPU: O(N) for kmeans clustering
    const fn t(street: Street) -> usize {
        match street {
            Street::Pref => 0,
            Street::Flop => crate::KMEANS_FLOP_TRAINING_ITERATIONS,
            Street::Turn => crate::KMEANS_TURN_TRAINING_ITERATIONS,
            Street::Rive => unreachable!(),
        }
    }
}
