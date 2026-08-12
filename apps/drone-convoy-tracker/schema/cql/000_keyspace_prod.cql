-- =============================================================================
-- DRONE CONVOY TRACKING SYSTEM - Production Keyspace
-- Multi-datacenter replication for high availability
-- =============================================================================

CREATE KEYSPACE IF NOT EXISTS drone_ops
WITH replication = {
    'class': 'NetworkTopologyStrategy',
    'afghanistan-dc1': 3,
    'qatar-dc2': 2
}
AND durable_writes = true;
