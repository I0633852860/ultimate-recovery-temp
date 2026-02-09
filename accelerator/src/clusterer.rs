use pyo3::prelude::*;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Configuration for clustering
#[derive(Clone, Copy)]
struct ClusterConfig {
    similarity_threshold: f32, // Min cosine similarity (0.0 - 1.0)
    distance_decay_factor: f32, // k in exp(-k * delta_mb)
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.75, // Stricter to prevent "Frankenstein" merges
            distance_decay_factor: 10.0, // Stronger decay per 100MB to prefer local clusters
        }
    }
}

/// N-gram feature vector (256-dim byte frequency profile)
/// We use simple byte frequency for speed and effectiveness on text/binary distinction.
/// For "text vs text" (e.g. youtube vs tiktok), we might want bigrams, 
/// but let's start with byte frequency + specific keyword boosting if needed.
type FeatureVector = [f32; 256];

#[pyclass]
pub struct FragmentClusterer {
    fragments: Vec<RawFragment>,
    config: ClusterConfig,
}

#[derive(Clone)]
struct RawFragment {
    id: usize,
    offset: u64,
    size: u64,
    features: FeatureVector,
    links: Vec<String>,
    words: Option<HashSet<String>>, // Semantic words for text clustering
}

#[pymethods]
impl FragmentClusterer {
    #[new]
    fn new() -> Self {
        Self {
            fragments: Vec::new(),
            config: ClusterConfig::default(),
        }
    }

    /// Add a fragment to the pool
    fn add_fragment(&mut self, offset: u64, data: &[u8], links: Vec<String>) {
        let id = self.fragments.len();
        let features = Self::compute_features(data);
        let words = Self::extract_words(data);
        
        self.fragments.push(RawFragment {
            id,
            offset,
            size: data.len() as u64,
            features,
            links,
            words,
        });
    }

    /// Set configuration parameters
    fn set_threshold(&mut self, threshold: f32) {
        self.config.similarity_threshold = threshold;
    }

    fn set_distance_decay(&mut self, decay: f32) {
        self.config.distance_decay_factor = decay;
    }

    /// Main clustering function
    /// Returns a list of lists (clusters of fragment indices)
    fn cluster_fragments(&self, py: Python) -> PyResult<Vec<Vec<usize>>> {
        let n = self.fragments.len();
        if n == 0 {
            return Ok(Vec::new());
        }

        // Precompute word sets for text fragments to avoid re-parsing
        // Note: words are already extracted in add_fragment, this block is for debug prints
        let word_sets: Vec<&Option<HashSet<String>>> = self.fragments.iter().map(|f| {
            if let Some(ref words) = f.words {
                if f.id < 5 {
                     // Debug print for first few fragments
                     let sample: Vec<_> = words.iter().take(5).collect();
                     eprintln!("[RUST DEBUG] Fragment {}: extracted {} words: {:?}", f.id, words.len(), sample);
                }
            } else {
                eprintln!("[RUST DEBUG] Fragment {}: NO WORDS extracted", f.id);
            }
            &f.words
        }).collect();

        // 1. Calculate Affinity Matrix (Parallel)
        // We only compute upper triangle
        let edges: Vec<(usize, usize, f32)> = py.allow_threads(|| {
            (0..n).into_par_iter().flat_map(|i| {
                let mut local_edges = Vec::new();
                let f1 = &self.fragments[i];
                let w1 = word_sets[i]; // Use precomputed word set reference
                
                for j in (i + 1)..n {
                    let f2 = &self.fragments[j];
                    let w2 = word_sets[j]; // Use precomputed word set reference
                    
                    // 1. Physical Distance Decay
                    // Delta in MB
                    let delta_bytes = if f1.offset > f2.offset { f1.offset - f2.offset } else { f2.offset - f1.offset };
                    let delta_mb = delta_bytes as f32 / (1024.0 * 1024.0);
                    
                    // Decay factor: e^(-k * delta_mb / 100.0)
                    // Config factor is per 100MB
                    let dist_factor = (-self.config.distance_decay_factor * (delta_mb / 100.0)).exp();
                    
                    if dist_factor < 0.1 {
                        continue; 
                    }

                    // 2. Content Similarity
                    let sim_score;
                    
                    // Semantic Word Similarity (Text vs Text)
                    if let (Some(words1), Some(words2)) = (&f1.words, &f2.words) {
                        // Use word Jaccard if both are text
                        let word_sim = Self::jaccard_similarity_sets(words1, words2);
                        sim_score = word_sim;
                    } else {
                        // Binary or mixed: use Cosine of byte profile
                        sim_score = Self::cosine_similarity(&f1.features, &f2.features);
                    }
                    
                    // 3. Link Overlap (Jaccard) - Bonus
                    // If links overlap, it's definitely same.
                    let link_sim = if !f1.links.is_empty() && !f2.links.is_empty() {
                         Self::jaccard_similarity(&f1.links, &f2.links)
                    } else {
                        0.0
                    };

                    // Combined Score
                    let final_sim = if link_sim > 0.5 {
                         link_sim.max(sim_score)
                    } else {
                        sim_score
                    };

                    let final_score = final_sim * dist_factor;

                    if i < 2 && j < 5 {
                        eprintln!("[RUST DEBUG] Sim({}, {}): Content={:.3}, Link={:.3}, Dist={:.3} -> Final={:.3}", 
                            i, j, sim_score, link_sim, dist_factor, final_score);
                    }

                    if final_score >= self.config.similarity_threshold {
                        local_edges.push((i, j, final_score));
                    }
                }
                local_edges
            }).collect()
        });

        // 2. Build Clusters (Graph Traversal)
        // Simple connected components on the filtered graph
        let mut adj: HashMap<usize, Vec<usize>> = HashMap::with_capacity(n);
        for (i, j, _) in edges {
            adj.entry(i).or_default().push(j);
            adj.entry(j).or_default().push(i);
        }

        let mut visited = HashSet::new();
        let mut clusters = Vec::new();

        for i in 0..n {
            if !visited.contains(&i) {
                let mut cluster = Vec::new();
                let mut stack = vec![i];
                visited.insert(i);

                while let Some(node) = stack.pop() {
                    cluster.push(node);
                    if let Some(neighbors) = adj.get(&node) {
                        for &neighbor in neighbors {
                            if !visited.contains(&neighbor) {
                                visited.insert(neighbor);
                                stack.push(neighbor);
                            }
                        }
                    }
                }
                
                // Sort by offset for assembly
                cluster.sort_by_key(|&idx| self.fragments[idx].offset);
                clusters.push(cluster);
            }
        }

        Ok(clusters)
    }
}

impl FragmentClusterer {
    fn compute_features(data: &[u8]) -> FeatureVector {
        let mut counts = [0.0; 256];
        let mut total = 0.0;
        
        for &b in data {
            counts[b as usize] += 1.0;
            total += 1.0;
        }

        if total > 0.0 {
            for c in counts.iter_mut() {
                *c /= total;
            }
        }
        
        counts
    }

    fn extract_words(data: &[u8]) -> Option<HashSet<String>> {
        // Heuristic: check if mostly text
        let text_chars = data.iter().filter(|&&b| b >= 32 && b <= 126).count();
        if text_chars < data.len() / 2 {
            return None; // Mostly binary
        }
        
        let mut words = HashSet::new();
        let s = String::from_utf8_lossy(data);
        
        // Split by non-alphanumeric, filter for valid words
        for word in s.split(|c: char| !c.is_alphanumeric()) {
            if word.len() > 3 && word.chars().all(|c| c.is_ascii_alphabetic()) {
                words.insert(word.to_lowercase());
            }
        }
        
        if words.is_empty() { None } else { Some(words) }
    }

    fn cosine_similarity(v1: &FeatureVector, v2: &FeatureVector) -> f32 {
        // Dot product
        let mut dot = 0.0;
        let mut mag1 = 0.0;
        let mut mag2 = 0.0;

        // Auto-vectorized loop
        for i in 0..256 {
            dot += v1[i] * v2[i];
            mag1 += v1[i] * v1[i];
            mag2 += v2[i] * v2[i];
        }

        if mag1 == 0.0 || mag2 == 0.0 {
            0.0
        } else {
            dot / (mag1.sqrt() * mag2.sqrt())
        }
    }
    
    fn jaccard_similarity(links1: &[String], links2: &[String]) -> f32 {
         let s1: HashSet<&String> = links1.iter().collect();
         let s2: HashSet<&String> = links2.iter().collect();
         
         let intersection = s1.intersection(&s2).count();
         let union = s1.union(&s2).count();
         
         if union == 0 { 0.0 } else { intersection as f32 / union as f32 }
    }

    fn jaccard_similarity_sets(s1: &HashSet<String>, s2: &HashSet<String>) -> f32 {
         let intersection = s1.intersection(&s2).count();
         let union = s1.union(&s2).count();
         
         if union == 0 { 0.0 } else { intersection as f32 / union as f32 }
    }
}



