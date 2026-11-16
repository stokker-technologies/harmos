## Purpose

Keep every aggregation and statistics kernel in one Harmos home and expose it only through row-listing query transforms, including the expression and summary transforms products need.

## ADDED Requirements

### Requirement: Single Statistics Home

All aggregation, moment, quantile, histogram, box, bucket and downsampling kernels SHALL live in the query statistics module and be reachable by products only through row-listing transforms. RMS and standard deviation SHALL use floating-point population semantics.

#### Scenario: Product Requests Standard Deviation

- **WHEN** a product lists rows with a standard deviation aggregate over an integer field
- **THEN** the result is the floating-point population value computed by the shared kernel

### Requirement: Shared Multi-Percentile

A request for several percentiles of one field SHALL share one spool and one rank selection pass and return each percentile with the existing interpolation semantics.

#### Scenario: Several Percentiles

- **WHEN** a product requests the 5th, 50th and 95th percentiles of one field
- **THEN** the field is spooled once and all three values are returned in one result

### Requirement: Box Summaries

A box-summary transform SHALL return quartiles, Tukey whiskers and a bounded deterministic outlier selection per group.

#### Scenario: Outliers Exceed the Bound

- **WHEN** a group has more outliers than the requested bound
- **THEN** the result returns exactly the bound, chosen deterministically, and reports the total count

### Requirement: Mergeable Summaries

Summary results SHALL be mergeable and SHALL carry count, sum, extrema, first and last values with their time and position, and moments, so ordered first and last are aggregate measures.

#### Scenario: Merge Two Ranges

- **WHEN** summaries of two adjacent time ranges are merged
- **THEN** the merged summary equals the summary of the combined range

### Requirement: Bounded Bucket Summaries

A bucket-summary transform SHALL keep at most a requested number of time buckets, doubling width and merging adjacent pairs when exceeded, preserving first, last, minimum and maximum points with positions, count, mean and moments. It SHALL merge a supplied prior summary with new rows.

#### Scenario: Continue a Stored Summary

- **WHEN** a product supplies a stored bucket summary and its source cut and requests rows after that cut
- **THEN** the result equals a bucket summary computed over the whole range at the bound

### Requirement: Expression Transforms

Row transforms SHALL support scaling and offsetting numeric fields and testing string or list containment, with existing null and missing-value semantics.

#### Scenario: Scaled Field

- **WHEN** a product projects a field multiplied by a scale and shifted by an offset
- **THEN** each row carries the computed value and null inputs stay null
