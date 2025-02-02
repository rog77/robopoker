use crate::cards::isomorphism::Isomorphism;
use crate::cards::observation::Observation;
use crate::cards::street::Street;
use crate::clustering::abstraction::Abstraction;
use crate::clustering::histogram::Histogram;
use crate::clustering::metric::Metric;
use crate::clustering::pair::Pair;
use crate::clustering::sinkhorn::Sinkhorn;
use crate::transport::coupling::Coupling;
use crate::Energy;
use crate::Probability;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_postgres::Client;
use tokio_postgres::Error as E;

pub struct API(Arc<Client>);

impl API {
    pub async fn new() -> Self {
        log::info!("connecting to db (API)");
        let (client, connection) = tokio_postgres::Config::default()
            .port(5432)
            .host("localhost")
            .user("postgres")
            .dbname("robopoker")
            .password("postgrespassword")
            .connect(tokio_postgres::NoTls)
            .await
            .expect("db connection");
        tokio::spawn(connection);
        Self(Arc::new(client))
    }

    // global lookups
    pub async fn encode(&self, obs: Observation) -> Result<Abstraction, E> {
        let iso = i64::from(Observation::from(Isomorphism::from(obs)));
        const SQL: &'static str = r#"
            SELECT abs
            FROM encoder
            WHERE obs = $1
        "#;
        Ok(self
            .0
            .query_one(SQL, &[&iso])
            .await?
            .get::<_, i64>(0)
            .into())
    }
    pub async fn metric(&self, street: Street) -> Result<Metric, E> {
        let street = street as i16;
        const SQL: &'static str = r#"
            SELECT
                a1.abs # a2.abs AS xor,
                m.dx            AS dx
            FROM abstraction a1
            JOIN abstraction a2
                ON a1.street = a2.street
            JOIN metric m
                ON (a1.abs # a2.abs) = m.xor
            WHERE
                a1.street   = $1 AND
                a1.abs     != a2.abs;
        "#;
        Ok(self
            .0
            .query(SQL, &[&street])
            .await?
            .iter()
            .map(|row| (row.get::<_, i64>(0), row.get::<_, Energy>(1)))
            .map(|(xor, distance)| (Pair::from(xor), distance))
            .collect::<BTreeMap<Pair, Energy>>()
            .into())
    }
    pub async fn basis(&self, street: Street) -> Result<Vec<Abstraction>, E> {
        let street = street as i16;
        const SQL: &'static str = r#"
            SELECT a2.abs
            FROM abstraction a2
            JOIN abstraction a1 ON a2.street = a1.street
            WHERE a1.abs = $1;
        "#;
        Ok(self
            .0
            .query(SQL, &[&street])
            .await?
            .iter()
            .map(|row| row.get::<_, i64>(0))
            .map(Abstraction::from)
            .collect())
    }

    // equity calculations
    pub async fn abs_equity(&self, abs: Abstraction) -> Result<Probability, E> {
        let iso = i64::from(abs);
        const SQL: &'static str = r#"
            SELECT equity
            FROM abstraction
            WHERE abs = $1
        "#;
        Ok(self
            .0
            .query_one(SQL, &[&iso])
            .await?
            .get::<_, f32>(0)
            .into())
    }
    pub async fn obs_equity(&self, obs: Observation) -> Result<Probability, E> {
        let iso = i64::from(Observation::from(Isomorphism::from(obs)));
        let sql = if obs.street() == Street::Rive {
            r#"
                SELECT equity
                FROM encoder
                WHERE obs = $1
            "#
        } else {
            r#"
                SELECT SUM(t.dx * a.equity)
                FROM transitions t
                JOIN encoder     e ON e.abs = t.prev
                JOIN abstraction a ON a.abs = t.next
                WHERE e.obs = $1
            "#
        };
        Ok(self
            .0
            .query_one(sql, &[&iso])
            .await?
            .get::<_, f32>(0)
            .into())
    }

    // distance calculations
    pub async fn abs_distance(&self, abs1: Abstraction, abs2: Abstraction) -> Result<Energy, E> {
        if abs1.street() != abs2.street() {
            return Err(E::__private_api_timeout());
        }
        if abs1 == abs2 {
            return Ok(0 as Energy);
        }
        let xor = i64::from(Pair::from((&abs1, &abs2)));
        const SQL: &'static str = r#"
            SELECT m.dx
            FROM metric m
            WHERE $1 = m.xor;
        "#;
        Ok(self.0.query_one(SQL, &[&xor]).await?.get::<_, Energy>(0))
    }
    pub async fn obs_distance(&self, obs1: Observation, obs2: Observation) -> Result<Energy, E> {
        // dob Kd8s~6dJsAc QhQs~QdQcAc
        if obs1.street() != obs2.street() {
            return Err(E::__private_api_timeout());
        }
        let (ref hx, ref hy, ref metric) = tokio::try_join!(
            self.obs_histogram(obs1),
            self.obs_histogram(obs2),
            self.metric(obs1.street().next())
        )?;
        Ok(Sinkhorn::from((hx, hy, metric)).minimize().cost())
    }

    // population lookups
    pub async fn abs_population(&self, abs: Abstraction) -> Result<usize, E> {
        let abs = i64::from(abs);
        const SQL: &'static str = r#"
            SELECT population
            FROM abstraction
            WHERE abs = $1
        "#;
        Ok(self.0.query_one(SQL, &[&abs]).await?.get::<_, i32>(0) as usize)
    }
    pub async fn obs_population(&self, obs: Observation) -> Result<usize, E> {
        let iso = i64::from(Observation::from(Isomorphism::from(obs)));
        const SQL: &'static str = r#"
            SELECT population
            FROM abstraction
            JOIN encoder ON encoder.abs = abstraction.abs
            WHERE obs = $1
        "#;
        Ok(self.0.query_one(SQL, &[&iso]).await?.get::<_, i64>(0) as usize)
    }

    // centrality (mean distance) lookups
    pub async fn abs_centrality(&self, abs: Abstraction) -> Result<Probability, E> {
        let abs = i64::from(abs);
        const SQL: &'static str = r#"
            SELECT centrality
            FROM abstraction
            WHERE abs = $1
        "#;
        Ok(self
            .0
            .query_one(SQL, &[&abs])
            .await?
            .get::<_, f32>(0)
            .into())
    }
    pub async fn obs_centrality(&self, obs: Observation) -> Result<Probability, E> {
        let iso = i64::from(Observation::from(Isomorphism::from(obs)));
        const SQL: &'static str = r#"
            SELECT centrality
            FROM abstraction
            JOIN encoder ON encoder.abs = abstraction.abs
            WHERE obs = $1
        "#;
        Ok(self
            .0
            .query_one(SQL, &[&iso])
            .await?
            .get::<_, f32>(0)
            .into())
    }

    // histogram aggregation via join
    pub async fn abs_histogram(&self, abs: Abstraction) -> Result<Histogram, E> {
        let idx = i64::from(abs);
        let mass = abs.street().n_children() as f32;
        const SQL: &'static str = r#"
            SELECT next, dx
            FROM transitions
            WHERE prev = $1
        "#;
        Ok(self
            .0
            .query(SQL, &[&idx])
            .await?
            .iter()
            .map(|row| (row.get::<_, i64>(0), row.get::<_, Energy>(1)))
            .map(|(next, dx)| (next, (dx * mass).round() as usize))
            .map(|(next, dx)| (Abstraction::from(next), dx))
            .fold(Histogram::default(), |mut h, (next, dx)| {
                h.set(next, dx);
                h
            }))
    }
    pub async fn obs_histogram(&self, obs: Observation) -> Result<Histogram, E> {
        // Kd8s~6dJsAc
        let idx = i64::from(Observation::from(Isomorphism::from(obs)));
        let mass = obs.street().n_children() as f32;
        const SQL: &'static str = r#"
            SELECT next, dx
            FROM transitions
            JOIN encoder ON encoder.abs = transitions.prev
            WHERE encoder.obs = $1
        "#;
        Ok(self
            .0
            .query(SQL, &[&idx])
            .await?
            .iter()
            .map(|row| (row.get::<_, i64>(0), row.get::<_, Energy>(1)))
            .map(|(next, dx)| (next, (dx * mass).round() as usize))
            .map(|(next, dx)| (Abstraction::from(next), dx))
            .fold(Histogram::default(), |mut h, (next, dx)| {
                h.set(next, dx);
                h
            }))
    }

    // observation similarity lookups
    pub async fn obs_similar(&self, obs: Observation) -> Result<Vec<Observation>, E> {
        // 8d8s~6dJs7c
        let iso = i64::from(Observation::from(Isomorphism::from(obs)));
        const SQL: &'static str = r#"
            SELECT obs
            FROM encoder
            WHERE abs = (
                SELECT abs
                FROM encoder
                WHERE obs = $1
            )
            AND obs != $1
            ORDER BY RANDOM()
            LIMIT 5;
        "#;
        Ok(self
            .0
            .query(SQL, &[&iso])
            .await?
            .iter()
            .map(|row| row.get::<_, i64>(0))
            .map(Observation::from)
            .collect())
    }
    pub async fn abs_similar(&self, abs: Abstraction) -> Result<Vec<Observation>, E> {
        let abs = i64::from(abs);
        const SQL: &'static str = r#"
            SELECT obs
            FROM encoder
            WHERE abs = $1
            ORDER BY RANDOM()
            LIMIT 5;
        "#;
        Ok(self
            .0
            .query(SQL, &[&abs])
            .await?
            .iter()
            .map(|row| row.get::<_, i64>(0))
            .map(Observation::from)
            .collect())
    }

    // proximity lookups
    pub async fn abs_nearby(&self, abs: Abstraction) -> Result<Vec<(Abstraction, Energy)>, E> {
        let abs = i64::from(abs);
        const SQL: &'static str = r#"
            SELECT a1.abs, m.dx
            FROM abstraction a1
            JOIN abstraction a2 ON a1.street = a2.street
            JOIN metric m ON (a1.abs # $1) = m.xor
            WHERE
                a2.abs  = $1 AND
                a1.abs != $1
            ORDER BY m.dx ASC
            LIMIT 5;
        "#;
        Ok(self
            .0
            .query(SQL, &[&abs])
            .await?
            .iter()
            .map(|row| (row.get::<_, i64>(0), row.get::<_, Energy>(1)))
            .map(|(abs, distance)| (Abstraction::from(abs), distance))
            .collect())
    }
    pub async fn obs_nearby(&self, obs: Observation) -> Result<Vec<(Abstraction, Energy)>, E> {
        let iso = i64::from(Observation::from(Isomorphism::from(obs)));
        const SQL: &'static str = r#"
            SELECT a1.abs, m.dx
            FROM encoder e
            JOIN abstraction a2 ON e.abs = a2.abs
            JOIN abstraction a1 ON a1.street = a2.street
            JOIN metric m ON (a1.abs # e.abs) = m.xor
            WHERE
                e.obs   = $1 AND
                a1.abs != e.abs
            ORDER BY m.dx ASC
            LIMIT 5;
        "#;
        Ok(self
            .0
            .query(SQL, &[&iso])
            .await?
            .iter()
            .map(|row| (row.get::<_, i64>(0), row.get::<_, Energy>(1)))
            .map(|(abs, distance)| (Abstraction::from(abs), distance))
            .collect())
    }
}

impl From<Client> for API {
    fn from(client: Client) -> Self {
        Self(Arc::new(client))
    }
}
