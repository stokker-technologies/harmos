/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

/* === Modules === */

/* === Re-Exports === */

pub use harmos_domain::model::storage::query::transform::aggregate::{Aggregate, Measure};

/* === Imports === */

// Standard Library Imports
use std::cmp::Ordering;
use std::collections::HashMap;
use std::result;
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema};

// Local Imports
use super::FieldPath;
use super::{exact_type, invalid, numeric_type, scalar_type, values_array};
use crate::Result;
use crate::statistics::Accumulator;
use crate::transform::FieldPathExecution;
use harmos_domain::model::storage::query::scalar::{Agg, ScalarValue};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Groups {
    plan: Aggregate,
    schema: Arc<Schema>,
    groups: Vec<Group>,
    capture: u64,
}

struct Group {
    keys: Vec<ScalarValue>,
    accumulators: Vec<Option<Accumulator>>,
    ranks: Vec<(FieldPath, Vec<crate::Aggregate>, Accumulator)>,
}

/* === Enums === */

/* === Traits === */

pub trait AggregateExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn start(&self, input: &Schema) -> Result<Groups>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;
}

/* === Implementations === */

impl AggregateExecution for Aggregate {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.groups.len() > 64 || self.measures.is_empty() || self.measures.len() > 64 {
            return Err(invalid("aggregate requires at most 64 groups and 1..64 measures"));
        }
        let mut fields = Vec::new();
        for group in &self.groups {
            let field = group.field.field(input)?;
            if !scalar_type(field.data_type()) {
                return Err(invalid("aggregate grouping requires scalar keys"));
            }
            fields.push(field.with_name(&group.name).with_nullable(true));
        }
        for measure in &self.measures {
            let field = measure.field.field(input)?;
            if !scalar_type(field.data_type()) {
                return Err(invalid("aggregate requires scalar values"));
            }
            let statistic = measure.statistic;
            if !matches!(
                statistic,
                crate::Aggregate::Count | crate::Aggregate::Min | crate::Aggregate::Max | crate::Aggregate::First | crate::Aggregate::Last
            ) && !numeric_type(field.data_type())
            {
                return Err(invalid("numeric aggregate requires numeric field"));
            }
            if let crate::Aggregate::Percentile(rank) = statistic {
                // The shared vocabulary validates the rank through its normal codec.
                if rank > 100 {
                    return Err(invalid("invalid percentile rank"));
                }
            }
            let output = match statistic {
                crate::Aggregate::Count => DataType::UInt64,
                crate::Aggregate::Min | crate::Aggregate::Max | crate::Aggregate::First | crate::Aggregate::Last => {
                    field.data_type().clone()
                }
                crate::Aggregate::Sum | crate::Aggregate::Mean | crate::Aggregate::Median
                    if !matches!(field.data_type(), DataType::Float32 | DataType::Float64) =>
                {
                    exact_type()
                }
                _ => DataType::Float64,
            };
            let mut field = Field::new(&measure.name, output, !matches!(statistic, crate::Aggregate::Count));
            if field.data_type() == &exact_type() {
                field = field.with_metadata(HashMap::from([("harmos.logical_type".to_owned(), "exact-rational-v1".to_owned())]));
            }
            fields.push(field);
        }
        for field in &fields {
            if field.name().is_empty() || fields.iter().filter(|other| other.name() == field.name()).count() != 1 {
                return Err(invalid("aggregate output names must be nonempty and unique"));
            }
        }
        Ok(Schema::new(fields))
    }

    fn start(&self, input: &Schema) -> Result<Groups> {
        let schema = Arc::new(self.output_schema(input)?);
        let mut groups = Groups {
            plan: self.clone(),
            schema,
            groups: Vec::new(),
            capture: 0,
        };
        // An ungrouped reduction always yields its identity row, even for no input.
        if self.groups.is_empty() {
            groups.groups.push(new_group(self, Vec::new()));
        }
        Ok(groups)
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        let mut groups = self.start(input.schema().as_ref())?;
        groups.push(input, check)?;
        groups.finish(check)
    }
}

impl Groups {
    pub fn push(&mut self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        // Fold One Row At A Time Without A Page-Sized Scalar Copy.
        for row in 0..input.num_rows() {
            check()?;
            let keys = self.plan.groups.iter().map(|group| group.field.value(input, row).map(normalized)).collect::<Result<Vec<_>>>()?;
            let values =
                self.plan.measures.iter().map(|measure| measure.field.value(input, row).map(normalized)).collect::<Result<Vec<_>>>()?;
            let index = match self.find(&keys)? {
                Ok(index) => index,
                Err(index) => {
                    self.groups.insert(index, new_group(&self.plan, keys));
                    index
                }
            };
            let group = &mut self.groups[index];
            for (field, _, accumulator) in &mut group.ranks {
                let column = self.plan.measures.iter().position(|measure| &measure.field == field).unwrap();
                accumulator.push(self.capture, &values[column])?;
            }
            for (accumulator, value) in group.accumulators.iter_mut().zip(values) {
                if let Some(accumulator) = accumulator {
                    accumulator.push(self.capture, &value)?;
                }
            }
            self.capture = self.capture.checked_add(1).ok_or_else(|| invalid("query capture count overflow"))?;
        }
        Ok(())
    }

    pub fn finish(self, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        check()?;
        let width = self.schema.fields().len();
        let mut values = vec![Vec::with_capacity(self.groups.len()); width];
        for group in self.groups {
            check()?;
            for (column, key) in group.keys.into_iter().enumerate() {
                values[column].push(key);
            }
            let ranks = group
                .ranks
                .into_iter()
                .map(|(field, statistics, accumulator)| {
                    let values = accumulator.finish_ranks(&statistics, check)?;
                    Ok((field, statistics, values))
                })
                .collect::<Result<Vec<_>>>()?;
            for (column, accumulator) in group.accumulators.into_iter().enumerate() {
                let measure = &self.plan.measures[column];
                let value = if let Some(accumulator) = accumulator {
                    accumulator.finish(check)?
                } else {
                    let (_, statistics, values) = ranks.iter().find(|(field, _, _)| field == &measure.field).unwrap();
                    let rank = statistics.iter().position(|statistic| *statistic == measure.statistic).unwrap();
                    values[rank].clone()
                };
                // Preserve The Empty Sum In Its Floating Output Type.
                let value = match value {
                    ScalarValue::Unsigned(0)
                        if measure.statistic == crate::Aggregate::Sum
                            && self.schema.field(self.plan.groups.len() + column).data_type() == &DataType::Float64 =>
                    {
                        ScalarValue::Float(0.0)
                    }
                    value => value,
                };
                values[self.plan.groups.len() + column].push(value);
            }
        }
        let columns = self
            .schema
            .fields()
            .iter()
            .zip(values)
            .map(|(field, values)| values_array(field.data_type(), &values))
            .collect::<Result<Vec<_>>>()?;
        RecordBatch::try_new(self.schema, columns).map_err(|_| invalid("aggregate output batch is invalid"))
    }

    // Binary Search Over Groups Kept In Key Order.
    fn find(&self, keys: &[ScalarValue]) -> Result<result::Result<usize, usize>> {
        let (mut low, mut high) = (0usize, self.groups.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match compare_keys(&self.groups[middle].keys, keys)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(low))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn shared(statistic: crate::Aggregate) -> bool {
    matches!(statistic, crate::Aggregate::Median | crate::Aggregate::Percentile(_))
}

fn new_group(plan: &Aggregate, keys: Vec<ScalarValue>) -> Group {
    let mut ranks: Vec<(FieldPath, Vec<crate::Aggregate>, Accumulator)> = Vec::new();
    for measure in &plan.measures {
        if !shared(measure.statistic) {
            continue;
        }
        if let Some((_, statistics, _)) = ranks.iter_mut().find(|(field, _, _)| field == &measure.field) {
            statistics.push(measure.statistic);
        } else {
            ranks.push((measure.field.clone(), vec![measure.statistic], Accumulator::new(Agg::Of(measure.statistic))));
        }
    }
    Group {
        keys,
        accumulators: plan
            .measures
            .iter()
            .map(|measure| if shared(measure.statistic) { None } else { Some(Accumulator::new(Agg::Of(measure.statistic))) })
            .collect(),
        ranks,
    }
}

fn compare_keys(left: &[ScalarValue], right: &[ScalarValue]) -> Result<Ordering> {
    for (left, right) in left.iter().zip(right) {
        let comparison = left.compare(right)?;
        if !comparison.is_eq() {
            return Ok(comparison);
        }
    }
    Ok(left.len().cmp(&right.len()))
}

fn normalized(value: ScalarValue) -> ScalarValue {
    match value {
        ScalarValue::Float(number) if !number.is_finite() => ScalarValue::Null,
        value => value,
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::super::aggregate;
    use super::{AggregateExecution, Measure};
    use crate::Aggregate;
    use crate::scalar::ScalarValue;
    use crate::transform::{self, project::Projection};
    use arrow_array::{Array, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    #[test]
    fn aggregate_percentiles_share_one_field_spool_and_rank_passes() {
        let schema = Arc::new(Schema::new(vec![Field::new("v", DataType::Float64, true)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(Float64Array::from(vec![Some(9.0), None, Some(-1.0), Some(3.0), Some(f64::NAN)]))],
        )
        .unwrap();
        let run = |statistics: &[Aggregate]| {
            let plan = aggregate::Aggregate {
                groups: vec![],
                measures: statistics
                    .iter()
                    .enumerate()
                    .map(|(index, statistic)| Measure {
                        field: "v".into(),
                        statistic: *statistic,
                        name: format!("m{index}"),
                    })
                    .collect(),
            };
            let mut groups = plan.start(&schema).unwrap();
            groups.push(&batch, &mut || Ok(())).unwrap();
            assert_eq!(groups.groups[0].ranks.len(), 1);
            let mut passes = 0;
            let batch = groups
                .finish(&mut || {
                    passes += 1;
                    Ok(())
                })
                .unwrap();
            (batch, passes)
        };
        let statistics = [Aggregate::Median, Aggregate::Percentile(25), Aggregate::Percentile(50), Aggregate::Percentile(95)];
        let (shared, passes) = run(&statistics);
        for (column, statistic) in statistics.into_iter().enumerate() {
            let (single, single_passes) = run(&[statistic]);
            assert_eq!(shared.column(column), single.column(0));
            assert_eq!(passes, single_passes);
        }
    }

    #[test]
    fn integer_median_and_percentiles_share_one_typed_spool() {
        let schema = Arc::new(Schema::new(vec![Field::new("v", DataType::UInt64, true)]));
        let input = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(UInt64Array::from(vec![Some(9_007_199_254_740_993), None, Some(9_007_199_254_740_995)]))],
        )
        .unwrap();
        let plan = aggregate::Aggregate {
            groups: vec![],
            measures: [Aggregate::Median, Aggregate::Percentile(95)]
                .into_iter()
                .enumerate()
                .map(|(index, statistic)| Measure {
                    field: "v".into(),
                    statistic,
                    name: format!("m{index}"),
                })
                .collect(),
        };
        let mut groups = plan.start(&schema).unwrap();
        groups.push(&input, &mut || Ok(())).unwrap();
        assert_eq!(groups.groups[0].ranks.len(), 1);
        assert!(groups.groups[0].accumulators.iter().all(Option::is_none));
        let output = groups.finish(&mut || Ok(())).unwrap();
        let median = transform::scalar(output.column(0).as_ref(), 0).unwrap();
        let ScalarValue::Exact(median) = median else {
            panic!("integer aggregate median must use exact output");
        };
        assert_eq!(median.numerator, 9_007_199_254_740_994);
        assert_eq!(median.denominator.get(), 1);
        assert_eq!(output.column(1).as_any().downcast_ref::<Float64Array>().unwrap().value(0), 9_007_199_254_740_996.0);
    }

    #[test]
    fn ordered_aggregate_measures_keep_scalar_types_and_skip_nulls() {
        let schema = Arc::new(Schema::new(vec![Field::new("number", DataType::Int64, true), Field::new("text", DataType::Utf8, true)]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int64Array::from(vec![None, Some(7), Some(2), None])),
                Arc::new(StringArray::from(vec![None, Some("z"), Some("a"), None])),
            ],
        )
        .unwrap();
        let plan = aggregate::Aggregate {
            groups: vec![],
            measures: [("number", Aggregate::First), ("number", Aggregate::Last), ("text", Aggregate::First), ("text", Aggregate::Last)]
                .into_iter()
                .enumerate()
                .map(|(index, (field, statistic))| Measure {
                    field: field.into(),
                    statistic,
                    name: format!("m{index}"),
                })
                .collect(),
        };
        let output = plan.apply(&batch, &mut || Ok(())).unwrap();
        assert_eq!(output.column(0).as_any().downcast_ref::<Int64Array>().unwrap().value(0), 7);
        assert_eq!(output.column(1).as_any().downcast_ref::<Int64Array>().unwrap().value(0), 2);
        assert_eq!(output.column(2).as_any().downcast_ref::<StringArray>().unwrap().value(0), "z");
        assert_eq!(output.column(3).as_any().downcast_ref::<StringArray>().unwrap().value(0), "a");
        let empty = plan.apply(&RecordBatch::new_empty(batch.schema()), &mut || Ok(())).unwrap();
        assert!(empty.columns().iter().all(|column| column.is_null(0)));
    }
    #[test]
    fn aggregate_empty_input_has_typed_identities_and_null_statistics() {
        let statistics = [
            Aggregate::Count,
            Aggregate::Sum,
            Aggregate::Mean,
            Aggregate::Median,
            Aggregate::Percentile(95),
            Aggregate::Min,
            Aggregate::Max,
            Aggregate::Rms,
            Aggregate::Std,
            Aggregate::First,
            Aggregate::Last,
        ];
        for kind in [DataType::Float32, DataType::Float64, DataType::Int64, DataType::UInt64] {
            let schema = Arc::new(Schema::new(vec![Field::new("v", kind, true)]));
            let input = RecordBatch::new_empty(schema);
            let mut plan = aggregate::Aggregate {
                groups: vec![],
                measures: statistics
                    .into_iter()
                    .enumerate()
                    .map(|(column, statistic)| Measure {
                        field: "v".into(),
                        statistic,
                        name: format!("m{column}"),
                    })
                    .collect(),
            };
            let result = plan.apply(&input, &mut || Ok(())).unwrap();
            assert_eq!(result.num_rows(), 1);
            assert_eq!(result.column(0).as_any().downcast_ref::<UInt64Array>().unwrap().value(0), 0);
            let sum = transform::scalar(result.column(1).as_ref(), 0).unwrap();
            match sum {
                ScalarValue::Float(value) => assert_eq!(value, 0.0),
                ScalarValue::Exact(value) => assert_eq!(value.numerator, 0),
                _ => panic!("empty sum must retain a numeric identity"),
            }
            assert!(result.columns()[2..].iter().all(|column| column.is_null(0)));
            plan.groups.push(Projection { field: "v".into(), name: "group".into() });
            assert_eq!(plan.apply(&input, &mut || Ok(())).unwrap().num_rows(), 0);
        }
    }
}
