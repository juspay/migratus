pub mod batch;
pub mod complete;
pub mod enrich;
pub mod merge;
pub mod migrate;
pub mod uninitialized;
pub mod validate;

pub trait State {}
pub trait TransitionTo<S: State> {}

pub struct Uninitialized;
pub struct MergeRequired;
pub struct MergeSkipped;
pub struct Validated;
pub struct Enriched;
pub struct Batched;
pub struct Migrated;
pub struct Completed;

impl State for Uninitialized {}
impl State for MergeRequired {}
impl State for MergeSkipped {}
impl State for Validated {}
impl State for Enriched {}
impl State for Batched {}
impl State for Migrated {}
impl State for Completed {}

impl TransitionTo<MergeRequired> for Uninitialized {}
impl TransitionTo<MergeSkipped> for Uninitialized {}
impl TransitionTo<Validated> for MergeRequired {}
impl TransitionTo<Validated> for MergeSkipped {}
impl TransitionTo<Enriched> for Validated {}
impl TransitionTo<Batched> for Enriched {}
impl TransitionTo<Migrated> for Batched {}
impl TransitionTo<Completed> for Migrated {}
