use ::oxipng as oxi;
use core::time::Duration;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    fmt::Debug,
    hash::{Hash, Hasher},
    iter::Iterator,
    num::NonZeroU8,
};

// NOTE: Deprecated as of v9.1
// Should use sequence or set where appropriate
#[derive(FromPyObject)]
pub enum Collection<T: Eq + Hash> {
    #[pyo3(transparent, annotation = "list | tuple")]
    Seq(Vec<T>),
    #[pyo3(transparent, annotation = "set | frozenset")]
    Set(HashSet<T>),
}

pub enum CollectionIterator<T: Eq + Hash> {
    SeqIter(std::vec::IntoIter<T>),
    SetIter(std::collections::hash_set::IntoIter<T>),
}

impl<T: Eq + Hash> Iterator for CollectionIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            CollectionIterator::SeqIter(iter) => iter.next(),
            CollectionIterator::SetIter(iter) => iter.next(),
        }
    }
}

impl<T: Eq + Hash> IntoIterator for Collection<T> {
    type Item = T;
    type IntoIter = CollectionIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Collection::Seq(vec) => CollectionIterator::SeqIter(vec.into_iter()),
            Collection::Set(set) => {
                eprintln!(
                    "(pyoxipng) Deprecation Warning: Python sets will not be accepted arguments in a future release. Please use a list or tuple instead."
                );
                CollectionIterator::SetIter(set.into_iter())
            }
        }
    }
}

impl<T: Eq + Hash> Collection<T> {
    pub fn remap<U, C>(self) -> C
    where
        U: From<T>,
        C: FromIterator<U>,
    {
        self.into_iter().map(|i| i.into()).collect()
    }
}

// Filter
#[pyclass(eq, eq_int)]
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub enum RowFilter {
    NoOp = 0,
    Sub = 1,
    Up = 2,
    Average = 3,
    Paeth = 4,
    MinSum = 5,
    Entropy = 6,
    Bigrams = 7,
    BigEnt = 8,
    Brute = 9,
}

#[pymethods]
impl RowFilter {
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.clone() as u64).hash(&mut hasher);
        hasher.finish()
    }
}

impl From<RowFilter> for oxi::FilterStrategy {
    fn from(val: RowFilter) -> Self {
        use oxi::FilterStrategy;

        match val {
            RowFilter::NoOp => FilterStrategy::NONE,
            RowFilter::Sub => FilterStrategy::SUB,
            RowFilter::Up => FilterStrategy::UP,
            RowFilter::Average => FilterStrategy::AVERAGE,
            RowFilter::Paeth => FilterStrategy::PAETH,
            RowFilter::MinSum => FilterStrategy::MinSum,
            RowFilter::Entropy => FilterStrategy::Entropy,
            RowFilter::Bigrams => FilterStrategy::Bigrams,
            RowFilter::BigEnt => FilterStrategy::BigEnt,
            RowFilter::Brute => FilterStrategy::Brute {
                num_lines: 8,
                level: 5,
            },
        }
    }
}

// Interlacing
#[pyclass(eq, eq_int)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum Interlacing {
    Off = 0,
    Adam7 = 1,
}

#[pymethods]
impl Interlacing {
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<Interlacing> for bool {
    fn from(val: Interlacing) -> Self {
        matches!(val, Interlacing::Adam7)
    }
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct StripChunks(pub oxi::StripChunks);

#[pymethods]
impl StripChunks {
    #[staticmethod]
    fn none() -> Self {
        Self(oxi::StripChunks::None)
    }

    #[staticmethod]
    fn strip(val: Collection<[u8; 4]>) -> PyResult<Self> {
        Ok(Self(oxi::StripChunks::Strip(val.remap())))
    }

    #[staticmethod]
    fn safe() -> Self {
        Self(oxi::StripChunks::Safe)
    }

    #[staticmethod]
    fn keep(val: Collection<[u8; 4]>) -> PyResult<Self> {
        Ok(Self(oxi::StripChunks::Keep(val.remap())))
    }
    #[staticmethod]
    fn all() -> Self {
        Self(oxi::StripChunks::All)
    }
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct Deflaters(pub oxi::Deflater);

#[pymethods]
impl Deflaters {
    #[staticmethod]
    fn libdeflater(compression: u8) -> Self {
        Self(oxi::Deflater::Libdeflater { compression })
    }

    #[staticmethod]
    fn zopfli(iterations: u8) -> PyResult<Self> {
        if let Some(iterations) = NonZeroU8::new(iterations) {
            let mut options = oxi::ZopfliOptions::default();
            options.iteration_count = iterations.into();
            Ok(Self(oxi::Deflater::Zopfli(options)))
        } else {
            Err(PyTypeError::new_err(format!(
                "Invalid zopfli iterations {}; must be in range [1, 255]",
                iterations
            )))
        }
    }
}

/// Extract a python value that may be None
pub fn py_option<'py, 'a, T>(val: &'py Bound<'py, PyAny>) -> PyResult<Option<T>>
where
    T: FromPyObject<'a, 'py>,
    'py: 'a,
{
    if val.is_none() {
        Ok(None)
    } else {
        Ok(Some(val.extract().map_err(|_| {
            PyValueError::new_err("Value can't be extracted")
        })?))
    }
}

/// Extract a python float as a rust Duration type
pub fn py_duration(val: &Bound<'_, PyAny>) -> PyResult<Option<Duration>> {
    Ok(py_option::<f64>(val)?.and_then(|v| Some(Duration::from_millis((v * 1000.) as u64))))
}
