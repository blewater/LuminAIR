use rayon::iter::{IntoParallelIterator, ParallelIterator};
use stwo_prover::core::{
    backend::{
        simd::{
            m31::{PackedBaseField, LOG_N_LANES},
            SimdBackend,
        },
        Backend, CpuBackend,
    },
    fields::m31::BaseField,
};

pub trait TensorField: Clone + Send + Sync {
    fn zero() -> Self;
}

impl TensorField for BaseField {
    fn zero() -> Self {
        BaseField::from_u32_unchecked(0)
    }
}

impl TensorField for PackedBaseField {
    fn zero() -> Self {
        PackedBaseField::broadcast(BaseField::from_u32_unchecked(0))
    }
}

#[derive(Clone, Debug)]
pub struct AirTensor<F: TensorField> {
    pub data: Vec<F>,
    pub dims: Vec<usize>,
    pub stride: Vec<usize>,
}

impl<F: TensorField> AirTensor<F> {
    pub fn new(data: Vec<F>, dims: Vec<usize>) -> Self {
        let stride = Self::compute_stride(&dims);
        Self { data, dims, stride }
    }

    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    pub fn stride(&self) -> &[usize] {
        &self.stride
    }

    pub fn data(&self) -> &[F] {
        &self.data
    }

    pub fn compute_stride(dims: &[usize]) -> Vec<usize> {
        let mut stride = vec![1; dims.len()];
        for i in (0..dims.len() - 1).rev() {
            stride[i] = stride[i + 1] * dims[i + 1];
        }
        stride
    }

    pub fn size(&self) -> usize {
        self.dims.iter().product()
    }
}

pub trait TensorPacker {
    type Field: TensorField;

    fn pack_data(data: Vec<u32>, dims: &[usize]) -> Vec<Self::Field>;
}

impl TensorPacker for CpuBackend {
    type Field = BaseField;

    fn pack_data(data: Vec<u32>, _dims: &[usize]) -> Vec<Self::Field> {
        data.into_iter()
            .map(|x| BaseField::from_u32_unchecked(x % 1000))
            .collect()
    }
}

impl TensorPacker for SimdBackend {
    type Field = PackedBaseField;

    fn pack_data(data: Vec<u32>, dims: &[usize]) -> Vec<Self::Field> {
        let total_size = dims.iter().product::<usize>();
        let n_packed = (total_size + (1 << LOG_N_LANES) - 1) >> LOG_N_LANES;

        (0..n_packed)
            .into_par_iter()
            .map(|chunk_idx| {
                let start = chunk_idx << LOG_N_LANES;
                let mut lane_values = [0u32; 1 << LOG_N_LANES];

                for (i, lane) in lane_values.iter_mut().enumerate() {
                    let data_idx = start + i;
                    *lane = if data_idx < data.len() {
                        data[data_idx] % 1000
                    } else {
                        0
                    };
                }

                PackedBaseField::from_array(lane_values.map(|x| BaseField::from_u32_unchecked(x)))
            })
            .collect()
    }
}

// Helper function to create tensors for specific backends
impl<F: TensorField> AirTensor<F> {
    pub fn create<B: Backend + TensorPacker<Field = F>>(data: Vec<u32>, dims: Vec<usize>) -> Self {
        let packed_data = B::pack_data(data, &dims);
        Self::new(packed_data, dims)
    }
}
