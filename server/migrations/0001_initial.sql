CREATE TABLE sensor
(
    id         int generated always as identity primary key,
    short_name text not null
);

CREATE UNIQUE INDEX uniq$sensor_short_name ON sensor (short_name);

CREATE TABLE sensor_data_v1
(
    id          int generated always as identity primary key,
    sensor_id   int              not null references sensor (id),
    timestamp   timestamptz      not null,
    temperature double precision not null,
    humidity    double precision not null
);

CREATE INDEX idx$sensor_data_v1_sensor_id_timestamp ON sensor_data_v1 (sensor_id, timestamp);
