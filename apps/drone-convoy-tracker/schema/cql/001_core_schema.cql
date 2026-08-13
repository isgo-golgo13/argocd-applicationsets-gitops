-- =============================================================================
-- DRONE CONVOY TRACKING SYSTEM - ScyllaDB Schema
-- Classification: UNCLASSIFIED // FOR OFFICIAL USE ONLY
-- Version: 1.0.0
-- Author: CHANGEME-ORG
-- =============================================================================
-- Design Principles:
--   1. Partition keys optimized for convoy-level and drone-level queries
--   2. Time-bucketed partitions for telemetry (avoid unbounded growth)
--   3. Clustering columns enable efficient range scans
--   4. Materialized views for leaderboard aggregations
--   5. UDTs for complex nested structures (coordinates, weapon systems)
-- =============================================================================
-- NOTE: Keyspace must be created first using:
--   - 000_keyspace_dev.cql  (local development)
--   - 000_keyspace_prod.cql (production deployment)
-- =============================================================================

USE drone_ops;

-- -----------------------------------------------------------------------------
-- USER DEFINED TYPES (UDTs)
-- -----------------------------------------------------------------------------

-- Geographic coordinates with precision
CREATE TYPE IF NOT EXISTS coordinates (
    latitude     double,
    longitude    double,
    altitude_m   double,
    heading_deg  float,
    speed_mps    float
);

-- Weapon system status
CREATE TYPE IF NOT EXISTS weapon_status (
    weapon_type      text,          -- 'AGM-114_HELLFIRE', 'GBU-12_PAVEWAY', 'AIM-9X_SIDEWINDER'
    rounds_remaining smallint,
    status           text           -- 'ARMED', 'SAFE', 'JAMMED', 'EXPENDED'
);

-- Target acquisition data
CREATE TYPE IF NOT EXISTS target_info (
    target_id        uuid,
    target_type      text,          -- 'VEHICLE', 'STRUCTURE', 'PERSONNEL', 'RADAR'
    coordinates      frozen<coordinates>,
    confidence       float,         -- 0.0 - 1.0
    threat_level     text           -- 'HIGH', 'MEDIUM', 'LOW'
);

-- Engagement result
CREATE TYPE IF NOT EXISTS engagement_result (
    impact_time      timestamp,
    impact_coords    frozen<coordinates>,
    damage_assessment text,         -- 'DESTROYED', 'DAMAGED', 'MISSED', 'PENDING_BDA'
    collateral_risk  text           -- 'NONE', 'MINIMAL', 'MODERATE', 'HIGH'
);

-- Sensor payload status
CREATE TYPE IF NOT EXISTS sensor_status (
    sensor_type      text,          -- 'EO_IR', 'SAR', 'SIGINT', 'LIDAR'
    operational      boolean,
    mode             text           -- 'ACTIVE', 'PASSIVE', 'STANDBY'
);

-- Communication link status
CREATE TYPE IF NOT EXISTS comm_link (
    link_type        text,          -- 'SATCOM', 'LOS', 'MESH', 'BACKUP'
    signal_strength  float,         -- dBm
    latency_ms       int,
    encryption       text           -- 'AES256', 'TYPE1'
);

-- -----------------------------------------------------------------------------
-- CORE TABLES
-- -----------------------------------------------------------------------------

-- CONVOYS: Mission-level grouping of drones
-- Partition: convoy_id (single partition per convoy - small, bounded)
CREATE TABLE IF NOT EXISTS convoys (
    convoy_id           uuid,
    convoy_callsign     text,
    mission_id          uuid,
    mission_type        text,           -- 'ISR', 'STRIKE', 'ESCORT', 'RESUPPLY'
    status              text,           -- 'PLANNING', 'ACTIVE', 'RTB', 'COMPLETE', 'ABORT'
    
    -- Temporal
    created_at          timestamp,
    mission_start       timestamp,
    mission_end         timestamp,
    
    -- AOR (Area of Responsibility)
    aor_name            text,
    aor_center          frozen<coordinates>,
    aor_radius_km       float,
    
    -- Command
    commanding_unit     text,
    authorization_level text,           -- 'TACTICAL', 'OPERATIONAL', 'STRATEGIC'
    roe_profile         text,           -- Rules of Engagement profile
    
    -- Drone roster (denormalized for fast lookup)
    drone_ids           set<uuid>,
    drone_count         smallint,
    
    PRIMARY KEY (convoy_id)
) WITH comment = 'Mission-level convoy metadata'
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'LeveledCompactionStrategy'};

-- Secondary index for mission lookups
CREATE INDEX IF NOT EXISTS convoys_by_mission ON convoys (mission_id);
CREATE INDEX IF NOT EXISTS convoys_by_status ON convoys (status);


-- DRONES: Individual drone platform data
-- Partition: convoy_id (co-locate drones in same convoy)
-- Clustering: drone_id
CREATE TABLE IF NOT EXISTS drones (
    convoy_id           uuid,
    drone_id            uuid,
    
    -- Platform identification
    tail_number         text,
    callsign            text,
    platform_type       text,           -- 'MQ-9_REAPER', 'MQ-1C_GRAY_EAGLE', 'RQ-4_GLOBAL_HAWK'
    serial_number       text,
    
    -- Current state
    status              text,           -- 'PREFLIGHT', 'AIRBORNE', 'LOITER', 'INGRESS', 'EGRESS', 'RTB', 'LANDED'
    current_position    frozen<coordinates>,
    fuel_remaining_pct  float,
    flight_time_hrs     float,
    
    -- Loadout
    weapons             list<frozen<weapon_status>>,
    sensors             list<frozen<sensor_status>>,
    
    -- Communications
    primary_link        frozen<comm_link>,
    backup_link         frozen<comm_link>,
    mesh_neighbors      set<uuid>,       -- Adjacent drones in mesh
    
    -- Performance metrics (denormalized for leaderboard)
    total_engagements   int,
    successful_hits     int,
    accuracy_pct        float,           -- Computed: successful_hits / total_engagements * 100
    
    -- Route progress (denormalized from waypoints for the dashboard cards)
    current_waypoint    smallint,
    total_waypoints     smallint,
    
    -- Metadata
    created_at          timestamp,
    updated_at          timestamp,
    
    PRIMARY KEY (convoy_id, drone_id)
) WITH comment = 'Individual drone platform data partitioned by convoy'
   AND CLUSTERING ORDER BY (drone_id ASC)
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'LeveledCompactionStrategy'};


-- WAYPOINTS: Pre-planned route waypoints (25 per drone)
-- Partition: drone_id
-- Clustering: sequence_number
CREATE TABLE IF NOT EXISTS waypoints (
    drone_id            uuid,
    sequence_number     smallint,       -- 1-25
    
    -- Waypoint data
    waypoint_id         uuid,
    waypoint_name       text,           -- 'ALPHA', 'BRAVO', etc.
    waypoint_type       text,           -- 'NAV', 'LOITER', 'STRIKE', 'REFUEL', 'RENDEZVOUS'
    
    -- Location
    coordinates         frozen<coordinates>,
    
    -- Timing
    planned_arrival     timestamp,
    actual_arrival      timestamp,
    planned_departure   timestamp,
    actual_departure    timestamp,
    
    -- Actions at waypoint
    loiter_duration_min int,
    authorized_actions  set<text>,      -- 'ENGAGE', 'OBSERVE', 'RELAY', 'REFUEL'
    
    -- Status
    status              text,           -- 'PENDING', 'ACTIVE', 'COMPLETE', 'SKIPPED'
    
    PRIMARY KEY (drone_id, sequence_number)
) WITH comment = 'Pre-planned route waypoints per drone'
   AND CLUSTERING ORDER BY (sequence_number ASC)
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'LeveledCompactionStrategy'};


-- TELEMETRY: Time-series sensor/position data
-- Partition: (drone_id, time_bucket) - bucketed by hour to bound partition size
-- Clustering: recorded_at DESC (most recent first)
-- Expected write rate: ~1 record per drone per second = 86,400/day/drone
CREATE TABLE IF NOT EXISTS telemetry (
    drone_id            uuid,
    time_bucket         text,           -- Format: 'YYYYMMDDHH' (hourly buckets)
    recorded_at         timestamp,
    
    -- Position & movement
    position            frozen<coordinates>,
    velocity_mps        float,
    acceleration_mps2   float,
    bank_angle_deg      float,
    pitch_angle_deg     float,
    
    -- Current waypoint context
    current_waypoint    smallint,
    distance_to_next_km float,
    eta_next_waypoint   timestamp,
    
    -- Systems status
    fuel_remaining_pct  float,
    engine_rpm          int,
    engine_temp_c       float,
    battery_voltage     float,
    
    -- Sensors
    sensor_data         map<text, text>, -- Flexible sensor readings
    
    -- Environment
    wind_speed_mps      float,
    wind_direction_deg  float,
    temperature_c       float,
    visibility_km       float,
    
    -- Comms health
    link_status         frozen<comm_link>,
    mesh_connectivity   float,           -- 0.0 - 1.0 (% of mesh visible)
    
    PRIMARY KEY ((drone_id, time_bucket), recorded_at)
) WITH comment = 'Time-series telemetry data with hourly partitioning'
   AND CLUSTERING ORDER BY (recorded_at DESC)
   AND gc_grace_seconds = 86400
   AND default_time_to_live = 2592000   -- 30 days TTL
   AND compaction = {'class': 'TimeWindowCompactionStrategy', 
                     'compaction_window_size': 1, 
                     'compaction_window_unit': 'DAYS'};


-- ENGAGEMENTS: Weapon employment records
-- Partition: convoy_id (group engagements by mission)
-- Clustering: engaged_at DESC, engagement_id
CREATE TABLE IF NOT EXISTS engagements (
    convoy_id           uuid,
    engaged_at          timestamp,
    engagement_id       uuid,
    
    -- Shooter
    drone_id            uuid,
    drone_callsign      text,
    
    -- Weapon
    weapon_type         text,
    weapon_serial       text,
    
    -- Target
    target              frozen<target_info>,
    
    -- Authorization
    authorization_code  text,
    authorized_by       text,
    roe_compliance      boolean,
    
    -- Result
    result              frozen<engagement_result>,
    hit                 boolean,         -- Simple hit/miss for aggregation
    
    -- Context
    waypoint_number     smallint,
    shooter_position    frozen<coordinates>,
    range_to_target_km  float,
    
    -- BDA (Battle Damage Assessment)
    bda_status          text,            -- 'PENDING', 'CONFIRMED', 'DISPUTED'
    bda_notes           text,
    
    PRIMARY KEY (convoy_id, engaged_at, engagement_id)
) WITH comment = 'Weapon engagement records partitioned by convoy'
   AND CLUSTERING ORDER BY (engaged_at DESC, engagement_id ASC)
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'LeveledCompactionStrategy'};


-- ENGAGEMENTS BY DRONE: Alternate access pattern
-- Partition: drone_id
-- For per-drone engagement history and accuracy calculation
CREATE TABLE IF NOT EXISTS engagements_by_drone (
    drone_id            uuid,
    engaged_at          timestamp,
    engagement_id       uuid,
    
    convoy_id           uuid,
    weapon_type         text,
    target              frozen<target_info>,
    result              frozen<engagement_result>,
    hit                 boolean,
    range_to_target_km  float,
    
    PRIMARY KEY (drone_id, engaged_at, engagement_id)
) WITH comment = 'Engagement records partitioned by drone for accuracy queries'
   AND CLUSTERING ORDER BY (engaged_at DESC, engagement_id ASC)
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'LeveledCompactionStrategy'};


-- -----------------------------------------------------------------------------
-- LEADERBOARD SUPPORT TABLES
-- -----------------------------------------------------------------------------

-- ACCURACY COUNTERS: Real-time accuracy tracking using counters
-- Partition: convoy_id
-- Clustering: drone_id
-- Note: Counter tables have restrictions - only counter columns allowed
CREATE TABLE IF NOT EXISTS accuracy_counters (
    convoy_id           uuid,
    drone_id            uuid,
    
    total_engagements   counter,
    successful_hits     counter,
    
    PRIMARY KEY (convoy_id, drone_id)
) WITH comment = 'Counter table for real-time accuracy aggregation';


-- LEADERBOARD SNAPSHOT: Materialized leaderboard for fast reads
-- Partition: convoy_id
-- Clustering: accuracy_pct DESC, drone_id (rank by accuracy)
-- Manually maintained via application logic (ScyllaDB MVs don't support computed columns)
CREATE TABLE IF NOT EXISTS leaderboard (
    convoy_id           uuid,
    accuracy_pct        float,
    drone_id            uuid,
    
    -- Denormalized drone info
    callsign            text,
    platform_type       text,
    
    -- Stats
    total_engagements   int,
    successful_hits     int,
    
    -- Streak tracking
    current_streak      int,             -- Consecutive hits
    best_streak         int,
    
    -- Computed rank (maintained by app)
    rank                smallint,
    
    -- Timestamp
    updated_at          timestamp,
    
    PRIMARY KEY (convoy_id, accuracy_pct, drone_id)
) WITH comment = 'Pre-computed leaderboard sorted by accuracy'
   AND CLUSTERING ORDER BY (accuracy_pct DESC, drone_id ASC)
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'LeveledCompactionStrategy'};


-- -----------------------------------------------------------------------------
-- MATERIALIZED VIEWS
-- -----------------------------------------------------------------------------

-- View: Drones by status (for operational dashboards)
CREATE MATERIALIZED VIEW IF NOT EXISTS drones_by_status AS
    SELECT convoy_id, drone_id, status, callsign, platform_type, 
           current_position, fuel_remaining_pct, accuracy_pct
    FROM drones
    WHERE convoy_id IS NOT NULL 
      AND drone_id IS NOT NULL 
      AND status IS NOT NULL
    PRIMARY KEY (status, convoy_id, drone_id);


-- View: Active convoys (for command dashboard)

-- Note: ScyllaDB MVs only allow one non-PK column from base table
CREATE MATERIALIZED VIEW IF NOT EXISTS active_convoys AS
    SELECT convoy_id, convoy_callsign, mission_type, status, 
           aor_name, drone_count, mission_start
    FROM convoys
    WHERE convoy_id IS NOT NULL 
      AND status IS NOT NULL
    PRIMARY KEY (status, convoy_id);

-- -----------------------------------------------------------------------------
-- AUDIT & OPERATIONS
-- -----------------------------------------------------------------------------

-- COMMAND LOG: Audit trail for all commands issued
CREATE TABLE IF NOT EXISTS command_log (
    convoy_id           uuid,
    issued_at           timestamp,
    command_id          uuid,
    
    command_type        text,            -- 'ENGAGE', 'RTB', 'HOLD', 'PROCEED', 'ABORT'
    target_drone_id     uuid,
    target_drone_ids    set<uuid>,       -- For convoy-wide commands
    
    issued_by           text,
    authorization       text,
    
    parameters          map<text, text>,
    acknowledged        boolean,
    acknowledged_at     timestamp,
    
    PRIMARY KEY (convoy_id, issued_at, command_id)
) WITH comment = 'Command audit log'
   AND CLUSTERING ORDER BY (issued_at DESC, command_id ASC)
   AND gc_grace_seconds = 864000
   AND compaction = {'class': 'TimeWindowCompactionStrategy',
                     'compaction_window_size': 7,
                     'compaction_window_unit': 'DAYS'};


-- ALERTS: System alerts and notifications
CREATE TABLE IF NOT EXISTS alerts (
    convoy_id           uuid,
    alert_time          timestamp,
    alert_id            uuid,
    
    severity            text,            -- 'CRITICAL', 'WARNING', 'INFO'
    alert_type          text,            -- 'FUEL_LOW', 'LINK_LOST', 'THREAT_DETECTED', 'WAYPOINT_DEVIATION'
    source_drone_id     uuid,
    
    message             text,
    acknowledged        boolean,
    acknowledged_by     text,
    acknowledged_at     timestamp,
    
    PRIMARY KEY (convoy_id, alert_time, alert_id)
) WITH comment = 'Operational alerts and notifications'
   AND CLUSTERING ORDER BY (alert_time DESC, alert_id ASC)
   AND default_time_to_live = 604800    -- 7 days TTL
   AND compaction = {'class': 'TimeWindowCompactionStrategy',
                     'compaction_window_size': 1,
                     'compaction_window_unit': 'DAYS'};


-- =============================================================================
-- PREPARED STATEMENT HINTS (for application layer)
-- =============================================================================
-- 
-- Get convoy leaderboard (top 10):
--   SELECT * FROM leaderboard WHERE convoy_id = ? LIMIT 10;
--
-- Get drone telemetry (last hour):
--   SELECT * FROM telemetry 
--   WHERE drone_id = ? AND time_bucket = ? 
--   LIMIT 3600;
--
-- Get drone engagements:
--   SELECT * FROM engagements_by_drone 
--   WHERE drone_id = ? 
--   LIMIT 100;
--
-- Increment accuracy counter (after engagement):
--   UPDATE accuracy_counters 
--   SET total_engagements = total_engagements + 1,
--       successful_hits = successful_hits + ? 
--   WHERE convoy_id = ? AND drone_id = ?;
--
-- Get all drones in convoy:
--   SELECT * FROM drones WHERE convoy_id = ?;
--
-- Get drone waypoints:
--   SELECT * FROM waypoints WHERE drone_id = ?;
--
-- =============================================================================
