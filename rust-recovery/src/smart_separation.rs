#[derive(Clone, Debug, PartialEq)]
pub struct ByteFrequency {
    pub values: [f32; 256],
}

impl ByteFrequency {
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut values = [0f32; 256];
        if data.is_empty() {
            return Self { values };
        }

        for &byte in data {
            values[byte as usize] += 1.0;
        }

        let len = data.len() as f32;
        for value in values.iter_mut() {
            *value /= len;
        }

        Self { values }
    }

    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        let mut dot = 0.0;
        let mut norm_self = 0.0;
        let mut norm_other = 0.0;

        for i in 0..256 {
            dot += self.values[i] * other.values[i];
            norm_self += self.values[i] * self.values[i];
            norm_other += other.values[i] * other.values[i];
        }

        if norm_self == 0.0 || norm_other == 0.0 {
            return 0.0;
        }

        dot / (norm_self.sqrt() * norm_other.sqrt())
    }
}

pub struct SmartSeparation;

impl SmartSeparation {
    pub fn feature_vector(data: &[u8]) -> ByteFrequency {
        ByteFrequency::from_bytes(data)
    }

    pub fn cosine_similarity(a: &ByteFrequency, b: &ByteFrequency) -> f32 {
        a.cosine_similarity(b)
    }

    pub fn similarity(a: &[u8], b: &[u8]) -> f32 {
        let vec_a = ByteFrequency::from_bytes(a);
        let vec_b = ByteFrequency::from_bytes(b);
        vec_a.cosine_similarity(&vec_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_vector_normalization() {
        let vector = ByteFrequency::from_bytes(&[0, 0, 1, 1]);
        assert!((vector.values[0] - 0.5).abs() < 1e-6);
        assert!((vector.values[1] - 0.5).abs() < 1e-6);
        let sum: f32 = vector.values.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let data = b"aaaaaa";
        let vec_a = ByteFrequency::from_bytes(data);
        let vec_b = ByteFrequency::from_bytes(data);
        let similarity = vec_a.cosine_similarity(&vec_b);
        assert!((similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_different() {
        let vec_a = ByteFrequency::from_bytes(&[0, 0, 0]);
        let vec_b = ByteFrequency::from_bytes(&[1, 1, 1]);
        let similarity = vec_a.cosine_similarity(&vec_b);
        assert!((similarity - 0.0).abs() < 1e-6);
    }
}
