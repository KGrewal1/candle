#![allow(unused)]
//! Wrappers around the Python API of Gymnasium (the new version of OpenAI gym)
use candle::{Device, Result, Tensor};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// The return value for a step.
#[derive(Debug)]
pub struct Step<A> {
    pub obs: Tensor,
    pub action: A,
    pub reward: f64,
    pub is_done: bool,
}

impl<A: Copy> Step<A> {
    /// Returns a copy of this step changing the observation tensor.
    pub fn copy_with_obs(&self, obs: &Tensor) -> Step<A> {
        Step {
            obs: obs.clone(),
            action: self.action,
            reward: self.reward,
            is_done: self.is_done,
        }
    }
}

/// An OpenAI Gym session.
pub struct GymEnv {
    env: PyObject,
    action_space: usize,
    observation_space: Vec<usize>,
}

fn w(res: PyErr) -> candle::Error {
    candle::Error::wrap(res)
}

impl GymEnv {
    /// Creates a new session of the specified OpenAI Gym environment.
    pub fn new(name: &str) -> Result<GymEnv> {
        Python::with_gil(|py| {
            let gym = py.import("gymnasium")?;
            let make = gym.getattr("make")?;
            let env = make.call1((name,))?;
            let action_space = env.getattr("action_space")?;
            let action_space = if let Ok(val) = action_space.getattr("n") {
                val.extract()?
            } else {
                let action_space: Vec<usize> = action_space.getattr("shape")?.extract()?;
                action_space[0]
            };
            let observation_space = env.getattr("observation_space")?;
            let observation_space = observation_space.getattr("shape")?.extract()?;
            Ok(GymEnv {
                env: env.into(),
                action_space,
                observation_space,
            })
        })
        .map_err(w)
    }

    /// Resets the environment, returning the observation tensor.
    pub fn reset(&self, seed: u64) -> Result<Tensor> {
        let obs: Vec<f32> = Python::with_gil(|py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("seed", seed)?;
            let obs = self.env.call_method(py, "reset", (), Some(kwargs))?;
            obs.as_ref(py).get_item(0)?.extract()
        })
        .map_err(w)?;
        Tensor::new(obs, &Device::Cpu)
    }

    /// Applies an environment step using the specified action.
    pub fn step<A: pyo3::IntoPy<pyo3::Py<pyo3::PyAny>> + Clone>(
        &self,
        action: A,
    ) -> Result<Step<A>> {
        let (obs, reward, is_done) = Python::with_gil(|py| {
            let step = self.env.call_method(py, "step", (action.clone(),), None)?;
            let step = step.as_ref(py);
            let obs: Vec<f32> = step.get_item(0)?.extract()?;
            let reward: f64 = step.get_item(1)?.extract()?;
            let is_done: bool = step.get_item(2)?.extract()?;
            Ok((obs, reward, is_done))
        })
        .map_err(w)?;
        let obs = Tensor::new(obs, &Device::Cpu)?;
        Ok(Step {
            obs,
            reward,
            is_done,
            action,
        })
    }

    /// Returns the number of allowed actions for this environment.
    pub fn action_space(&self) -> usize {
        self.action_space
    }

    /// Returns the shape of the observation tensors.
    pub fn observation_space(&self) -> &[usize] {
        &self.observation_space
    }
}
