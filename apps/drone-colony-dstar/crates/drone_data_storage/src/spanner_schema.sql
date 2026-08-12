CREATE TABLE drones (
    id INT64 NOT NULL,
    name STRING(50) NOT NULL,
    status STRING(20) NOT NULL,
    last_known_latitude FLOAT64,
    last_known_longitude FLOAT64,
    last_update TIMESTAMP DEFAULT CURRENT_TIMESTAMP(),
) PRIMARY KEY (id);

CREATE TABLE waypoints (
    id INT64 NOT NULL,
    drone_id INT64 NOT NULL,
    latitude FLOAT64 NOT NULL,
    longitude FLOAT64 NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP(),
) PRIMARY KEY (id),
INTERLEAVE IN PARENT drones ON DELETE CASCADE;

CREATE TABLE sortie (
    id INT64 NOT NULL,
    drone_id INT64 NOT NULL,
    status STRING(20) NOT NULL,
    start_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP(),
    end_time TIMESTAMP,
) PRIMARY KEY (id),
INTERLEAVE IN PARENT drones ON DELETE CASCADE;
