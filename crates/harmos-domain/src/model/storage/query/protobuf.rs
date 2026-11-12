/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Convert Generated Query Selections and Transforms to Native Plans.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::contract::*;
use super::scalar::{ExactNumber, ScalarValue};
use super::transform::{self, FieldPath, Transform};
use crate::{Error, Identifier, Result};
use harmos_protocol::storage as wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl From<RowSlice> for wire::RowSlice {
    fn from(value: RowSlice) -> Self {
        Self { offset: value.offset, limit: value.limit }
    }
}
impl TryFrom<wire::RowSlice> for RowSlice {
    type Error = Error;
    fn try_from(value: wire::RowSlice) -> Result<Self> {
        Ok(Self { offset: value.offset, limit: value.limit })
    }
}
impl From<ObjectSelection> for wire::ObjectSelection {
    fn from(value: ObjectSelection) -> Self {
        Self {
            schema: value.schema.map(Into::into),
            key: value.key.map(|key| key.to_string()),
            versions: wire::ObjectVersions::from(value.versions) as i32,
        }
    }
}
impl TryFrom<wire::ObjectSelection> for ObjectSelection {
    type Error = Error;
    fn try_from(value: wire::ObjectSelection) -> Result<Self> {
        Ok(Self {
            schema: value.schema.map(TryInto::try_into).transpose()?,
            key: value.key.map(crate::model::storage::ObjectKey::from_owned).transpose()?,
            versions: value.versions.try_into()?,
        })
    }
}
impl From<RecordSelection> for wire::RecordSelection {
    fn from(value: RecordSelection) -> Self {
        Self {
            schema: Some(value.schema.into()),
            scope: value.scope.map(|id| id.to_string()),
            item: value.item.map(|id| id.to_string()),
            source_start: value.source_start.map(Into::into),
            source_end: value.source_end.map(Into::into),
        }
    }
}
impl TryFrom<wire::RecordSelection> for RecordSelection {
    type Error = Error;
    fn try_from(value: wire::RecordSelection) -> Result<Self> {
        Ok(Self {
            schema: value.schema.ok_or_else(|| invalid("schema is required"))?.try_into()?,
            scope: value.scope.map(Identifier::new).transpose()?,
            item: value.item.map(Identifier::new).transpose()?,
            source_start: value.source_start.map(|time| time.try_into().map_err(|_| invalid("invalid timestamp"))).transpose()?,
            source_end: value.source_end.map(|time| time.try_into().map_err(|_| invalid("invalid timestamp"))).transpose()?,
        })
    }
}
impl From<StreamSelection> for wire::StreamSelection {
    fn from(value: StreamSelection) -> Self {
        Self {
            schema: Some(value.schema.into()),
            scope: value.scope.map(|id| id.to_string()),
            item: value.item.map(|id| id.to_string()),
            stream: value.stream.map(|id| id.to_string()),
            source_start: value.source_start.map(Into::into),
            source_end: value.source_end.map(Into::into),
            after: value.after,
        }
    }
}
impl TryFrom<wire::StreamSelection> for StreamSelection {
    type Error = Error;
    fn try_from(value: wire::StreamSelection) -> Result<Self> {
        Ok(Self {
            schema: value.schema.ok_or_else(|| invalid("schema is required"))?.try_into()?,
            scope: value.scope.map(Identifier::new).transpose()?,
            item: value.item.map(Identifier::new).transpose()?,
            stream: value.stream.map(Identifier::new).transpose()?,
            source_start: value.source_start.map(|time| time.try_into().map_err(|_| invalid("invalid timestamp"))).transpose()?,
            source_end: value.source_end.map(|time| time.try_into().map_err(|_| invalid("invalid timestamp"))).transpose()?,
            after: value.after,
        })
    }
}
impl From<SchemaSelection> for wire::SchemaSelection {
    fn from(value: SchemaSelection) -> Self {
        Self { schema: value.schema.map(Into::into) }
    }
}
impl TryFrom<wire::SchemaSelection> for SchemaSelection {
    type Error = Error;
    fn try_from(value: wire::SchemaSelection) -> Result<Self> {
        Ok(Self {
            schema: value.schema.map(TryInto::try_into).transpose()?,
        })
    }
}
impl From<FileSelection> for wire::FileSelection {
    fn from(value: FileSelection) -> Self {
        Self {
            scope: value.scope.map(|id| id.to_string()),
            item: value.item.map(|id| id.to_string()),
        }
    }
}
impl TryFrom<wire::FileSelection> for FileSelection {
    type Error = Error;
    fn try_from(value: wire::FileSelection) -> Result<Self> {
        Ok(Self {
            scope: value.scope.map(Identifier::new).transpose()?,
            item: value.item.map(Identifier::new).transpose()?,
        })
    }
}
impl From<FieldPath> for wire::FieldPath {
    fn from(value: FieldPath) -> Self {
        Self { fields: value.fields }
    }
}
impl TryFrom<wire::FieldPath> for FieldPath {
    type Error = Error;
    fn try_from(value: wire::FieldPath) -> Result<Self> {
        Ok(Self { fields: value.fields })
    }
}
impl From<transform::filter::Filter> for wire::Filter {
    fn from(value: transform::filter::Filter) -> Self {
        Self {
            clauses: value.clauses.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<wire::Filter> for transform::filter::Filter {
    type Error = Error;
    fn try_from(value: wire::Filter) -> Result<Self> {
        Ok(Self {
            clauses: value.clauses.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
        })
    }
}
impl From<transform::filter::Clause> for wire::Clause {
    fn from(value: transform::filter::Clause) -> Self {
        Self {
            terms: value.terms.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<wire::Clause> for transform::filter::Clause {
    type Error = Error;
    fn try_from(value: wire::Clause) -> Result<Self> {
        Ok(Self {
            terms: value.terms.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
        })
    }
}
impl From<transform::filter::Term> for wire::Term {
    fn from(value: transform::filter::Term) -> Self {
        Self {
            field: Some(value.field.into()),
            condition: Some(value.condition.into()),
            negate: value.negate,
        }
    }
}
impl TryFrom<wire::Term> for transform::filter::Term {
    type Error = Error;
    fn try_from(value: wire::Term) -> Result<Self> {
        Ok(Self {
            field: value.field.ok_or_else(|| invalid("field is required"))?.try_into()?,
            condition: value.condition.ok_or_else(|| invalid("condition is required"))?.try_into()?,
            negate: value.negate,
        })
    }
}
impl From<transform::project::Project> for wire::Project {
    fn from(value: transform::project::Project) -> Self {
        Self {
            fields: value.fields.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<wire::Project> for transform::project::Project {
    type Error = Error;
    fn try_from(value: wire::Project) -> Result<Self> {
        Ok(Self {
            fields: value.fields.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
        })
    }
}
impl From<transform::project::Projection> for wire::Projection {
    fn from(value: transform::project::Projection) -> Self {
        Self {
            field: Some(value.field.into()),
            name: value.name,
        }
    }
}
impl TryFrom<wire::Projection> for transform::project::Projection {
    type Error = Error;
    fn try_from(value: wire::Projection) -> Result<Self> {
        Ok(Self {
            field: value.field.ok_or_else(|| invalid("field is required"))?.try_into()?,
            name: value.name,
        })
    }
}
impl From<transform::aggregate::Aggregate> for wire::Aggregate {
    fn from(value: transform::aggregate::Aggregate) -> Self {
        Self {
            groups: value.groups.into_iter().map(Into::into).collect(),
            measures: value.measures.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<wire::Aggregate> for transform::aggregate::Aggregate {
    type Error = Error;
    fn try_from(value: wire::Aggregate) -> Result<Self> {
        Ok(Self {
            groups: value.groups.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
            measures: value.measures.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
        })
    }
}
impl From<transform::aggregate::Measure> for wire::Measure {
    fn from(value: transform::aggregate::Measure) -> Self {
        Self {
            field: Some(value.field.into()),
            statistic: Some(value.statistic.into()),
            name: value.name,
        }
    }
}
impl TryFrom<wire::Measure> for transform::aggregate::Measure {
    type Error = Error;
    fn try_from(value: wire::Measure) -> Result<Self> {
        Ok(Self {
            field: value.field.ok_or_else(|| invalid("field is required"))?.try_into()?,
            statistic: value.statistic.ok_or_else(|| invalid("statistic is required"))?.try_into()?,
            name: value.name,
        })
    }
}
impl From<transform::histogram::Histogram> for wire::Histogram {
    fn from(value: transform::histogram::Histogram) -> Self {
        Self {
            field: Some(value.field.into()),
            edges: value.edges,
        }
    }
}
impl TryFrom<wire::Histogram> for transform::histogram::Histogram {
    type Error = Error;
    fn try_from(value: wire::Histogram) -> Result<Self> {
        Ok(Self {
            field: value.field.ok_or_else(|| invalid("field is required"))?.try_into()?,
            edges: value.edges,
        })
    }
}
impl From<transform::downsample::Downsample> for wire::Downsample {
    fn from(value: transform::downsample::Downsample) -> Self {
        Self {
            max_rows: value.max_rows as u64,
            algorithm: wire::DownsampleAlgorithm::from(value.algorithm) as i32,
            value: value.value.map(Into::into),
            axis: value.axis.map(Into::into),
        }
    }
}
impl TryFrom<wire::Downsample> for transform::downsample::Downsample {
    type Error = Error;
    fn try_from(value: wire::Downsample) -> Result<Self> {
        Ok(Self {
            max_rows: value.max_rows.try_into().map_err(|_| invalid("row count exceeds platform range"))?,
            algorithm: value.algorithm.try_into()?,
            value: value.value.map(TryInto::try_into).transpose()?,
            axis: value.axis.map(TryInto::try_into).transpose()?,
        })
    }
}
impl From<transform::sort::Sort> for wire::Sort {
    fn from(value: transform::sort::Sort) -> Self {
        Self {
            keys: value.keys.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<wire::Sort> for transform::sort::Sort {
    type Error = Error;
    fn try_from(value: wire::Sort) -> Result<Self> {
        Ok(Self {
            keys: value.keys.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
        })
    }
}
impl From<transform::sort::SortKey> for wire::SortKey {
    fn from(value: transform::sort::SortKey) -> Self {
        Self {
            field: Some(value.field.into()),
            descending: value.descending,
            nulls_first: value.nulls_first,
        }
    }
}
impl TryFrom<wire::SortKey> for transform::sort::SortKey {
    type Error = Error;
    fn try_from(value: wire::SortKey) -> Result<Self> {
        Ok(Self {
            field: value.field.ok_or_else(|| invalid("field is required"))?.try_into()?,
            descending: value.descending,
            nulls_first: value.nulls_first,
        })
    }
}
impl From<ObjectVersions> for wire::ObjectVersions {
    fn from(value: ObjectVersions) -> Self {
        match value {
            ObjectVersions::Latest => Self::Latest,
            ObjectVersions::Revisions => Self::Revisions,
        }
    }
}
impl TryFrom<i32> for ObjectVersions {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match wire::ObjectVersions::try_from(value) {
            Ok(wire::ObjectVersions::Latest) => Ok(Self::Latest),
            Ok(wire::ObjectVersions::Revisions) => Ok(Self::Revisions),
            Err(_) => Err(invalid("unknown ObjectVersions")),
        }
    }
}
impl From<transform::downsample::Algorithm> for wire::DownsampleAlgorithm {
    fn from(value: transform::downsample::Algorithm) -> Self {
        match value {
            transform::downsample::Algorithm::Decimate => Self::Decimate,
            transform::downsample::Algorithm::Extrema => Self::Extrema,
            transform::downsample::Algorithm::Lttb => Self::Lttb,
            transform::downsample::Algorithm::LttbLog => Self::LttbLog,
        }
    }
}
impl TryFrom<i32> for transform::downsample::Algorithm {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match wire::DownsampleAlgorithm::try_from(value) {
            Ok(wire::DownsampleAlgorithm::Decimate) => Ok(Self::Decimate),
            Ok(wire::DownsampleAlgorithm::Extrema) => Ok(Self::Extrema),
            Ok(wire::DownsampleAlgorithm::Lttb) => Ok(Self::Lttb),
            Ok(wire::DownsampleAlgorithm::LttbLog) => Ok(Self::LttbLog),
            Err(_) => Err(invalid("unknown DownsampleAlgorithm")),
        }
    }
}
impl From<transform::filter::Compare> for wire::Compare {
    fn from(value: transform::filter::Compare) -> Self {
        match value {
            transform::filter::Compare::Eq => Self::Eq,
            transform::filter::Compare::Ne => Self::Ne,
            transform::filter::Compare::Lt => Self::Lt,
            transform::filter::Compare::Le => Self::Le,
            transform::filter::Compare::Gt => Self::Gt,
            transform::filter::Compare::Ge => Self::Ge,
        }
    }
}
impl TryFrom<i32> for transform::filter::Compare {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match wire::Compare::try_from(value) {
            Ok(wire::Compare::Eq) => Ok(Self::Eq),
            Ok(wire::Compare::Ne) => Ok(Self::Ne),
            Ok(wire::Compare::Lt) => Ok(Self::Lt),
            Ok(wire::Compare::Le) => Ok(Self::Le),
            Ok(wire::Compare::Gt) => Ok(Self::Gt),
            Ok(wire::Compare::Ge) => Ok(Self::Ge),
            Err(_) => Err(invalid("unknown Compare")),
        }
    }
}
impl From<Source> for wire::QuerySource {
    fn from(value: Source) -> Self {
        use wire::query_source::Source as Variant;
        Self {
            source: Some(match value {
                Source::Objects(value) => Variant::Objects(value.into()),
                Source::Records(value) => Variant::Records(value.into()),
                Source::Stream(value) => Variant::Stream(value.into()),
                Source::Schemas(value) => Variant::Schemas(value.into()),
                Source::Files(value) => Variant::Files(value.into()),
            }),
        }
    }
}
impl TryFrom<wire::QuerySource> for Source {
    type Error = Error;
    fn try_from(value: wire::QuerySource) -> Result<Self> {
        use wire::query_source::Source as Variant;
        Ok(match value.source.ok_or_else(|| invalid("source is required"))? {
            Variant::Objects(value) => Self::Objects(value.try_into()?),
            Variant::Records(value) => Self::Records(value.try_into()?),
            Variant::Stream(value) => Self::Stream(value.try_into()?),
            Variant::Schemas(value) => Self::Schemas(value.try_into()?),
            Variant::Files(value) => Self::Files(value.try_into()?),
        })
    }
}
impl TryFrom<Transform> for wire::Transform {
    type Error = Error;
    fn try_from(value: Transform) -> Result<Self> {
        use wire::transform::Transform as Variant;
        Ok(Self {
            transform: Some(match value {
                Transform::Filter(value) => Variant::Filter(value.into()),
                Transform::Project(value) => Variant::Project(value.into()),
                Transform::Aggregate(value) => Variant::Aggregate(value.into()),
                Transform::Histogram(value) => Variant::Histogram(value.into()),
                Transform::Downsample(value) => Variant::Downsample(value.into()),
                Transform::Sort(value) => Variant::Sort(value.into()),
                Transform::TimeBuckets(value) => Variant::TimeBuckets(value.try_into()?),
            }),
        })
    }
}
impl TryFrom<wire::Transform> for Transform {
    type Error = Error;
    fn try_from(value: wire::Transform) -> Result<Self> {
        use wire::transform::Transform as Variant;
        Ok(match value.transform.ok_or_else(|| invalid("transform is required"))? {
            Variant::Filter(value) => Self::Filter(value.try_into()?),
            Variant::Project(value) => Self::Project(value.try_into()?),
            Variant::Aggregate(value) => Self::Aggregate(value.try_into()?),
            Variant::Histogram(value) => Self::Histogram(value.try_into()?),
            Variant::Downsample(value) => Self::Downsample(value.try_into()?),
            Variant::Sort(value) => Self::Sort(value.try_into()?),
            Variant::TimeBuckets(value) => Self::TimeBuckets(value.try_into()?),
        })
    }
}
impl From<QueryCompletion> for wire::QueryCompletion {
    fn from(value: QueryCompletion) -> Self {
        Self {
            at: Some(value.at.into()),
            rows: value.rows,
        }
    }
}
impl From<ScalarValue> for wire::ScalarValue {
    fn from(value: ScalarValue) -> Self {
        use wire::scalar_value::Value;
        Self {
            value: Some(match value {
                ScalarValue::Missing => Value::Missing(wire::Empty {}),
                ScalarValue::Null => Value::Null(wire::Empty {}),
                ScalarValue::Bool(value) => Value::Boolean(value),
                ScalarValue::Signed(value) => Value::Signed(value),
                ScalarValue::Unsigned(value) => Value::Unsigned(value),
                ScalarValue::Float(value) => Value::Floating(value),
                ScalarValue::Text(value) => Value::Text(value),
                ScalarValue::Datetime(value) => Value::Datetime(value.to_string()),
                ScalarValue::Exact(value) => Value::Exact(wire::ExactNumber {
                    negative: value.negative,
                    numerator: value.numerator.to_string(),
                    denominator: value.denominator.to_string(),
                }),
            }),
        }
    }
}
impl TryFrom<wire::ScalarValue> for ScalarValue {
    type Error = Error;
    fn try_from(value: wire::ScalarValue) -> Result<Self> {
        use wire::scalar_value::Value;
        let native = match value.value.ok_or_else(|| invalid("scalar value is required"))? {
            Value::Missing(_) => Self::Missing,
            Value::Null(_) => Self::Null,
            Value::Boolean(value) => Self::Bool(value),
            Value::Signed(value) => Self::Signed(value),
            Value::Unsigned(value) => Self::Unsigned(value),
            Value::Floating(value) => Self::Float(value),
            Value::Text(value) => Self::Text(value),
            Value::Datetime(value) => {
                let parsed: i128 = value.parse().map_err(|_| invalid("datetime exceeds signed 128-bit range"))?;
                if parsed.to_string() != value {
                    return Err(invalid("datetime requires minimal decimal notation"));
                }
                Self::Datetime(parsed)
            }
            Value::Exact(value) => {
                let numerator: u128 = value.numerator.parse().map_err(|_| invalid("invalid exact numerator"))?;
                let denominator: std::num::NonZeroU128 = value.denominator.parse().map_err(|_| invalid("invalid exact denominator"))?;
                if numerator.to_string() != value.numerator || denominator.to_string() != value.denominator {
                    return Err(invalid("exact numbers require minimal decimal notation"));
                }
                Self::Exact(ExactNumber {
                    negative: value.negative,
                    numerator,
                    denominator,
                })
            }
        };
        native.check()?;
        Ok(native)
    }
}
impl From<crate::Aggregate> for wire::Statistic {
    fn from(value: crate::Aggregate) -> Self {
        use wire::statistic::Statistic;
        Self {
            statistic: Some(match value {
                crate::Aggregate::Count => Statistic::Count(wire::Empty {}),
                crate::Aggregate::Sum => Statistic::Sum(wire::Empty {}),
                crate::Aggregate::Mean => Statistic::Mean(wire::Empty {}),
                crate::Aggregate::Median => Statistic::Median(wire::Empty {}),
                crate::Aggregate::Min => Statistic::Min(wire::Empty {}),
                crate::Aggregate::Max => Statistic::Max(wire::Empty {}),
                crate::Aggregate::Rms => Statistic::Rms(wire::Empty {}),
                crate::Aggregate::Std => Statistic::Std(wire::Empty {}),
                crate::Aggregate::Percentile(rank) => Statistic::Percentile(u32::from(rank)),
            }),
        }
    }
}
impl TryFrom<wire::Statistic> for crate::Aggregate {
    type Error = Error;
    fn try_from(value: wire::Statistic) -> Result<Self> {
        use wire::statistic::Statistic;
        Ok(match value.statistic.ok_or_else(|| invalid("statistic is required"))? {
            Statistic::Count(_) => Self::Count,
            Statistic::Sum(_) => Self::Sum,
            Statistic::Mean(_) => Self::Mean,
            Statistic::Median(_) => Self::Median,
            Statistic::Min(_) => Self::Min,
            Statistic::Max(_) => Self::Max,
            Statistic::Rms(_) => Self::Rms,
            Statistic::Std(_) => Self::Std,
            Statistic::Percentile(rank) if rank <= 100 => Self::Percentile(rank as u8),
            Statistic::Percentile(_) => return Err(invalid("percentile must be at most 100")),
        })
    }
}
impl From<transform::filter::Condition> for wire::Condition {
    fn from(value: transform::filter::Condition) -> Self {
        use wire::condition::Condition;
        Self {
            condition: Some(match value {
                transform::filter::Condition::Compare { op, value } => Condition::Compare(wire::Comparison {
                    op: wire::Compare::from(op) as i32,
                    value: Some(value.into()),
                }),
                transform::filter::Condition::In { values } => Condition::InValues(wire::ScalarValues {
                    values: values.into_iter().map(Into::into).collect(),
                }),
                transform::filter::Condition::IsNull => Condition::IsNull(wire::Empty {}),
            }),
        }
    }
}
impl TryFrom<wire::Condition> for transform::filter::Condition {
    type Error = Error;
    fn try_from(value: wire::Condition) -> Result<Self> {
        use wire::condition::Condition;
        Ok(match value.condition.ok_or_else(|| invalid("condition is required"))? {
            Condition::Compare(value) => Self::Compare {
                op: value.op.try_into()?,
                value: value.value.ok_or_else(|| invalid("comparison value is required"))?.try_into()?,
            },
            Condition::InValues(value) => Self::In {
                values: value.values.into_iter().map(TryInto::try_into).collect::<Result<_>>()?,
            },
            Condition::IsNull(_) => Self::IsNull,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage query", reason }
}

/* === Tests === */

impl TryFrom<wire::TimeBucketPlan> for transform::TimeBucketPlan {
    type Error = Error;
    fn try_from(value: wire::TimeBucketPlan) -> Result<Self> {
        Ok(Self {
            origin_ns: value.origin_ns,
            width_ns: std::num::NonZeroU64::new(value.width_ns).ok_or_else(|| invalid("bucket width must be nonzero"))?,
            time: value.time,
            value: value.value,
            identity: value.identity,
            group: value.group,
            max_samples: usize::try_from(value.max_samples)
                .ok()
                .and_then(std::num::NonZeroUsize::new)
                .ok_or_else(|| invalid("invalid sample limit"))?,
            max_bytes: usize::try_from(value.max_bytes)
                .ok()
                .and_then(std::num::NonZeroUsize::new)
                .ok_or_else(|| invalid("invalid byte limit"))?,
        })
    }
}
impl TryFrom<transform::TimeBucketPlan> for wire::TimeBucketPlan {
    type Error = Error;
    fn try_from(value: transform::TimeBucketPlan) -> Result<Self> {
        Ok(Self {
            origin_ns: value.origin_ns,
            width_ns: value.width_ns.get(),
            time: value.time,
            value: value.value,
            identity: value.identity,
            group: value.group,
            max_samples: value.max_samples.get().try_into().map_err(|_| invalid("sample limit exceeds wire range"))?,
            max_bytes: value.max_bytes.get().try_into().map_err(|_| invalid("byte limit exceeds wire range"))?,
        })
    }
}
