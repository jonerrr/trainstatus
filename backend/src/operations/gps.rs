//! Full-fidelity GPS history with bounded tails and immutable interval-index nodes.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex, Weak},
};

use anyhow::{Context, ensure};
use chrono::{DateTime, Utc};
use moka::sync::Cache;
use serde::{Deserialize, Serialize, ser::SerializeSeq};

use super::{history::GpsSample, identity::RunId};
use crate::storage::{
    blocks::{BlockStore, RecordRef},
    content::ContentId,
};

pub const PAGE_SAMPLES: usize = 256;
const NODE_ITEMS: usize = 64;
const MAX_HEIGHT: u8 = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GpsPage {
    first_us: i64,
    last_us: i64,
    record: RecordRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Summary {
    first_us: i64,
    last_us: i64,
    pages: u64,
    height: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Root {
    summary: Summary,
    record: RecordRef,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GpsSeries {
    root: Option<Arc<Root>>,
    tail: BTreeMap<i64, (f64, f64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GpsSeal {
    pub run: RunId,
    series: GpsSeries,
}

/// A borrowed history and the store that resolves its immutable page references.
pub struct GpsRead<'a> {
    pub series: &'a HashMap<RunId, Arc<GpsSeries>>,
    pub store: &'a GpsStore,
}

#[derive(Serialize, Deserialize)]
struct Sample(i64, f64, f64);

#[derive(Serialize, Deserialize)]
enum Node {
    Leaf(Vec<GpsPage>),
    Branch(Vec<Root>),
}

#[derive(Serialize)]
enum NodeRecord<'a> {
    Leaf(&'a [GpsPage]),
    Branch(ResolvedChildren<'a>),
}

#[derive(Serialize)]
struct RootRecord<'a> {
    summary: &'a Summary,
    record: &'a RecordRef,
}

struct ResolvedChildren<'a> {
    plan: &'a Plan,
    children: &'a [Link],
    locations: &'a [Option<RecordRef>],
}

impl Serialize for ResolvedChildren<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.children.len()))?;
        for child in self.children {
            let root = self
                .plan
                .reference(child, self.locations)
                .map_err(serde::ser::Error::custom)?;
            sequence.serialize_element(&root)?;
        }
        sequence.end()
    }
}

#[derive(Clone)]
pub struct GpsStore {
    blocks: BlockStore,
    nodes: Cache<RecordRef, Arc<Node>>,
    roots: Arc<Mutex<HashMap<usize, Weak<Root>>>>,
}

impl GpsStore {
    pub fn open(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self::from_blocks(BlockStore::open(root)?))
    }

    pub fn from_blocks(blocks: BlockStore) -> Self {
        Self {
            blocks,
            nodes: Cache::builder()
                .max_capacity(8 * 1024 * 1024)
                .weigher(|_: &RecordRef, node: &Arc<Node>| match node.as_ref() {
                    Node::Leaf(pages) => {
                        (128 + pages.capacity() * std::mem::size_of::<GpsPage>()) as u32
                    }
                    Node::Branch(children) => {
                        (128 + children.capacity() * std::mem::size_of::<Root>()) as u32
                    }
                })
                .build(),
            roots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn blocks(&self) -> &BlockStore {
        &self.blocks
    }

    /// Checkpoint/archive descriptors are sealed; validation does not read samples.
    pub fn validate_descriptor(&self, series: &GpsSeries) -> anyhow::Result<()> {
        ensure!(
            series.tail.is_empty(),
            "durable GPS descriptor has an unsealed tail"
        );
        if let Some(root) = &series.root {
            validate_summary(root.summary)?;
        }
        self.track_series(series);
        Ok(())
    }

    pub fn track_series(&self, series: &GpsSeries) {
        if let Some(root) = &series.root {
            self.roots
                .lock()
                .expect("GPS root registry lock")
                .insert(Arc::as_ptr(root) as usize, Arc::downgrade(root));
        }
    }

    /// A seal contains the bounded remainder as well as its new root. Replacing
    /// both reconciles replay after an uncertain checkpoint sealed a different tail.
    pub fn apply_seal(&self, series: &mut GpsSeries, seal: GpsSeal) {
        self.track_series(&seal.series);
        *series = seal.series;
    }

    pub fn live_blocks(
        &self,
        series: &GpsSeries,
        live: &mut HashSet<ContentId>,
    ) -> anyhow::Result<()> {
        if let Some(root) = &series.root {
            self.trace(root, live, &mut HashSet::new())?;
        }
        Ok(())
    }

    pub fn live_pinned_blocks(&self, live: &mut HashSet<ContentId>) -> anyhow::Result<()> {
        let roots: Vec<_> = {
            let mut registry = self.roots.lock().expect("GPS root registry lock");
            let roots = registry.values().filter_map(Weak::upgrade).collect();
            registry.retain(|_, root| root.strong_count() > 0);
            roots
        };
        let mut visited = HashSet::new();
        for root in roots {
            self.trace(&root, live, &mut visited)?;
        }
        Ok(())
    }

    fn trace(
        &self,
        root: &Root,
        live: &mut HashSet<ContentId>,
        visited: &mut HashSet<RecordRef>,
    ) -> anyhow::Result<()> {
        if !visited.insert(root.record.clone()) {
            return Ok(());
        }
        live.insert(root.record.block);
        match self.read_node(root)?.as_ref() {
            Node::Leaf(pages) => live.extend(pages.iter().map(|page| page.record.block)),
            Node::Branch(children) => {
                for child in children {
                    self.trace(child, live, visited)?;
                }
            }
        }
        Ok(())
    }

    pub fn contains(&self, series: &GpsSeries, time: DateTime<Utc>) -> anyhow::Result<bool> {
        let time = time.timestamp_micros();
        if series.tail.contains_key(&time) {
            return Ok(true);
        }
        self.visit_series(series, time, time, &mut |page| {
            Ok(self
                .read_page(page)?
                .binary_search_by_key(&time, |sample| sample.0)
                .is_ok())
        })
    }

    pub fn latest(
        &self,
        series: &GpsSeries,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Option<GpsSample>> {
        let end = at.timestamp_micros();
        let start = end.saturating_sub(300_000_000);
        let mut latest = series
            .tail
            .range(start..=end)
            .next_back()
            .map(|(&time, &(x, y))| Sample(time, x, y));
        self.visit_series(series, start, end, &mut |page| {
            if latest
                .as_ref()
                .is_some_and(|sample| page.last_us <= sample.0)
            {
                return Ok(false);
            }
            let samples = self.read_page(page)?;
            let index = samples.partition_point(|sample| sample.0 <= end);
            if let Some(sample) = index.checked_sub(1).and_then(|index| samples.get(index))
                && sample.0 >= start
                && latest.as_ref().is_none_or(|old| sample.0 > old.0)
            {
                latest = Some(Sample(sample.0, sample.1, sample.2));
            }
            Ok(false)
        })?;
        latest.map(Sample::into_observation).transpose()
    }

    /// Complete pages and all changed index nodes are durable before WAL append.
    pub fn prepare_seals(
        &self,
        current: &HashMap<RunId, Arc<GpsSeries>>,
        baselines: &HashMap<RunId, &GpsSeries>,
        accepted: &[(RunId, GpsSample)],
    ) -> anyhow::Result<Vec<GpsSeal>> {
        let mut tails: HashMap<RunId, BTreeMap<i64, (f64, f64)>> = HashMap::new();
        for (run, sample) in accepted {
            let tail = tails.entry(*run).or_insert_with(|| {
                current
                    .get(run)
                    .map(|series| series.tail.clone())
                    .or_else(|| baselines.get(run).map(|series| series.tail.clone()))
                    .unwrap_or_default()
            });
            tail.entry(sample.observed_at.timestamp_micros())
                .or_insert((sample.point.x(), sample.point.y()));
        }
        let mut runs: Vec<_> = tails.into_iter().collect();
        runs.sort_unstable_by_key(|(run, _)| *run);
        let mut records = Vec::new();
        let mut prepared = Vec::new();
        for (run, tail) in runs {
            let count = tail.len() / PAGE_SAMPLES;
            if count == 0 {
                continue;
            }
            let mut tail = tail.into_iter();
            let mut ranges = Vec::with_capacity(count);
            for _ in 0..count {
                let samples: Vec<_> = tail
                    .by_ref()
                    .take(PAGE_SAMPLES)
                    .map(|(t, (x, y))| Sample(t, x, y))
                    .collect();
                ranges.push((samples[0].0, samples[PAGE_SAMPLES - 1].0));
                records.push(samples);
            }
            let root = current
                .get(&run)
                .map(Arc::as_ref)
                .or_else(|| baselines.get(&run).copied())
                .and_then(|series| series.root.clone());
            prepared.push(PreparedRun {
                run,
                root,
                ranges,
                tail: tail.collect(),
            });
        }
        self.seal_records(records, prepared)
    }

    pub fn flush_tails(
        &self,
        current: &HashMap<RunId, Arc<GpsSeries>>,
    ) -> anyhow::Result<Vec<GpsSeal>> {
        let mut runs: Vec<_> = current
            .iter()
            .filter(|(_, series)| !series.tail.is_empty())
            .collect();
        runs.sort_unstable_by_key(|(run, _)| **run);
        let mut records = Vec::with_capacity(runs.len());
        let mut prepared = Vec::with_capacity(runs.len());
        for (run, series) in runs {
            ensure!(
                series.tail.len() < PAGE_SAMPLES,
                "GPS tail exceeds page capacity"
            );
            let samples: Vec<_> = series
                .tail
                .iter()
                .map(|(&time, &(x, y))| Sample(time, x, y))
                .collect();
            let first = samples.first().expect("nonempty tail").0;
            let last = samples.last().expect("nonempty tail").0;
            records.push(samples);
            prepared.push(PreparedRun {
                run: *run,
                root: series.root.clone(),
                ranges: vec![(first, last)],
                tail: BTreeMap::new(),
            });
        }
        self.seal_records(records, prepared)
    }

    fn seal_records(
        &self,
        records: Vec<Vec<Sample>>,
        prepared: Vec<PreparedRun>,
    ) -> anyhow::Result<Vec<GpsSeal>> {
        let mut locations = self.blocks.write(&records)?.into_iter();
        let mut plan = Plan::default();
        let mut roots = Vec::with_capacity(prepared.len());
        for prepared in prepared {
            let pages: Vec<_> = prepared
                .ranges
                .into_iter()
                .map(|(first_us, last_us)| GpsPage {
                    first_us,
                    last_us,
                    record: locations.next().expect("one record per GPS page"),
                })
                .collect();
            let mut links = self.update(prepared.root.as_deref(), pages, &mut plan)?;
            while links.len() > 1 {
                let mut parents = Vec::with_capacity(links.len().div_ceil(NODE_ITEMS));
                let mut children = links.into_iter();
                while children.len() > 0 {
                    parents.push(plan.branch(children.by_ref().take(NODE_ITEMS).collect())?);
                }
                links = parents;
            }
            roots.push((
                prepared.run,
                links.pop().expect("sealed pages produce a root"),
                prepared.tail,
            ));
        }
        let locations = self.persist_plan(&plan)?;
        roots
            .into_iter()
            .map(|(run, root, tail)| {
                Ok(GpsSeal {
                    run,
                    series: GpsSeries {
                        root: Some(Arc::new(plan.resolve(&root, &locations)?)),
                        tail,
                    },
                })
            })
            .collect()
    }

    fn update(
        &self,
        root: Option<&Root>,
        mut added: Vec<GpsPage>,
        plan: &mut Plan,
    ) -> anyhow::Result<Vec<Link>> {
        let Some(root) = root else {
            return plan.leaves(added);
        };
        if root.summary.height == 0
            && root.summary.pages == NODE_ITEMS as u64
            && added
                .first()
                .is_some_and(|page| page.first_us > root.summary.last_us)
        {
            let mut links = vec![Link::Stored(root.clone())];
            links.extend(plan.leaves(added)?);
            return Ok(links);
        }
        let node = self.read_node(root)?;
        let children = match node.as_ref() {
            Node::Leaf(old) => {
                added.extend_from_slice(old);
                added.sort_unstable_by_key(|page| page.first_us);
                ensure!(
                    added
                        .windows(2)
                        .all(|pair| pair[0].first_us < pair[1].first_us),
                    "duplicate GPS page start"
                );
                return plan.leaves(added);
            }
            Node::Branch(children) => children,
        };
        let mut next = Vec::new();
        let mut added = added.into_iter().peekable();
        for (index, child) in children.iter().enumerate() {
            let limit = children.get(index + 1).map(|child| child.summary.first_us);
            let mut changes = Vec::new();
            while added
                .peek()
                .is_some_and(|page| limit.is_none_or(|limit| page.first_us < limit))
            {
                changes.push(added.next().expect("peeked page"));
            }
            if changes.is_empty() {
                next.push(Link::Stored(child.clone()));
            } else {
                next.extend(self.update(Some(child), changes, plan)?);
            }
        }
        let mut links = Vec::with_capacity(next.len().div_ceil(NODE_ITEMS));
        let mut next = next.into_iter();
        while next.len() > 0 {
            links.push(plan.branch(next.by_ref().take(NODE_ITEMS).collect())?);
        }
        Ok(links)
    }

    fn persist_plan(&self, plan: &Plan) -> anyhow::Result<Vec<Option<RecordRef>>> {
        let mut levels: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        for (index, node) in plan.nodes.iter().enumerate() {
            levels.entry(node.summary.height).or_default().push(index);
        }
        let mut locations = vec![None; plan.nodes.len()];
        for indices in levels.into_values() {
            let records: Vec<_> = indices
                .iter()
                .map(|index| match &plan.nodes[*index].body {
                    Body::Leaf(pages) => NodeRecord::Leaf(pages),
                    Body::Branch(children) => NodeRecord::Branch(ResolvedChildren {
                        plan,
                        children,
                        locations: &locations,
                    }),
                })
                .collect();
            let written = self.blocks.write(&records)?;
            drop(records);
            for (index, location) in indices.into_iter().zip(written) {
                locations[index] = Some(location);
            }
        }
        Ok(locations)
    }

    pub fn window(
        &self,
        series: &GpsSeries,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> anyhow::Result<Vec<GpsSample>> {
        ensure!(start <= end, "invalid GPS window");
        self.samples(series, start.timestamp_micros(), end.timestamp_micros())
    }

    pub fn all(&self, series: &GpsSeries) -> anyhow::Result<Vec<GpsSample>> {
        self.samples(series, i64::MIN, i64::MAX)
    }

    fn samples(&self, series: &GpsSeries, start: i64, end: i64) -> anyhow::Result<Vec<GpsSample>> {
        let mut samples = BTreeMap::new();
        self.visit_series(series, start, end, &mut |page| {
            for sample in self.read_page(page)? {
                if sample.0 >= start && sample.0 <= end {
                    samples.entry(sample.0).or_insert((sample.1, sample.2));
                }
            }
            Ok(false)
        })?;
        for (&time, &point) in series.tail.range(start..=end) {
            samples.entry(time).or_insert(point);
        }
        samples
            .into_iter()
            .map(|(time, (x, y))| Sample(time, x, y).into_observation())
            .collect()
    }

    fn visit_series(
        &self,
        series: &GpsSeries,
        start: i64,
        end: i64,
        visit: &mut impl FnMut(&GpsPage) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool> {
        match &series.root {
            Some(root) => self.visit_node(root, start, end, visit),
            None => Ok(false),
        }
    }

    fn visit_node(
        &self,
        root: &Root,
        start: i64,
        end: i64,
        visit: &mut impl FnMut(&GpsPage) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool> {
        if root.summary.last_us < start || root.summary.first_us > end {
            return Ok(false);
        }
        match self.read_node(root)?.as_ref() {
            Node::Leaf(pages) => {
                for page in pages.iter().rev() {
                    if page.last_us >= start && page.first_us <= end && visit(page)? {
                        return Ok(true);
                    }
                }
            }
            Node::Branch(children) => {
                for child in children.iter().rev() {
                    if self.visit_node(child, start, end, visit)? {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn read_node(&self, root: &Root) -> anyhow::Result<Arc<Node>> {
        validate_summary(root.summary)?;
        let node = self
            .nodes
            .try_get_with(root.record.clone(), || {
                self.blocks.read::<Node>(&root.record).map(Arc::new)
            })
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let summary = match node.as_ref() {
            Node::Leaf(pages) => leaf_summary(pages)?,
            Node::Branch(children) => branch_summary(children.iter().map(|child| child.summary))?,
        };
        ensure!(summary == root.summary, "GPS index summary mismatch");
        Ok(node)
    }

    fn read_page(&self, page: &GpsPage) -> anyhow::Result<Vec<Sample>> {
        let samples: Vec<Sample> = self.blocks.read(&page.record)?;
        ensure!(
            !samples.is_empty() && samples.len() <= PAGE_SAMPLES,
            "invalid GPS page sample count"
        );
        ensure!(
            samples.first().map(|sample| sample.0) == Some(page.first_us)
                && samples.last().map(|sample| sample.0) == Some(page.last_us),
            "GPS page bounds mismatch"
        );
        ensure!(
            samples.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "GPS page timestamps are not strictly ordered"
        );
        Ok(samples)
    }
}

struct PreparedRun {
    run: RunId,
    root: Option<Arc<Root>>,
    ranges: Vec<(i64, i64)>,
    tail: BTreeMap<i64, (f64, f64)>,
}

#[derive(Clone)]
enum Link {
    Stored(Root),
    Pending(usize),
}
enum Body {
    Leaf(Vec<GpsPage>),
    Branch(Vec<Link>),
}
struct PlannedNode {
    summary: Summary,
    body: Body,
}
#[derive(Default)]
struct Plan {
    nodes: Vec<PlannedNode>,
}

impl Plan {
    fn summary(&self, link: &Link) -> Summary {
        match link {
            Link::Stored(root) => root.summary,
            Link::Pending(index) => self.nodes[*index].summary,
        }
    }

    fn reference<'a>(
        &'a self,
        link: &'a Link,
        locations: &'a [Option<RecordRef>],
    ) -> anyhow::Result<RootRecord<'a>> {
        match link {
            Link::Stored(root) => Ok(RootRecord {
                summary: &root.summary,
                record: &root.record,
            }),
            Link::Pending(index) => Ok(RootRecord {
                summary: &self.nodes[*index].summary,
                record: locations[*index]
                    .as_ref()
                    .context("GPS index child was not persisted")?,
            }),
        }
    }

    fn resolve(&self, link: &Link, locations: &[Option<RecordRef>]) -> anyhow::Result<Root> {
        match link {
            Link::Stored(root) => Ok(root.clone()),
            Link::Pending(index) => Ok(Root {
                summary: self.nodes[*index].summary,
                record: locations[*index]
                    .clone()
                    .context("GPS index child was not persisted")?,
            }),
        }
    }

    fn leaves(&mut self, pages: Vec<GpsPage>) -> anyhow::Result<Vec<Link>> {
        let mut pages = pages.into_iter();
        let mut links = Vec::with_capacity(pages.len().div_ceil(NODE_ITEMS));
        while pages.len() > 0 {
            let pages: Vec<_> = pages.by_ref().take(NODE_ITEMS).collect();
            let summary = leaf_summary(&pages)?;
            links.push(Link::Pending(self.nodes.len()));
            self.nodes.push(PlannedNode {
                summary,
                body: Body::Leaf(pages),
            });
        }
        Ok(links)
    }

    fn branch(&mut self, children: Vec<Link>) -> anyhow::Result<Link> {
        let summary = branch_summary(children.iter().map(|child| self.summary(child)))?;
        let link = Link::Pending(self.nodes.len());
        self.nodes.push(PlannedNode {
            summary,
            body: Body::Branch(children),
        });
        Ok(link)
    }
}

fn validate_summary(summary: Summary) -> anyhow::Result<()> {
    ensure!(
        summary.first_us <= summary.last_us && summary.pages > 0 && summary.height <= MAX_HEIGHT,
        "invalid GPS index bounds"
    );
    Ok(())
}

fn leaf_summary(pages: &[GpsPage]) -> anyhow::Result<Summary> {
    ensure!(
        !pages.is_empty() && pages.len() <= NODE_ITEMS,
        "invalid GPS index leaf size"
    );
    ensure!(
        pages.iter().all(|page| page.first_us <= page.last_us)
            && pages
                .windows(2)
                .all(|pair| pair[0].first_us < pair[1].first_us),
        "invalid GPS leaf ordering"
    );
    Ok(Summary {
        first_us: pages[0].first_us,
        last_us: pages
            .iter()
            .map(|page| page.last_us)
            .max()
            .expect("nonempty leaf"),
        pages: pages.len() as u64,
        height: 0,
    })
}

fn branch_summary(children: impl Iterator<Item = Summary>) -> anyhow::Result<Summary> {
    let mut children = children.peekable();
    let first = *children.peek().context("empty GPS index branch")?;
    let mut summary = Summary {
        first_us: first.first_us,
        last_us: first.last_us,
        pages: 0,
        height: first
            .height
            .checked_add(1)
            .context("GPS index height overflow")?,
    };
    let mut prior = None;
    let mut count = 0;
    for child in children {
        validate_summary(child)?;
        ensure!(
            child.height == first.height && prior.is_none_or(|prior| prior < child.first_us),
            "invalid GPS branch ordering"
        );
        prior = Some(child.first_us);
        summary.last_us = summary.last_us.max(child.last_us);
        summary.pages = summary
            .pages
            .checked_add(child.pages)
            .context("GPS page count overflow")?;
        count += 1;
    }
    ensure!(
        count <= NODE_ITEMS && summary.height <= MAX_HEIGHT,
        "GPS index branch exceeds bounds"
    );
    Ok(summary)
}

impl Sample {
    fn into_observation(self) -> anyhow::Result<GpsSample> {
        Ok(GpsSample {
            observed_at: DateTime::from_timestamp_micros(self.0)
                .context("GPS instant out of range")?,
            point: geo::Point::new(self.1, self.2),
        })
    }
}

impl GpsSeries {
    pub fn insert(&mut self, sample: GpsSample) {
        self.tail
            .entry(sample.observed_at.timestamp_micros())
            .or_insert((sample.point.x(), sample.point.y()));
    }
}

#[cfg(test)]
#[path = "tests/gps_index.rs"]
mod tests;
