-- 002_waypoint_columns.cql
--
-- Upgrade migration: adds route-progress columns to the `drones` table for
-- databases created before 001_core_schema.cql carried them.
--
-- Fresh installs already get these columns from 001; on those, this ALTER
-- fails with "conflicts with an existing column" — which is why the Makefile
-- applies this file with `|| true`. CQL has no `ADD COLUMN IF NOT EXISTS`,
-- so tolerated failure IS the idempotency mechanism. Do not "fix" that.

USE drone_ops;

ALTER TABLE drones ADD (
    current_waypoint    smallint,
    total_waypoints     smallint
);
