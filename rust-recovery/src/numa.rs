//! NUMA-aware memory allocation and thread pinning

use std::collections::HashMap;

#[cfg(target_os = "linux")]
use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};

/// NUMA Node information
#[derive(Debug, Clone)]
pub struct NumaNode {
    pub node_id: usize,
    pub cpu_cores: Vec<usize>,
    pub memory_size_mb: u64,
}

/// NUMA Topology
#[derive(Debug, Clone)]
pub struct NumaTopology {
    pub nodes: Vec<NumaNode>,
    pub total_cores: usize,
}

impl NumaTopology {
    /// Detect NUMA topology from /sys/devices/system/node/
    #[cfg(target_os = "linux")]
    pub fn detect() -> Option<Self> {
        use std::fs;
        use std::path::Path;
        
        let node_dir = Path::new("/sys/devices/system/node");
        if !node_dir.exists() {
            return None;
        }
        
        let mut nodes = Vec::new();
        let mut total_cores = 0;
        
        if let Ok(entries) = fs::read_dir(node_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_str().unwrap_or("");
                
                if name_str.starts_with("node") {
                    if let Some(node_id) = name_str.strip_prefix("node").and_then(|s| s.parse::<usize>().ok()) {
                        let cpu_list_path = entry.path().join("cpulist");
                        let cpu_list = fs::read_to_string(&cpu_list_path).ok()?;
                        
                        let cpu_cores = parse_cpu_list(&cpu_list);
                        total_cores += cpu_cores.len();
                        
                        // Попытка прочитать размер памяти
                        let meminfo_path = entry.path().join("meminfo");
                        let memory_size_mb = if let Ok(meminfo) = fs::read_to_string(&meminfo_path) {
                            parse_memory_size(&meminfo).unwrap_or(0)
                        } else {
                            0
                        };
                        
                        nodes.push(NumaNode {
                            node_id,
                            cpu_cores,
                            memory_size_mb,
                        });
                    }
                }
            }
        }
        
        if nodes.is_empty() {
            None
        } else {
            // Сортировка по node_id для предсказуемости
            nodes.sort_by_key(|n| n.node_id);
            Some(NumaTopology { nodes, total_cores })
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    pub fn detect() -> Option<Self> {
        None // NUMA detection только для Linux
    }
    
    /// Get NUMA node for specific CPU core
    pub fn node_for_cpu(&self, cpu: usize) -> Option<usize> {
        for node in &self.nodes {
            if node.cpu_cores.contains(&cpu) {
                return Some(node.node_id);
            }
        }
        None
    }
    
    /// Distribute work across NUMA nodes
    pub fn distribute_chunks(&self, total_chunks: usize) -> Vec<(usize, Vec<usize>)> {
        let mut distribution = Vec::new();
        if self.nodes.is_empty() {
             return vec![(0, (0..total_chunks).collect())];
        }

        let chunks_per_node = total_chunks / self.nodes.len();
        let remainder = total_chunks % self.nodes.len();
        
        let mut chunk_id = 0;
        for (i, node) in self.nodes.iter().enumerate() {
            let mut chunks = Vec::new();
            let count = chunks_per_node + if i < remainder { 1 } else { 0 };
            
            for _ in 0..count {
                if chunk_id < total_chunks {
                    chunks.push(chunk_id);
                    chunk_id += 1;
                }
            }
            
            distribution.push((node.node_id, chunks));
        }
        
        distribution
    }
}

/// Parse CPU list from sysfs (e.g., "0-3,8-11")
fn parse_cpu_list(cpu_list: &str) -> Vec<usize> {
    let mut cpus = Vec::new();
    
    for part in cpu_list.trim().split(',') {
        if part.is_empty() { continue; }
        if let Some((start, end)) = part.split_once('-') {
            if let (Ok(s), Ok(e)) = (start.parse::<usize>(), end.parse::<usize>()) {
                cpus.extend(s..=e);
            }
        } else if let Ok(cpu) = part.parse::<usize>() {
            cpus.push(cpu);
        }
    }
    
    cpus
}

/// Parse memory size from meminfo
fn parse_memory_size(meminfo: &str) -> Option<u64> {
    for line in meminfo.lines() {
        if line.starts_with("Node") && line.contains("MemTotal:") {
            // Format: "Node X MemTotal:       YYYYY kB"
            if let Some(kb_str) = line.split_whitespace().rev().nth(1) {
                if let Ok(kb) = kb_str.parse::<u64>() {
                    return Some(kb / 1024); // Convert to MB
                }
            }
        }
    }
    None
}

/// Pin thread to specific CPU core
#[cfg(target_os = "linux")]
pub fn pin_thread_to_cpu(cpu: usize) -> Result<(), std::io::Error> {
    unsafe {
        let mut cpu_set: cpu_set_t = std::mem::zeroed();
        CPU_ZERO(&mut cpu_set);
        CPU_SET(cpu, &mut cpu_set);
        
        let result = sched_setaffinity(
            0, // Current thread
            std::mem::size_of::<cpu_set_t>(),
            &cpu_set
        );
        
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn pin_thread_to_cpu(_cpu: usize) -> Result<(), std::io::Error> {
    Ok(()) // No-op на non-Linux
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_list() {
        assert_eq!(parse_cpu_list("0-3,8-11"), vec![0, 1, 2, 3, 8, 9, 10, 11]);
        assert_eq!(parse_cpu_list("0,2,4"), vec![0, 2, 4]);
    }
}
