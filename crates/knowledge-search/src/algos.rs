use std::collections::{HashMap, HashSet};

#[derive(Debug)]
struct PageRank {
    graph: HashMap<String, HashSet<String>>,
    ranks: HashMap<String, f32>,
    damping_factor: f32,
    epsilon: f32,
}

impl PageRank {
    fn new(graph: HashMap<String, HashSet<String>>) -> Self {
        let node_count = graph.len() as f32;
        let damping_factor = 0.85;
        let epsilon = 0.0001;

        let rank = 1.0 / node_count;

        let ranks = graph
            .keys()
            .map(|k| (k.clone(), rank))
            .collect::<HashMap<_, _>>();

        PageRank {
            graph,
            ranks,
            damping_factor,
            epsilon,
        }
    }

    fn compute(&mut self) {
        let mut error = f32::INFINITY;
        let mut new_ranks = HashMap::new();

        while error > self.epsilon {
            new_ranks.clear();

            let dangling_nodes_sum = self.compute_dangling_nodes_sum();

            for (node_id, neighbors) in &self.graph {
                let mut rank = (1.0 - self.damping_factor) / self.graph.len() as f32;

                for neighbor_id in neighbors.iter() {
                    let neighbor_rank = self.ranks.get(neighbor_id).unwrap_or(&0.0);
                    let neighbor_outdegree =
                        self.graph.get(neighbor_id).map_or(0.0, |n| n.len() as f32);
                    rank += self.damping_factor * neighbor_rank / neighbor_outdegree;
                }

                rank += self.damping_factor * dangling_nodes_sum / self.graph.len() as f32;
                new_ranks.insert(node_id.clone(), rank);
            }

            let mut max_error = 0.0;
            for (node_id, rank) in &new_ranks {
                let old_rank = self.ranks.get(node_id).unwrap_or(&0.0);
                let error = (rank - old_rank).abs();

                if error > max_error {
                    max_error = error;
                }
            }

            error = max_error;
            self.ranks = new_ranks.clone();
        }
    }

    fn compute_dangling_nodes_sum(&self) -> f32 {
        let mut sum = 0.0;

        for (node_id, neighbors) in &self.graph {
            if neighbors.is_empty() {
                sum += self.ranks.get(node_id).unwrap_or(&0.0);
            }
        }

        sum
    }

    fn top_n(&self, n: usize) -> Vec<String> {
        let mut rank_vec: Vec<(String, f32)> =
            self.ranks.iter().map(|(k, v)| (k.clone(), *v)).collect();

        rank_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        rank_vec
            .iter()
            .take(n)
            .map(|(node_id, _)| node_id.clone())
            .collect()
    }
}
