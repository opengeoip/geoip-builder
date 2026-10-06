use std::collections::HashMap;

use analysis::coverage::AsCoverage;
use anyhow::Result;
use model::{AsName, Asn};

use crate::Log;
use crate::build::{Inputs, prepare};
use crate::io::in_memory;

pub struct Coverage {
    pub rows: Vec<AsCoverage>,
    pub names: HashMap<Asn, AsName>,
}

pub fn coverage(inputs: &Inputs<'_>, log: &mut Log<'_>) -> Result<Coverage> {
    let prepared = prepare(inputs, log)?;
    let (asn, _) = merge::asn_db(
        &prepared.selected,
        &prepared.names,
        prepared.policy,
        prepared.epoch,
    );
    let (located, _, _) = merge::location_dbs(
        Vec::new(),
        Vec::new(),
        vec![prepared.listed, prepared.anchored],
        prepared.epoch,
    );
    let rows = analysis::coverage::coverage(&in_memory(asn)?, &in_memory(located)?)?;
    Ok(Coverage {
        rows,
        names: prepared.names,
    })
}
