// Intelligent Fragment Linker - Rust implementation
// Links scattered file fragments using similarity analysis
// V11.5: Military grade reliability and comprehensive testing

use pyo3::prelude::*;
use ahash::{HashMap, HashSet, HashMapExt, HashSetExt};
use rayon::prelude::*;

/// Fragment metadata for linking
#[derive(Clone, Debug)]
pub struct FragmentInfo {
    pub offset: u64,
    pub size: u64,
    pub file_type: String,
    pub links: HashSet<String>,
}

/// Calculate Jaccard similarity between two sets
fn jaccard_similarity(set1: &HashSet<String>, set2: &HashSet<String>) -> f32 {
    if set1.is_empty() || set2.is_empty() {
        return 0.0;
    }
    
    let intersection = set1.intersection(set2).count();
    let union = set1.union(set2).count();
    
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Fragment linker with similarity-based grouping
#[pyclass]
pub struct RustFragmentLinker {
    similarity_threshold: f32,
    fragments: Vec<FragmentInfo>,
}

#[pymethods]
impl RustFragmentLinker {
    #[new]
    #[pyo3(signature = (similarity_threshold = 0.3))]
    fn new(similarity_threshold: f32) -> Self {
        RustFragmentLinker {
            similarity_threshold,
            fragments: Vec::new(),
        }
    }
    
    fn add_fragment(&mut self, offset: u64, size: u64, file_type: String, links: Vec<String>) {
        let link_set: HashSet<String> = links.into_iter().collect();
        self.fragments.push(FragmentInfo {
            offset,
            size,
            file_type,
            links: link_set,
        });
    }
    
    fn find_related_groups(&self, py: Python) -> PyResult<Vec<PyObject>> {
        let n = self.fragments.len();
        if n == 0 { return Ok(Vec::new()); }
        
        let threshold = self.similarity_threshold;
        let fragments = &self.fragments;
        
        let edges: Vec<(usize, usize)> = py.allow_threads(|| {
            (0..n).into_par_iter()
                .flat_map(|i| {
                    let mut local_edges = Vec::new();
                    for j in (i + 1)..n {
                        if fragments[i].file_type == fragments[j].file_type {
                            let sim = jaccard_similarity(&fragments[i].links, &fragments[j].links);
                            if sim >= threshold {
                                local_edges.push((i, j));
                            }
                        }
                    }
                    local_edges
                })
                .collect()
        });
        
        let mut adj: HashMap<usize, Vec<usize>> = HashMap::with_capacity(n);
        for (i, j) in edges {
            adj.entry(i).or_default().push(j);
            adj.entry(j).or_default().push(i);
        }
        
        let mut visited: HashSet<usize> = HashSet::with_capacity(n);
        let mut groups: Vec<Vec<usize>> = Vec::new();
        
        for i in 0..n {
            if !visited.contains(&i) {
                let mut group = Vec::new();
                let mut stack = vec![i];
                while let Some(node) = stack.pop() {
                    if visited.insert(node) {
                        group.push(node);
                        if let Some(neighbors) = adj.get(&node) {
                            for &neighbor in neighbors {
                                if !visited.contains(&neighbor) {
                                    stack.push(neighbor);
                                }
                            }
                        }
                    }
                }
                if !group.is_empty() {
                    group.sort_by_key(|&idx| fragments[idx].offset);
                    groups.push(group);
                }
            }
        }
        
        let result: Vec<PyObject> = groups.iter().map(|group| {
            let py_group: Vec<PyObject> = group.iter().map(|&idx| {
                let frag = &fragments[idx];
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("offset", frag.offset).unwrap();
                dict.set_item("size", frag.size).unwrap();
                dict.set_item("file_type", &frag.file_type).unwrap();
                dict.to_object(py)
            }).collect();
            pyo3::types::PyList::new(py, py_group).to_object(py)
        }).collect();
        
        Ok(result)
    }

    fn clear(&mut self) {
        self.fragments.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard() {
        let mut s1 = HashSet::new();
        s1.insert("a".to_string());
        s1.insert("b".to_string());
        let mut s2 = HashSet::new();
        s2.insert("b".to_string());
        s2.insert("c".to_string());
        assert!((jaccard_similarity(&s1, &s2) - 0.333).abs() < 0.01);
        assert_eq!(jaccard_similarity(&s1, &s1), 1.0);
        assert_eq!(jaccard_similarity(&s1, &HashSet::new()), 0.0);
    }

    #[test]
    fn test_empty_sets() {
        let s1: HashSet<String> = HashSet::new();
        let s2: HashSet<String> = HashSet::new();
        assert_eq!(jaccard_similarity(&s1, &s2), 0.0);
    }
}
