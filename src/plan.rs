use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
pub struct Plan {
    pub schema_version: u32,
    pub plan_id: String,
    #[serde(default)]
    pub phases: Vec<Phase>,
}

#[derive(Debug, Deserialize)]
pub struct Phase {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl Plan {
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        if self.schema_version != 1 {
            return Err(crate::error::Error::UnsupportedSchema(self.schema_version));
        }
        if self.plan_id.is_empty() {
            return Err(crate::error::Error::EmptyPlanId);
        }
        if self.phases.is_empty() {
            return Err(crate::error::Error::ZeroPhases);
        }

        let mut seen_ids: HashSet<&str> = HashSet::new();

        for phase in &self.phases {
            if phase.id.is_empty() {
                return Err(crate::error::Error::EmptyPhaseId);
            }
            if phase.title.is_empty() {
                return Err(crate::error::Error::EmptyPhaseTitle);
            }
            if !seen_ids.insert(&phase.id) {
                return Err(crate::error::Error::DuplicatePhaseId(phase.id.clone()));
            }
        }

        for phase in &self.phases {
            for dep in &phase.depends_on {
                if dep == &phase.id {
                    return Err(crate::error::Error::SelfDependency(phase.id.clone()));
                }
                if !seen_ids.contains(dep.as_str()) {
                    return Err(crate::error::Error::UnknownDependency(
                        dep.clone(),
                        phase.id.clone(),
                    ));
                }
            }
        }

        self.check_cycles()?;

        Ok(())
    }

    fn check_cycles(&self) -> Result<(), crate::error::Error> {
        let idx: HashMap<&str, usize> = self
            .phases
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.as_str(), i))
            .collect();

        let mut visited = vec![0u8; self.phases.len()];

        fn dfs(phases: &[Phase], idx: &HashMap<&str, usize>, visited: &mut [u8], i: usize) -> bool {
            match visited[i] {
                1 => return true,
                2 => return false,
                _ => {}
            }
            visited[i] = 1;
            for dep in &phases[i].depends_on {
                if let Some(&j) = idx.get(dep.as_str()) {
                    if dfs(phases, idx, visited, j) {
                        return true;
                    }
                }
            }
            visited[i] = 2;
            false
        }

        for i in 0..self.phases.len() {
            if dfs(&self.phases, &idx, &mut visited, i) {
                return Err(crate::error::Error::DependencyCycle);
            }
        }

        Ok(())
    }
}
