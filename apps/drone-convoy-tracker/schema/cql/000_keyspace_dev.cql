-- =============================================================================
-- DRONE CONVOY TRACKING SYSTEM - Development Keyspace
-- For local Docker/single-node development
-- =============================================================================

CREATE KEYSPACE IF NOT EXISTS drone_ops
WITH replication = {
    'class': 'SimpleStrategy',
    'replication_factor': 1
}
AND durable_writes = true;
