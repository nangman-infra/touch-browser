use touch_browser_contracts::{SnapshotDocument, SourceRisk};
use touch_browser_observation::ObservationInput;

use crate::{FixtureCatalog, ReadOnlyRuntime, RuntimeError};

impl ReadOnlyRuntime {
    pub(crate) fn load_snapshot(
        &self,
        catalog: &FixtureCatalog,
        target_url: &str,
    ) -> Result<(SnapshotDocument, SourceRisk, Option<String>), RuntimeError> {
        let document = catalog
            .get(target_url)
            .ok_or_else(|| RuntimeError::UnknownSource(target_url.to_string()))?;

        let snapshot = self
            .observation
            .compile(&ObservationInput::new(
                document.source_url.clone(),
                document.source_type.clone(),
                document.html.clone(),
                512,
            ))
            .map_err(RuntimeError::Observation)?;

        Ok((
            snapshot,
            document.source_risk.clone(),
            document.source_label.clone(),
        ))
    }
}
