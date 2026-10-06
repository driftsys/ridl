# Design check calibration

Precision bar: 80%. Floor: 50%. Fewer than ten retained findings permits Info at most. Recall is reported and does not select or gate thresholds. Zero findings have undefined precision.

## inconsistent-unit

Finding workspaces: mavlink.

| Candidate threshold | Findings | Accepted | Precision | Detected applicable issues | Total applicable issues | Recall | Level |
| --- | --- | --- | --- | --- | --- | --- | --- |
| none | 7 | 7 | 100.00% | 0 | 0 | not applicable | Info |

Selected level: Info. Threshold: none. Retained findings: 7. Small sample: fewer than ten findings.

## inconsistent-abbreviation

Finding workspaces: mavlink, ros2.

| Candidate threshold | Findings | Accepted | Precision | Detected applicable issues | Total applicable issues | Recall | Level |
| --- | --- | --- | --- | --- | --- | --- | --- |
| none | 780 | 0 | 0.00% | 0 | 0 | not applicable | Dropped |

Selected level: not a lint. No candidate qualifies.

## duplicate-shape

Finding workspaces: mavlink, ros2, vss.

| Candidate threshold | Findings | Accepted | Precision | Detected applicable issues | Total applicable issues | Recall | Level |
| --- | --- | --- | --- | --- | --- | --- | --- |
| fields >= 2, variants >= 2 | 16 | 8 | 50.00% | 3 | 3 | 100.00% | Info |
| fields >= 2, variants >= 4 | 9 | 1 | 11.11% | 2 | 3 | 66.67% | Dropped |
| fields >= 3, variants >= 2 | 13 | 8 | 61.54% | 3 | 3 | 100.00% | Info |
| fields >= 3, variants >= 4 | 6 | 1 | 16.67% | 2 | 3 | 66.67% | Dropped |
| fields >= 4, variants >= 2 | 8 | 8 | 100.00% | 2 | 3 | 66.67% | Info |
| fields >= 4, variants >= 4 | 1 | 1 | 100.00% | 1 | 3 | 33.33% | Info |
| fields >= 5, variants >= 2 | 7 | 7 | 100.00% | 1 | 3 | 33.33% | Info |
| fields >= 5, variants >= 4 | 0 | 0 | undefined | 0 | 3 | 0.00% | Dropped |

Selected level: Info. Threshold: fields >= 2, variants >= 2. Retained findings: 16.

## low-cohesion-interface

Finding workspaces: mavlink, ros2, vss.

| Candidate threshold | Findings | Accepted | Precision | Detected applicable issues | Total applicable issues | Recall | Level |
| --- | --- | --- | --- | --- | --- | --- | --- |
| groups >= 2, min group size >= 1 | 16 | 1 | 6.25% | 1 | 1 | 100.00% | Dropped |
| groups >= 2, min group size >= 2 | 8 | 0 | 0.00% | 0 | 1 | 0.00% | Dropped |
| groups >= 2, min group size >= 3 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 4, min group size >= 1 | 5 | 1 | 20.00% | 1 | 1 | 100.00% | Dropped |
| groups >= 4, min group size >= 2 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 4, min group size >= 3 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 5, min group size >= 1 | 3 | 1 | 33.33% | 1 | 1 | 100.00% | Dropped |
| groups >= 5, min group size >= 2 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 5, min group size >= 3 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 7, min group size >= 1 | 2 | 1 | 50.00% | 1 | 1 | 100.00% | Info |
| groups >= 7, min group size >= 2 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 7, min group size >= 3 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 9, min group size >= 1 | 1 | 1 | 100.00% | 1 | 1 | 100.00% | Info |
| groups >= 9, min group size >= 2 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 9, min group size >= 3 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 18, min group size >= 1 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 18, min group size >= 2 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |
| groups >= 18, min group size >= 3 | 0 | 0 | undefined | 0 | 1 | 0.00% | Dropped |

Selected level: Info. Threshold: groups >= 7, min group size >= 1. Retained findings: 2. Small sample: fewer than ten findings.

## package-fan-out

Finding workspaces: ros2.

| Candidate threshold | Findings | Accepted | Precision | Detected applicable issues | Total applicable issues | Recall | Level |
| --- | --- | --- | --- | --- | --- | --- | --- |
| fan-out > 3 | 1 | 0 | 0.00% | 0 | 0 | not applicable | Dropped |
| fan-out > 4 | 0 | 0 | undefined | 0 | 0 | not applicable | Dropped |

Selected level: not a lint. No candidate qualifies.

